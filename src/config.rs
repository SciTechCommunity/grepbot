use log::LevelFilter;
use chrono::Utc;
use dotenv;
use fern;

use std::env;
use std::time::Duration;

/// The minimum time between two pings for a single user in a single channel - 5 minutes.
pub const TIME_OUT: Duration = Duration::from_secs(5 * 60);

/// The configuration information for the grep bot.
pub struct Config {
    /// The token used to connect to discord.
    pub discord_bot_token: String,
    /// The file used to store all greps.
    pub storage_file: String,
    /// The log file that all output goes to.
    pub log_file: String,
}

impl Config {
    /// Will read environment variables, populate the config struct, and panic on error.
    pub fn new() -> Self {
        dotenv::dotenv().unwrap();
        let discord_bot_token = env::var("DISCORD_BOT_TOKEN").unwrap();
        let storage_file = env::var("STORAGE_FILE").unwrap();
        let log_file = env::var("LOG_FILE").unwrap();

        Config {
            discord_bot_token,
            storage_file,
            log_file,
        }
    }

    /// Initializes the logger for the process. Will panic on error.
    pub fn setup_logger(&self) {
        fern::Dispatch::new()
            .format(|out, message, record| {
                if record.target() == "grepbot" {
                    out.finish(format_args!(
                        "{} [{}] {}",
                        Utc::now().format("%+"),
                        record.level(),
                        message
                    ))
                } else {
                    out.finish(format_args!(
                        "{} [{}][{}] {}",
                        Utc::now().format("%+"),
                        record.target(),
                        record.level(),
                        message
                    ))
                }
            })
            .level(LevelFilter::Warn)
            .level_for("grepbot", LevelFilter::Info)
            .chain(fern::log_file(&self.log_file).unwrap())
            .apply()
            .unwrap();
    }
}
