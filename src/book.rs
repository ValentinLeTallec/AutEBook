use rss::Channel;
use std::error::Error;
use std::fmt::Debug;
use std::fs;
use std::{path::Path, process::Command};

use crate::source;
use crate::source::{FanFicFare, Syndication};

// mod source;

// #[derive(Debug)]
pub struct Book<'a> {
    path: &'a Path,
    source: Option<Box<dyn FanFicFare>>,
}

// impl Debug for Book {
//     fn
// }

impl<'a> Book<'a> {
    pub fn new(path: &'a Path) -> Book<'a> {
        Book {
            path,
            source: source::get(path),
        }
    }

    pub fn update(&self) {
        let output = Command::new("fanficfare")
            .arg("--non-interactive")
            .arg("-u")
            .arg(self.path)
            .output()
            .expect("Failed to execute command");

        // assert_eq!(b"Hello world\n", output.stdout.as_slice());
    }

    pub fn has_new_chapters(&self) -> bool {
        todo!();
    }

    async fn example_feed() -> Result<Channel, Box<dyn Error>> {
        let content = reqwest::get("http://example.com/feed.xml")
            .await?
            .bytes()
            .await?;
        let channel = Channel::read_from(&content[..])?;
        Ok(channel)
    }
}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        assert!(true);
    }
}
