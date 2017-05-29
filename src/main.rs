extern crate discord;
extern crate regex;
#[macro_use]
extern crate log;
extern crate env_logger;
extern crate serde;
extern crate serde_json;

use std::collections::{HashSet, HashMap};
use std::env;
use std::cmp::{PartialEq, Eq};
use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant};

use discord::{Discord, State};
use discord::model::{Event, ChannelId, Message, UserId};

use regex::Regex;

use serde::{Serialize, Serializer, Deserialize, Deserializer};
use serde::de::Error;

const TIMEOUT: u64 = 5 * 60; // 5 minutes

/// Wrapper around a `Regex, UserId` tuple that implements `PartialEq`, `Eq` and `Hash` manually,
/// using the `Regex::as_str()` function as the `Regex` object itself cannot be hashed.
struct Grep(Regex, UserId);

impl PartialEq<Grep> for Grep {
    fn eq(&self, other: &Self) -> bool {
        let Grep(ref regex, id) = *self;
        let Grep(ref other_regex, other_id) = *other;

        regex.as_str() == other_regex.as_str() && id == other_id
    }
}

impl Eq for Grep {}

impl Hash for Grep {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let Grep(ref regex, id) = *self;
        regex.as_str().hash(state);
        id.hash(state);
    }
}

impl Serialize for Grep {
    fn serialize<S>(&self, serializer: &mut S) -> Result<(), <S as Serializer>::Error>
        where S: Serializer
    {
        let Grep(ref regex, UserId(id)) = *self;
        Serialize::serialize(&(regex.as_str(), id), serializer)
    }
}

impl Deserialize for Grep {
    fn deserialize<D>(deserializer: &mut D) -> Result<Self, <D as Deserializer>::Error>
        where D: Deserializer
    {
        let (regex, id): (String, u64) = Deserialize::deserialize(deserializer)?;
        let regex = Regex::new(&regex)
            .map_err(|e| <D as Deserializer>::Error::custom(format!("{}", e)))?;
        let id = UserId(id);
        Ok(Grep(regex, id))
    }
}

fn handle_command(message: &Message, greps: &mut HashSet<Grep>) -> String {
    let content = &message.content;
    let author = &message.author;
    // split the message
    let content = {
        let index = match content.find(' ') {
            Some(index) => index,
            None => return include_str!("usage.md").into(),
        };
        let (mention, content) = content.split_at(index);
        if !mention.starts_with("<@") {
            return include_str!("usage.md").into();
        }
        &content[1..]
    };
    if content == "help" {
        include_str!("help.md").into()
    } else if content == "list" {
        if greps.iter().any(|&Grep(_, id)| id == author.id) {
            greps
                .iter()
                .filter(|&&Grep(_, id)| id == author.id)
                .map(|&Grep(ref regex, _)| regex)
                .fold(String::new(),
                      |string, regex| format!("{}\n{}", string, regex))
        } else {
            "you have no greps".into()
        }
    } else if content.starts_with("add ") {
        content
            .splitn(2, ' ')
            .nth(1)
            .map(|pattern| match Regex::new(pattern) {
                     Ok(regex) => {
                         if greps
                                .iter()
                                .any(|&Grep(ref regex, id)| {
                                         id == author.id && regex.as_str() == pattern
                                     }) {
                             "Regex already exists".into()
                         } else {
                             greps.insert(Grep(regex, author.id));
                             "Regex added".into()
                         }
                     }
                     Err(error) => format!("Invalid regex. {}", error),
                 })
            .unwrap()
    } else if content.starts_with("remove ") {
        content
            .splitn(2, ' ')
            .nth(1)
            .map(|pattern| {
                let mut removals = false;
                greps.retain(|&Grep(ref regex, id)| if id == author.id &&
                                                       regex.as_str() == pattern {
                                 removals = true;
                                 false
                             } else {
                                 true
                             });
                if removals {
                    format!("Regex {} removed", pattern)
                } else {
                    format!("Regex {} was not found", pattern)
                }
            })
            .unwrap()
    } else if content == "syntax" {
        include_str!("syntax.md").into()
    } else if content == "source" {
        "https://github.com/TumblrCommunity/grepbot".into()
    } else if content == "author" {
        "talk to artemis (https://github.com/ashfordneil)".into()
    } else {
        include_str!("usage.md").into()
    }
}

fn handle_message(message: &Message,
                  greps: &HashSet<Grep>,
                  timeouts: &mut HashMap<(UserId, ChannelId), Instant>)
                  -> Option<String> {
    let users: HashSet<_> = greps
        .iter()
        .filter(|&&Grep(ref regex, _)| regex.is_match(&message.content))
        .map(|&Grep(_, id)| id)
        .filter(|&id| id != message.author.id)
        .filter(|&id| match timeouts.get(&(id, message.channel_id)) {
                    Some(instant) => instant.elapsed() > Duration::from_secs(TIMEOUT),
                    None => true,
                })
        .collect();
    if !users.is_empty() {
        Some(users
                 .into_iter()
                 .inspect(|&id| { timeouts.insert((id, message.channel_id), Instant::now()); })
                 .fold("Hey!".into(),
                       |string, id| format!("{} {}", string, id.mention())))
    } else {
        None
    }
}

fn main() {
    env_logger::init().unwrap();
    // state
    let mut greps = HashSet::new();
    let mut timeouts = HashMap::new();
    // api
    let mut discord = Discord::from_bot_token(&env::var("DISCORD_BOT_TOKEN")
                                                   .expect("DISCORD_BOT_TOKEN not set"))
            .expect("Login Failed");
    let (mut connection, event) = discord
        .connect()
        .expect("Could not connect to websocket API");
    let uid = {
        let state = State::new(event);
        state.user().id
    };
    connection.set_game_name("I grep things for you".to_string());
    // main loop time
    loop {
        match connection.recv_event() {
            Ok(event) => {
                if let Event::MessageCreate(message) = event {
                    let response = if message.author.bot {
                        None
                    } else if message.mentions.iter().any(|user| user.id == uid) {
                        Some(handle_command(&message, &mut greps))
                    } else {
                        handle_message(&message, &greps, &mut timeouts)
                    };
                    if let Some(content) = response {
                        match discord.send_message(message.channel_id, &content, "", false) {
                            Ok(_) => (),
                            Err(e) => error!("Could not send message: {}", e),
                        }
                    }
                }
            }
            Err(e) => {
                error!("Could not recieve event from discord: {}", e);
                discord = Discord::from_bot_token(&env::var("DISCORD_BOT_TOKEN").unwrap())
                    .expect("Login failed");
                connection = discord
                    .connect()
                    .map(|(conn, _)| conn)
                    .expect("Could not connect to websocket API");
            }
        }
    }
}
