use crate::updater::Update;
use crate::updater::UpdateResult;

use lazy_static::lazy_static;
use regex::Regex;
// use rss::Channel;
use std::error::Error;
use std::io;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

lazy_static! {
    static ref UPDATING: Regex = Regex::new(r"^Updating .*, URL: .*$").unwrap();
    static ref UP_TO_DATE: Regex = Regex::new(r"^.* already contains \d+ chapters\.$").unwrap();
    static ref DO_UPDATE: Regex =
        Regex::new(r"^Do update - epub\((\d+)\) vs url\((\d+)\)$").unwrap();
    static ref MORE_CHAPTER_THAN_SOURCE: Regex =
        Regex::new(r"^.* contains (\d+) chapters, more than source: (\d+)\.$").unwrap();
}

pub struct FanFicFare;

impl FanFicFare {
    fn do_update(path: &Path) -> Result<UpdateResult, Box<dyn Error>> {
        let path = path
            .to_str()
            .ok_or_else(|| io::Error::from(io::ErrorKind::Unsupported))?;

        let cmd = Command::new("fanficfare")
            .arg("--non-interactive")
            .arg("--update-epub")
            // .arg("--no-output") // TODO : remove line
            .arg(path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdout = cmd
            .stdout
            .ok_or_else(|| io::Error::from(io::ErrorKind::Unsupported))?;
        let update_result = BufReader::new(stdout)
            .lines()
            .filter_map(std::result::Result::ok)
            .filter(|line| UPDATING.captures(line).is_none())
            .find_map(|line| {
                if UP_TO_DATE.captures(&line).is_some() {
                    return Some(UpdateResult::UpToDate);
                }
                if let Some(c) = DO_UPDATE.captures(&line) {
                    let nb_chapter_epub = &c[1].parse::<u16>().ok()?;
                    let nb_chapter_url = &c[2].parse::<u16>().ok()?;
                    return Some(UpdateResult::Updated(nb_chapter_url - nb_chapter_epub));
                }
                if let Some(c) = MORE_CHAPTER_THAN_SOURCE.captures(&line) {
                    let nb_chapter_epub = &c[1].parse::<u16>().ok()?;
                    let nb_chapter_url = &c[2].parse::<u16>().ok()?;
                    return Some(UpdateResult::MoreChapterThanSource(
                        nb_chapter_epub - nb_chapter_url,
                    ));
                }
                None
            })
            .ok_or_else(|| io::Error::from(io::ErrorKind::Unsupported))?;

        Ok(update_result)
    }
}
impl Update for FanFicFare {
    fn new() -> Self {
        Self {}
    }
    fn update(&self, path: &Path) -> UpdateResult {
        Self::do_update(path).unwrap_or(UpdateResult::NotSupported)
    }
}
