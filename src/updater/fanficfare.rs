use crate::updater::Update;
use crate::updater::UpdateResult;

// use rss::Channel;
use lazy_regex::regex;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

pub struct FanFicFare;

impl FanFicFare {
    fn do_update(path: &Path) -> Option<UpdateResult> {
        let updating = regex!(r"^Updating .*, URL: .*$");
        let up_to_date = regex!(r"^.* already contains \d+ chapters\.$");
        let do_update = regex!(r"^Do update - epub\((\d+)\) vs url\((\d+)\)$");
        let more_chapter_than_source =
            regex!(r"^.* contains (\d+) chapters, more than source: (\d+)\.$");
        let skipped = " - Skipping";

        let cmd = Command::new("fanficfare")
            .arg("--non-interactive")
            .arg("--update-epub")
            .arg("--update-cover")
            // .arg("--no-output") // TODO : remove line
            .arg(path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .ok()?;

        let stdout = cmd.stdout?;
        let stderr = cmd.stderr?;
        let update_result = BufReader::new(stderr)
            .lines()
            .chain(BufReader::new(stdout).lines())
            .map_while(Result::ok)
            .filter(|line| updating.captures(line).is_none())
            .find_map(|line| {
                if up_to_date.captures(&line).is_some() {
                    return Some(UpdateResult::UpToDate);
                }
                if let Some(c) = do_update.captures(&line) {
                    let nb_chapter_epub = &c[1].parse::<u16>().ok()?;
                    let nb_chapter_url = &c[2].parse::<u16>().ok()?;
                    return Some(UpdateResult::Updated(nb_chapter_url - nb_chapter_epub));
                }
                if let Some(c) = more_chapter_than_source.captures(&line) {
                    let nb_chapter_epub = &c[1].parse::<u16>().ok()?;
                    let nb_chapter_url = &c[2].parse::<u16>().ok()?;
                    return Some(UpdateResult::MoreChapterThanSource(
                        nb_chapter_epub - nb_chapter_url,
                    ));
                }
                if line.ends_with(skipped) {
                    return Some(UpdateResult::Skipped);
                }
                None
            })?;

        Some(update_result)
    }
}
impl Update for FanFicFare {
    fn new() -> Self {
        Self {}
    }
    fn update(&self, path: &Path) -> UpdateResult {
        Self::do_update(path).unwrap_or(UpdateResult::Unsupported)
    }
}
