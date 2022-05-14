use epub::doc::EpubDoc;
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
        let source_url = get_source(path);
        if let Some(source_url) = get_source(path) {
            Book {
                path,
                source: source::get(&source_url),
            }
        } else {
            Book { path, source: None }
        }
        // ma
    }

    pub fn update(&self) {
        let output = Command::new("echo")
            .arg("Hello world")
            .output()
            .expect("Failed to execute command");

        assert_eq!(b"Hello world\n", output.stdout.as_slice());
        todo!();
    }

    pub fn has_new_chapters(&self) -> bool {
        todo!();
    }
}

fn get_source(path: &Path) -> Option<String> {
    EpubDoc::new(path).ok()?.mdata("source")
}

//   https://www.royalroad.com/fiction/36049/the-primal-hunter
//  https://www.royalroad.com/fiction/syndication/36049
// Source ->  https://www.royalroad.com/fiction/36049

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_source_ok() {
        let path = Path::new("./tests/ressource/Zogarth - The Primal Hunter.epub");
        assert_eq!(
            Some(String::from("https://www.royalroad.com/fiction/36049")),
            get_source(path)
        );
    }

    #[test]
    fn test_get_source_ko() {
        assert_eq!(None, get_source(Path::new("./Cargo.toml")));
        assert_eq!(None, get_source(Path::new("./file_that_do_not_exist")));
    }
}
