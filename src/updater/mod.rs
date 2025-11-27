pub mod native;

use epub::doc::EpubDoc;
use eyre::{Error, Result};
use std::path::Path;

use crate::updater::native::book::Book;

#[derive(Debug)]
pub enum UpdateResult {
    Unsupported,
    UpToDate,
    Updated(u16),
    #[cfg(feature = "fanficfare")]
    Skipped,
    #[cfg(feature = "fanficfare")]
    MoreChapterThanSource(u16),
    Error(Error),
}

type DisplayName = String;

pub trait Download {
    fn get_title(&self, path: &Path) -> String {
        EpubDoc::new(path)
            .ok()
            .and_then(|e| e.mdata("title"))
            .unwrap_or_else(|| format!("{} (No Title)", path.to_string_lossy()))
    }

    fn already_up_to_date(&self, _current_book: Option<&Book>) -> bool {
        false
    }

    fn create(&self, dir: &Path, filename: Option<&str>, url: &str) -> Result<DisplayName>;
    fn update(&self, path: &Path) -> UpdateResult;
}
