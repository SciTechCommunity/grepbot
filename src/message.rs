//! Message handling.
//! This module defines the functions used to handle messages the bot reads that are not commands -
//! ie it determines if a message matches any greps, and figures out who to tag.

use {Grep, TIMEOUT};

use std::collections::{HashSet, HashMap};
use std::time::{Duration, Instant};

use discord::model::{ChannelId, Message, UserId};

pub fn handle(message: &Message,
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
