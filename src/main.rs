extern crate discord;
extern crate regex;
#[macro_use]
extern crate log;
extern crate serde;
extern crate serde_json;
extern crate mvdb;
extern crate dotenv;
extern crate fern;
extern crate chrono;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::env;
use std::path::Path;

use discord::{Discord, State};
use discord::model::Event;

use mvdb::Mvdb;

mod grep;
use grep::Grep;

mod command;

mod message;

const TIME_OUT: u64 = 5 * 60; // 5 minutes
const DISCORD_BOT_TOKEN: &'static str = "DISCORD_BOT_TOKEN";
const STORAGE_FILE: &'static str = "STORAGE_FILE";
const LOG_FILE: &'static str = "LOG_FILE";

fn main() {
    setup();

    // state
    let greps: Mvdb<HashSet<Grep>> = Mvdb::from_file(Path::new(&env::var(STORAGE_FILE).unwrap()))
        .or_else(|_| {
            Mvdb::new(
                Default::default(),
                Path::new(&env::var(STORAGE_FILE).unwrap()),
            )
        })
        .expect("Could not access db file");
    let timeouts = RefCell::new(HashMap::new());

    // api
    let mut discord =
        Discord::from_bot_token(&env::var(DISCORD_BOT_TOKEN).unwrap()).expect("Login Failed");
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
            Ok(event) => if let Event::MessageCreate(message) = event {
                let response = if message.author.bot {
                    None
                } else if message.mentions.iter().any(|user| user.id == uid) {
                    match greps.access_mut(|greps| Some(command::handle(&message, greps))) {
                        Ok(response) => response,
                        Err(e) => {
                            error!("Could not access file: {}", e);
                            None
                        }
                    }
                } else {
                    match greps.access(|greps| {
                        message::handle(&message, greps, &mut *timeouts.borrow_mut())
                    }) {
                        Ok(response) => response,
                        Err(e) => {
                            error!("Could not access file: {}", e);
                            None
                        }
                    }
                };

                if let Some(content) = response {
                    match discord.send_message(message.channel_id, &content, "", false) {
                        Ok(_) => (),
                        Err(e) => error!("Could not send message: {}", e),
                    }
                }
            },
            Err(e) => {
                error!("Could not recieve event from discord: {}", e);
                discord = Discord::from_bot_token(&env::var(DISCORD_BOT_TOKEN).unwrap())
                    .expect("Login failed");
                connection = discord
                    .connect()
                    .map(|(conn, _)| conn)
                    .expect("Could not connect to websocket API");
            }
        }
    }
}

/// Initialises everything.
/// Will panic if errors occur at this stage.
fn setup() {
    dotenv::dotenv().unwrap();
    env::var(DISCORD_BOT_TOKEN).expect("Please specify the environment variable DISCORD_BOT_TOKEN");
    env::var(STORAGE_FILE).expect("Please specify the environment variable STORAGE_FILE");
    env::var(LOG_FILE).expect("Please specify the environment variable LOG_FILE");

    fern::Dispatch::new()
        .format(|out, message, record| if record.target() == "grepbot" {
            out.finish(format_args!(
                "{} [{}] {}",
                chrono::Utc::now().format("%+"),
                record.level(),
                message
            ))
        } else {
            out.finish(format_args!(
                "{} [{}][{}] {}",
                chrono::Utc::now().format("%+"),
                record.target(),
                record.level(),
                message
            ))
        })
        .level(log::LogLevelFilter::Warn)
        .level_for("grepbot", log::LogLevelFilter::Info)
        .chain(fern::log_file(env::var(LOG_FILE).unwrap()).unwrap())
        .apply()
        .unwrap();

    info!("Starting the bot");
}
