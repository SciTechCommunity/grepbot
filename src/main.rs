extern crate discord;
extern crate regex;

use std::collections::{HashSet, HashMap};
use std::env;
use std::cmp::{PartialEq, Eq};
use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant};

use discord::Discord;
use discord::model::{Event, ChannelId, Message, UserId};

use regex::Regex;

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

fn handle_message(message: Message,
                  greps: &mut HashSet<Grep>,
                  timeouts: &mut HashMap<(UserId, ChannelId), Instant>)
                  -> Option<String> {
    let channel = message.channel_id;
    let content = message.content;
    let author = message.author;

    if author.bot {
        return None;
    }

    if content == "!grephelp" {
        Some(include_str!("help.md").into())
    } else if content == "!mygreps" {
        Some(greps.iter()
            .filter(|&&Grep(_, id)| id == author.id)
            .map(|&Grep(ref regex, _)| regex)
            .fold(String::new(),
                  |string, regex| format!("{}\n{}", string, regex)))
    } else if content.starts_with("!grep ") {
        content.splitn(2, ' ').nth(1).map(|pattern| match Regex::new(pattern) {
            Ok(regex) => {
                if greps.iter()
                    .any(|&Grep(ref regex, id)| id == author.id && regex.as_str() == pattern) {
                    "Regex already exists".into()
                } else {
                    greps.insert(Grep(regex, author.id));
                    "Regex added".into()
                }
            }
            Err(error) => format!("Invalid regex. {}", error),
        })
    } else if content.starts_with("!ungrep ") {
        content.splitn(2, ' ').nth(1).map(|pattern| {
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
    } else {
        let users: HashSet<_> = greps.iter()
            .filter(|&&Grep(ref regex, _)| regex.is_match(&content))
            .map(|&Grep(_, id)| id)
            .filter(|&id| id != author.id)
            .filter(|&id| match timeouts.get(&(id, channel)) {
                Some(instant) => instant.elapsed() > Duration::from_secs(TIMEOUT),
                None => true,
            })
            .collect();
        if !users.is_empty() {
            Some(users.into_iter()
                .inspect(|&id| {
                    timeouts.insert((id, channel), Instant::now());
                })
                .fold("Hey!".into(),
                      |string, id| format!("{} {}", string, id.mention())))
        } else {
            None
        }
    }
}

fn main() {
    // state
    let mut greps = HashSet::new();
    let mut timeouts = HashMap::new();
    // api
    let discord = Discord::from_bot_token(&env::var("DISCORD_BOT_TOKEN")
            .expect("DISCORD_BOT_TOKEN not set"))
        .expect("Login Failed");
    let mut connection = match discord.connect() {
        Ok((connection, _)) => connection,
        Err(e) => panic!("Unable to connect to discord API: {}", e),
    };
    // generic fun stuff
    connection.set_game_name("Talk to me with !grephelp".to_string());
    // main loop time
    while let Ok(event) = connection.recv_event() {
        if let Event::MessageCreate(message) = event {
            let channel = message.channel_id;
            if let Some(content) = handle_message(message, &mut greps, &mut timeouts) {
                let _ = discord.send_message(channel, &content, "", false);
            }
        }
    }
}
