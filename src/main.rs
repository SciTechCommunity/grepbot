extern crate discord;
extern crate regex;
#[macro_use]
extern crate log;
extern crate env_logger;
extern crate serde;
extern crate serde_json;

use std::collections::{HashSet, HashMap};
use std::env;
use std::io;
use std::time::{Duration, Instant};

use discord::{Discord, State};
use discord::model::{Event, ChannelId, Message, UserId};

use regex::Regex;


mod grep;
use grep::Grep;

const TIMEOUT: u64 = 5 * 60; // 5 minutes

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
    } else if content == "save" {
        serde_json::to_string(greps).unwrap()
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
    let mut greps = serde_json::from_reader(io::stdin()).unwrap_or_default();
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
