mod fanficfare;
use color_eyre::Result;
pub use fanficfare::FanFicFare;
use std::path::Path;
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

    fn create(&self, path: &Path, url: &str) -> Result<Book> {
        Err(Unsupported.into())
    }
    fn update(&self, path: &Path) -> UpdateResult {
        UpdateResult::Unsupported
    }
}
