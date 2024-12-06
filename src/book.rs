use crate::source;
use crate::updater::{Unsupported, UpdateResult, WebNovel};

use color_eyre::Result;
use epub::doc::EpubDoc;
use std::fmt::{Debug, Formatter};
use std::path::Path;

pub struct Book {
    pub title: String,
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
            updater: source.get_updater(),
        }
    }

    pub fn update(&self, file_path: &Path) -> UpdateResult {
        self.updater
            .as_ref()
            .map_or(UpdateResult::Unsupported, |s| s.update(file_path))
    }

    pub fn create(dir: &Path, url: &str) -> Result<Self> {
        Self::get_source(url).map_or(Err(Unsupported.into()), |s| s.create(dir, None, url))
    }

    pub fn stash_and_recreate(&self, file_path: &Path, stash_dir: &Path) -> Result<Self> {
        self.updater.as_ref().map_or(Err(Unsupported.into()), |s| {
            s.stash_and_recreate(file_path, stash_dir, &self.url)
        })
    }
}

impl Debug for Book {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "Book : {{ title: {}, source_is_recognized: {}}}",
            self.title,
            self.updater.is_some()
        )
    }
}
