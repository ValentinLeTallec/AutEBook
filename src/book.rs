use lazy_static::lazy_static;
use regex::Regex;
use rss::Channel;
use std::error::Error;
use std::fmt::{Debug, Formatter};
use std::io;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::ChildStdout;
use std::process::{Command, Stdio};

lazy_static! {
    static ref DO_UPDATE: Regex =
        Regex::new(r"^Do update - epub\((\d+)\) vs url\((\d+)\)$").unwrap();
    static ref UPDATING: Regex = Regex::new(r"^Updating .*, URL: .*$").unwrap();
    static ref UP_TO_DATE: Regex = Regex::new(r"^.* already contains \d+ chapters\.$").unwrap();
    static ref STUB: Regex =
        Regex::new(r"^.* contains \d+ chapters, more than source: \d+\.$").unwrap();
}

use crate::source;
use crate::source::{FanFicFare, Syndication, UpdateResult};

// mod source;

// #[derive(Debug)]
pub struct Book {
    path: Box<Path>,
    source: Option<Box<dyn FanFicFare>>,
}

impl Book {
    pub fn new(path: &Path) -> Book {
        Book {
            path: path.to_path_buf().into_boxed_path(),
            source: source::get(&path),
        }
    }
    pub fn print_path(&self) {
        println!("{}", self.path.to_str().unwrap());
    }

    pub fn update(&self) {
        self.call_fanficfare();
    }

    fn parse_output(stdout: Option<ChildStdout>) -> Option<UpdateResult> {
        BufReader::new(stdout?)
            .lines()
            .filter_map(|line| line.ok())
            .filter(|line| UPDATING.captures(&line).is_none())
            // .inspect(|line| println!("~~~> {:?}", line))
            // .inspect(|line| println!("DO_UPDATE {:?}", DO_UPDATE.captures(&line)))
            // .inspect(|line| println!("UP_TO_DATE {:?}", UP_TO_DATE.captures(&line)))
            // .inspect(|line| println!("STUB {:?}", STUB.captures(&line)))
            // .inspect(|line| println!("UPDATING {:?}", UPDATING.captures(&line)))
            .filter_map(|line| {
                if let Some(c) = DO_UPDATE.captures(&line) {
                    let nb_chapter_epub = &c[1].parse::<u16>().ok()?;
                    let nb_chapter_url = &c[2].parse::<u16>().ok()?;
                    return Some(UpdateResult::Updated(nb_chapter_url - nb_chapter_epub));
                }
                None
            })
            .nth(0)
    }

    pub fn call_fanficfare(&self) -> Result<UpdateResult, Box<dyn Error>> {
        match self.source {
            Some(_) => {
                let path = self
                    .path
                    .to_str()
                    .ok_or(io::Error::from(io::ErrorKind::Unsupported))?;

                let mut cmd = Command::new("fanficfare")
                    .arg("--non-interactive")
                    .arg("--update-epub")
                    .arg("--no-output") // TODO : remove line
                    .arg(path)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()?;

                // Book::parse_output(cmd.stdout);
                //     .wait_with_output()?;

                // let stdin = String::from_utf8(output.stdout)?;
                let update_result = Book::parse_output(cmd.stdout)
                    .ok_or(io::Error::from(io::ErrorKind::Unsupported))?;
                Ok(update_result)
            }
            None => {
                // println!("need to save it");
                Ok(UpdateResult::NotSupported)
            }
        }

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

impl Debug for Book {
    fn fmt(&self, _: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        print!(
            "Book : {{ path: {}, source: {}}}",
            self.path.display(),
            if let Some(_) = self.source {
                true
            } else {
                false
            }
        );
        Ok(())
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
