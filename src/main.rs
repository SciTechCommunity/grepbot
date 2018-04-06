extern crate mvdb;
extern crate serenity;
#[macro_use]
extern crate log;
extern crate chrono;
extern crate dotenv;
extern crate fern;
extern crate regex;
extern crate serde;
extern crate serde_json;

mod config;
use config::Config;

mod grep;

mod handler;
use handler::Handler;

use serenity::Client;

fn main() {
    let config = Config::new();
    config.setup_logger();

    Client::new(&(config.discord_bot_token), Handler::new(&config)).unwrap().start().unwrap()
}
