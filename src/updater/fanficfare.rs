use crate::book::Book;
use crate::updater::UpdateResult;
use crate::updater::WebNovel;

// use rss::Channel;
use color_eyre::eyre::{bail, eyre};
use color_eyre::Result;
use lazy_regex::regex;
use serde::Deserialize;
use std::ffi::OsStr;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Deserialize)]
struct FanFicFareJson {
    output_filename: String,
}

pub struct FanFicFare;

impl WebNovel for FanFicFare {
    fn new() -> Self {
        Self {}
    }
    fn create(&self, dir: &Path, filename: Option<&OsStr>, url: &str) -> Result<Book> {
        let cmd = Command::new("fanficfare")
            .arg("--non-interactive")
            .arg("--json-meta")
            .arg(url)
            .current_dir(dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // Retrieve the metadata of the newly created book
        let stdout = cmd.stdout.ok_or_else(|| eyre!("Stdout is unavailable"))?;
        let book_metadata = BufReader::new(stdout)
            .lines()
            .map_while(Result::ok)
            .reduce(|accum, line| accum + &line)
            .ok_or_else(|| eyre!("Failed to read book metadata."))?;

        let generated_filename =
            serde_json::from_str::<FanFicFareJson>(&book_metadata).map(|e| e.output_filename)?;

        // Manage error cases
        let err_lines: String = cmd.stderr.map_or(String::new(), |stderr| {
            BufReader::new(stderr)
                .lines()
                .map_while(Result::ok)
                .collect()
        });

        if !err_lines.is_empty() {
            bail!("The execution of Fanficfare for '{url}'' ended with an error \n{err_lines}");
        }

        let mut file_path = dir.join(generated_filename);
        if let Some(filename) = filename {
            let new_file_path = dir.join(filename);
            fs::rename(file_path, &new_file_path)?;
            file_path = new_file_path;
        }

        Ok(Book::new(&file_path))
    }

    fn update(&self, path: &Path) -> UpdateResult {
        do_update(path).unwrap_or(UpdateResult::Unsupported)
    }
}

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
