extern crate discord;
extern crate regex;

use std::collections::{HashSet, HashMap};
use std::env;
use std::time::{Duration, Instant};

use discord::Discord;
use discord::model::{Event, ChannelId, Message, UserId};

use regex::Regex;

const TIMEOUT: u64 = 5 * 60; // 5 minutes

fn handle_message(message: Message,
                  greps: &mut Vec<(Regex, UserId)>,
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
    } else if content.starts_with("!grep") {
        Some(match Regex::new(&content[6..]) {
            Ok(regex) => {
                if greps.iter()
                    .filter(|&&(_, id)| id == author.id)
                    .filter(|&&(ref regex, _)| regex.as_str() == &content[6..])
                    .next()
                    .is_some() {
                    format!("Regex already exists")
                } else {
                    greps.push((regex, author.id));
                    format!("Regex Added")
                }
            }
            Err(error) => format!("Invalid regex. {}", error),
        })
    } else if content.starts_with("!ungrep") {
        let mut removals = false;
        greps.retain(|&(ref regex, id)| {
            if id != author.id {
                true
            } else if regex.as_str() == &content[8..] {
                removals = true;
                false
            } else {
                true
            }
        });
        match removals {
            true => Some(format!("Regex {} removed", &content[8..])),
            false => Some(format!("Regex {} was not found", &content[8..])),
        }
    } else if content == "!mygreps" {
        Some(greps.iter()
            .filter(|&&(_, id)| id == author.id)
            .map(|&(ref regex, _)| regex)
            .fold(String::new(),
                  |string, ref regex| format!("{}\n{}", string, regex)))
    } else {
        let users: HashSet<_> = greps.iter()
            .filter(|&&(ref regex, _)| regex.is_match(&content))
            .map(|&(_, id)| id)
            .filter(|&id| id != author.id)
            .filter(|&id| match timeouts.get(&(id, channel)) {
                Some(instant) => instant.elapsed() > Duration::from_secs(TIMEOUT),
                None => true,
            })
            .collect();
        if users.len() > 0 {
            Some(users.into_iter()
                .inspect(|&id| {
                    timeouts.insert((id, channel), Instant::now());
                })
                .fold(format!("Hey!"),
                      |string, id| format!("{} {}", string, id.mention())))
        } else {
            None
        }
    }
}

fn main() {
    // state
    let mut greps = Vec::new();
    let mut timeouts = HashMap::new();
    // api
    let discord = Discord::from_bot_token(&env::var("DISCORD_BOT_TOKEN")
            .expect("DISCORD_BOT_TOKEN not set"))
        .expect("Login Failed");
    let mut connection = match discord.connect() {
        Ok((connection, _)) => connection,
        Err(_) => panic!("Unable to connect to discord API"),
    };
    // generic fun stuff
    connection.set_game_name("Talk to me with !grephelp".to_string());
    // main loop time
    while let Ok(event) = connection.recv_event() {
        match event {
            Event::MessageCreate(message) => {
                let channel = message.channel_id;
                match handle_message(message, &mut greps, &mut timeouts) {
                    Some(content) => {
                        let _ = discord.send_message(channel, &content, "", false);
                    }
                    None => {}
                };
            }
            _ => {}
        }
    }
}
