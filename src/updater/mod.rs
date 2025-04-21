pub mod native;

use epub::doc::EpubDoc;
use eyre::{bail, Error, Result};
use std::path::Path;

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

    fn create(&self, _dir: &Path, _filename: Option<&str>, _url: &str) -> Result<DisplayName> {
        bail!("This webnovel does not contain a supported source URL")
    }

    fn update(&self, path: &Path) -> UpdateResult {
        self.do_update(path).unwrap_or_else(UpdateResult::Error)
    }

    fn do_update(&self, _path: &Path) -> Result<UpdateResult> {
        Ok(UpdateResult::Unsupported)
    }
}
