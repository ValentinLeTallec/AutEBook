use crate::source;
use crate::updater::Update;
use crate::updater::UpdateResult;

use epub::doc::EpubDoc;
// use rss::Channel;
use std::fmt::{Debug, Formatter};
use std::path::Path;

pub struct Book {
    name: String,
    path: Box<Path>,
    updater: Option<Box<dyn Update>>,
}

pub struct BookResult {
    pub name: String,
    pub result: UpdateResult,
}

impl Book {
    fn get_book_name(path: &Path) -> Option<String> {
        EpubDoc::new(path).ok()?.mdata("title")
    }
    fn get_book_url(path: &Path) -> Option<String> {
        EpubDoc::new(path).ok()?.mdata("source")
    }

    pub fn new(path: &Path) -> Self {
        let book_url = Self::get_book_url(path).unwrap_or_default();
        let source = source::get(&book_url);
        Self {
            name: Self::get_book_name(path).unwrap_or_else(|| String::from("Unknown Title")),
            path: path.to_path_buf().into_boxed_path(),
            updater: source.get_updater(),
        }
    }

    pub fn update(&self) -> BookResult {
        let mut short_name = self.name.clone();
        short_name.truncate(50);
        BookResult {
            name: short_name,
            result: self
                .updater
                .as_ref()
                .map_or(UpdateResult::Unsupported, |s| s.update(&self.path)),
        }
    }

    // async fn example_feed() -> Result<Channel, Box<dyn Error>> {
    //     let content = reqwest::get("http://example.com/feed.xml")
    //         .await?
    //         .bytes()
    //         .await?;
    //     let channel = Channel::read_from(&content[..])?;
    //     Ok(channel)
    // }
}

impl Debug for Book {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "Book : {{ path: {}, source_is_recognized: {}}}",
            self.path.display(),
            self.updater.is_some()
        )
    }
}
