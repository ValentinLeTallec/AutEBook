#![allow(unused)]

use epub::doc::EpubDoc;
use rss::Channel;
use std::error::Error;
use std::fmt::Debug;
use std::fs;
use std::{path::Path, process::Command};

mod book;
mod source;

const PATH: &str = "/home/valentin/Dropbox/Applications/Dropbox PocketBook";

async fn example_feed() -> Result<Channel, Box<dyn Error>> {
    let content = reqwest::get("http://example.com/feed.xml")
        .await?
        .bytes()
        .await?;
    let channel = Channel::read_from(&content[..])?;
    Ok(channel)
}

fn get_source(path: &Path) -> Option<String> {
    EpubDoc::new(path).ok()?.mdata("source")
}

fn main() {
    let paths = fs::read_dir(PATH).unwrap();
    // println!("{:?}", fs::read(PATH_TO_FILE).unwrap());

    for path in paths {
        // println!("Name: {}", path.unwrap().path().display());
        // let e = path.unwrap().path().display();
        // let e = get_source(path.unwrap().path());
        if let Some(s) = get_source(&path.unwrap().path()) {
            println!("{}", s)
        }
        // println!("{}", get_source(path.unwrap().path()).unwrap());
    }
    // println!("{}", paths.count())
}

fn post_action() {
    todo!("Remove empty files");
}
#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
