extern crate discord;
extern crate regex;
#[macro_use]
extern crate log;
extern crate env_logger;
extern crate serde;
extern crate serde_json;

use std::collections::HashMap;
use std::io;

use discord::{Discord, State};
use discord::model::Event;

mod grep;
use grep::Grep;

mod command;

mod message;

pub const TIMEOUT: u64 = 5 * 60; // 5 minutes

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
                        message::handle(&message, &greps, &mut timeouts)
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
