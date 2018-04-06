use config::{Config, TIME_OUT};
use grep::Grep;

use std::time::Instant;
use std::sync::Mutex;
use std::path::Path;
use std::collections::{HashMap, HashSet};

use regex::Regex;

use serde_json;

use serenity::prelude::Mentionable;
use serenity::client::{Context, EventHandler};
use serenity::model::user::User;
use serenity::model::channel::Message;
use serenity::model::id::{ChannelId, UserId};

use mvdb::Mvdb;

/// The event handler for the grep bot.
pub struct Handler {
    greps: Mvdb<HashSet<Grep>>,
    timeouts: Mutex<HashMap<(UserId, ChannelId), Instant>>,
}

impl Handler {
    /// Creates a new handler.
    pub fn new(config: &Config) -> Self {
        let greps = Mvdb::from_file(Path::new(&(config.storage_file)))
            .or_else(|_| Mvdb::new(Default::default(), Path::new(&(config.storage_file))))
            .unwrap();
        let timeouts = Mutex::new(HashMap::new());

        Handler { greps, timeouts }
    }

    fn is_self(&self, user: &User) -> bool {
        user.discriminator == 9866 && user.name == "TestApp"
    }

    fn handle_command(&self, message: &Message) -> Option<String> {
        let output = self.greps.access_mut(|greps| {
            let author = &message.author;

            let content = {
                let index = match message.content.find(' ') {
                    Some(index) => index,
                    None => return include_str!("usage.md").into(),
                };
                let (mention, content) = message.content.split_at(index);
                if !mention.starts_with("<@") {
                    return include_str!("usage.md").into();
                }
                &content[1..]
            };

            if content == "help" {
                include_str!("help.md").into()
            } else if content == "list" {
                Self::list_greps(greps, author)
            } else if content.starts_with("add ") {
                Self::add_grep(content, greps, author)
            } else if content.starts_with("remove ") {
                Self::remove_grep(content, greps, author)
            } else if content == "save" {
                format!("`{}`", serde_json::to_string(greps).unwrap())
            } else if content == "syntax" {
                include_str!("syntax.md").into()
            } else if content == "source" {
                "https://github.com/TumblrCommunity/grepbot".into()
            } else if content == "author" {
                "talk to artemis (https://github.com/ashfordneil)".into()
            } else {
                include_str!("usage.md").into()
            }
        });

        match output {
            Ok(response) => Some(response),
            Err(e) => {
                error!("Could not access file: {}", e);
                None
            }
        }
    }

    /// Lists the greps for a given user. Accepts the set of all greps and the ID of a user as
    /// arguments, and returns a newline delimited list of all the regular expressions associated with
    /// that user.
    fn list_greps(greps: &HashSet<Grep>, author: &User) -> String {
        if greps.iter().any(|&Grep(_, id)| id == author.id) {
            greps
                .iter()
                .filter(|&&Grep(_, id)| id == author.id)
                .map(|&Grep(ref regex, _)| regex)
                .fold(String::new(), |string, regex| {
                    format!("{}\n{}", string, regex)
                })
        } else {
            "you have no greps".into()
        }
    }

    /// Adds a new grep for a user. Accepts a message that beings with "add ", and then parses the
    /// remainder of the message as a regular expression, and updates the internal state in `greps` so
    /// that the user the function is called with is now listening for that grep.
    fn add_grep(content: &str, greps: &mut HashSet<Grep>, author: &User) -> String {
        content
            .splitn(2, ' ')
            .nth(1)
            .map(|pattern| match Regex::new(pattern) {
                Ok(regex) => if greps
                    .iter()
                    .any(|&Grep(ref regex, id)| id == author.id && regex.as_str() == pattern)
                {
                    "Regex already exists".into()
                } else {
                    greps.insert(Grep(regex, author.id));
                    "Regex added".into()
                },
                Err(error) => format!("Invalid regex. {}", error),
            })
            .unwrap()
    }

    /// Removes a grep from a user. Accepts a message that beings with "remove ", and then removes the
    /// grep associated with the user whos regular expression is equal to the remainder of the message.
    fn remove_grep(content: &str, greps: &mut HashSet<Grep>, author: &User) -> String {
        content
            .splitn(2, ' ')
            .nth(1)
            .map(|pattern| {
                let mut removals = false;
                greps.retain(|&Grep(ref regex, id)| {
                    if id == author.id && regex.as_str() == pattern {
                        removals = true;
                        false
                    } else {
                        true
                    }
                });
                if removals {
                    format!("Regex {} removed", pattern)
                } else {
                    format!("Regex {} was not found", pattern)
                }
            })
            .unwrap()
    }

    /// Message parser. Recieves a message and determines if it matches any greps that were not tagged
    /// within the time frame specified. If it does, generate a reply to the message that tags all
    /// users whos greps matched the message.
    pub fn handle_message(&self, message: &Message) -> Option<String> {
        let output = self.greps.access(|greps| {
            let mut timeouts = self.timeouts.lock().unwrap();

            let users: HashSet<_> = greps
                .iter()
                .filter(|&&Grep(ref regex, _)| regex.is_match(&message.content))
                .map(|&Grep(_, id)| id)
                .filter(|&id| id != message.author.id)
                .filter(|&id| match timeouts.get(&(id, message.channel_id)) {
                    Some(instant) => instant.elapsed() > TIME_OUT,
                    None => true,
                })
                .collect();

            if !users.is_empty() {
                Some(
                    users
                        .into_iter()
                        .inspect(|&id| {
                            timeouts.insert((id, message.channel_id), Instant::now());
                        })
                        .fold("Hey!".into(), |string, id| {
                            format!("{} {}", string, id.mention())
                        }),
                )
            } else {
                None
            }
        });

        match output {
            Ok(response) => response,
            Err(e) => {
                error!("Could not access file: {}", e);
                None
            }
        }
    }
}

impl EventHandler for Handler {
    fn message(&self, _ctx: Context, message: Message) {
        let response = if message.author.bot {
            None
        } else if message.mentions.iter().any(|user| self.is_self(user)) {
            self.handle_command(&message)
        } else {
            self.handle_message(&message)
        };

        if let Some(content) = response {
            match message.channel_id.say(content) {
                Ok(_) => (),
                Err(e) => error!("Could not send message: {}", e),
            }
        }
    }
}
