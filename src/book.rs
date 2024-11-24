use crate::source;
use crate::updater::Unsupported;
use crate::updater::UpdateResult;
use crate::updater::WebNovel;

use epub::doc::EpubDoc;
// use rss::Channel;
use color_eyre::Result;
use std::fmt::{Debug, Formatter};
use std::path::Path;

pub struct Book {
    pub title: String,
    pub path: Box<Path>,
    url: String,
    updater: Option<Box<dyn WebNovel>>,
}

impl Book {
    fn get_book_title(path: &Path) -> Option<String> {
        EpubDoc::new(path).ok()?.mdata("title")
    }
    fn get_book_url(path: &Path) -> Option<String> {
        EpubDoc::new(path).ok()?.mdata("source")
    }

    pub fn get_source(url: &str) -> Option<Box<dyn WebNovel>> {
        source::get(url).get_updater()
    }

    pub fn new(path: &Path) -> Self {
        let url = Self::get_book_url(path).unwrap_or_default();
        let source = source::get(&url);
        let title = Self::get_book_title(path).unwrap_or_else(|| String::from("Unknown Title"));
        Self {
            title,
            url,
            path: path.to_path_buf().into_boxed_path(),
            updater: source.get_updater(),
        }
    }

    pub fn update(&self) -> UpdateResult {
        self.updater
            .as_ref()
            .map_or(UpdateResult::Unsupported, |s| s.update(&self.path))
    }

    pub fn create(dir: &Path, url: &str) -> Result<Self> {
        Self::get_source(url).map_or(Err(Unsupported.into()), |s| s.create(dir, None, url))
    }

    pub fn stash_and_recreate(&self, stash_dir: &Path) -> Result<Self> {
        self.updater.as_ref().map_or(Err(Unsupported.into()), |s| {
            s.stash_and_recreate(&self.path, stash_dir, &self.url)
        })
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
