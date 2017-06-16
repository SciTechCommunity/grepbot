extern crate discord;
extern crate regex;
#[macro_use]
extern crate log;
extern crate env_logger;
extern crate serde;
extern crate serde_json;

use std::collections::{HashSet, HashMap};
use std::io;
use std::time::{Duration, Instant};

use discord::{Discord, State};
use discord::model::{Event, ChannelId, Message, UserId};

mod grep;
use grep::Grep;

mod command;

const TIMEOUT: u64 = 5 * 60; // 5 minutes

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
    let mut discord = Discord::from_bot_token(env!("DISCORD_BOT_TOKEN")).expect("Login Failed");
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
                        Some(command::handle(&message, &mut greps))
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
                discord = Discord::from_bot_token(env!("DISCORD_BOT_TOKEN")).expect("Login failed");
                connection = discord
                    .connect()
                    .map(|(conn, _)| conn)
                    .expect("Could not connect to websocket API");
            }
        }
    }
}
