//! Command handling.
//! This module defines the functions used to handle commands that are passed to the bot. These
//! commands are listed in `usage.md`.

use grep::Grep;

use std::collections::HashSet;

use discord::model::{Message, User};

use regex::Regex;

use serde_json;

/// Top level command handler. Recieves a message that is guaranteed to contain a mention to the
/// current bot user. Parses the message, updates the internal status in `greps` if necessary, and
/// returns the bot's response.
pub fn handle(message: &Message, greps: &mut HashSet<Grep>) -> String {
    let author = &message.author;

    let content = {
        let index = match message.content.find(' ') {
            Some(index) => index,
            None => return include_str!("usage.md").into(),
        };
        let (mention, content) = message.content.split_at(index);
        if !mention.starts_with("<@") {
            return include_str!("usage.md").into();
        }
        &content[1..]
    };

    if content == "help" {
        include_str!("help.md").into()
    } else if content == "list" {
        list_greps(greps, author)
    } else if content.starts_with("add ") {
        add_grep(content, greps, author)
    } else if content.starts_with("remove ") {
        remove_grep(content, greps, author)
    } else if content == "save" {
        format!("`{}`", serde_json::to_string(greps).unwrap())
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

/// Lists the greps for a given user. Accepts the set of all greps and the ID of a user as
/// arguments, and returns a newline delimited list of all the regular expressions associated with
/// that user.
fn list_greps(greps: &HashSet<Grep>, author: &User) -> String {
    if greps.iter().any(|&Grep(_, id)| id == author.id) {
        greps
            .iter()
            .filter(|&&Grep(_, id)| id == author.id)
            .map(|&Grep(ref regex, _)| regex)
            .fold(String::new(), |string, regex| {
                format!("{}\n{}", string, regex)
            })
    } else {
        "you have no greps".into()
    }
}

/// Adds a new grep for a user. Accepts a message that beings with "add ", and then parses the
/// remainder of the message as a regular expression, and updates the internal state in `greps` so
/// that the user the function is called with is now listening for that grep.
fn add_grep(content: &str, greps: &mut HashSet<Grep>, author: &User) -> String {
    content
        .splitn(2, ' ')
        .nth(1)
        .map(|pattern| match Regex::new(pattern) {
            Ok(regex) => if greps.iter().any(|&Grep(ref regex, id)| {
                id == author.id && regex.as_str() == pattern
            }) {
                "Regex already exists".into()

            } else {
                greps.insert(Grep(regex, author.id));
                "Regex added".into()
            },
            Err(error) => format!("Invalid regex. {}", error),
        })
        .unwrap()
}

/// Removes a grep from a user. Accepts a message that beings with "remove ", and then removes the
/// grep associated with the user whos regular expression is equal to the remainder of the message.
fn remove_grep(content: &str, greps: &mut HashSet<Grep>, author: &User) -> String {
    content
        .splitn(2, ' ')
        .nth(1)
        .map(|pattern| {
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
        .unwrap()
}
