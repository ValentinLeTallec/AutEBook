use epub::doc::EpubDoc;
use rss::Channel;
use std::error::Error;
use std::fmt::Debug;
use std::fs;
use std::{path::Path, process::Command};

use crate::source::*;

// mod source;

// #[derive(Debug)]
pub struct Book<'a, T: Syndication + FanFicFare> {
    path: &'a Path,
    source: T,
    rss_url: String,
}

// impl Debug for Book {
//     fn
// }

impl<'a> Book<'a> {
    pub fn new<P: 'a + AsRef<Path>>(path: P) -> Book<'a> {
        let id: u16 = 52639;
        let url = get_source(path);

        // Book {
        //     path: Box::new(path),
        //     id,
        //     rss_url: String::new(),
        // }
        todo!();
    }

    pub fn update(self) {
        let output = Command::new("echo")
            .arg("Hello world")
            .output()
            .expect("Failed to execute command");

        assert_eq!(b"Hello world\n", output.stdout.as_slice());
        todo!();
    }

    pub fn has_new_chapters(self) -> bool {
        todo!();
    }
}

fn get_source<P: AsRef<Path>>(path: P) -> Option<String> {
    EpubDoc::new(path).ok()?.mdata("source")
}

//   https://www.royalroad.com/fiction/36049/the-primal-hunter
//  https://www.royalroad.com/fiction/syndication/36049
// Source ->  https://www.royalroad.com/fiction/36049

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_book() {
        let path = Path::new("./tests/ressource/Zogarth - The Primal Hunter.epub");
        let book = Book::new(path);
        assert_eq!(book.source.get_syndication_url(), 52639);
    }

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
