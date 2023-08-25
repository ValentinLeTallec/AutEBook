mod fanficfare;
use color_eyre::eyre::eyre;
use color_eyre::Result;
pub use fanficfare::FanFicFare;
use std::{fs, path::Path};
use thiserror::Error;

use crate::book::Book;

#[derive(Debug)]
pub enum UpdateResult {
    Unsupported,
    UpToDate,
    Updated(u16),
    Skipped,
    MoreChapterThanSource(u16),
}

#[derive(Error, Debug)]
#[error("This webnovel does not contain a supported source URL")]
pub struct Unsupported;

pub trait WebNovel {
    fn new() -> Self
    where
        Self: Sized;

    #[allow(unused_variables)]
    fn create(&self, path: &Path, url: &str) -> Result<Book> {
        Err(Unsupported.into())
    }
    #[allow(unused_variables)]
    fn update(&self, path: &Path) -> UpdateResult {
        UpdateResult::Unsupported
    }

    fn stash_and_recreate(&self, book: &Path, stash_folder: &Path, url: &str) -> Result<Book> {
        let parent_dir = book
            .parent()
            .ok_or_else(|| eyre!("Could not retrieve the book's parent directory."))?;

        // Stashing of the current instance of the book in an sub-directory
        let timestamp = chrono::Utc::now().format("_%Y-%m-%d_%Hh%M").to_string();
        let extension = book
            .extension()
            .ok_or_else(|| eyre!("Could not retrieve the book's extension."))?;

        let mut filename = book
            .file_stem()
            .ok_or_else(|| eyre!("Could not retrieve the book's filename."))?
            .to_owned();
        filename.push(timestamp);
        filename.push(".");
        filename.push(extension);

        fs::create_dir_all(stash_folder)?;
        fs::rename(book, stash_folder.join(filename))?;

        // Creation of the new instance of the book
        self.create(parent_dir, url)
    }
}
