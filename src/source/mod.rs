#[cfg(feature = "fanficfare")]
mod fanficfare;
mod royalroad;
use std::path::Path;

use epub::doc::EpubDoc;
use eyre::{bail, Result};
use royalroad::RoyalRoad;

#[cfg(feature = "fanficfare")]
use crate::source::fanficfare::FanFicFare;
use crate::updater::{Download, UpdateResult};

macro_rules! try_source {
    ($book_source:ident, $url:expr) => {{
        if let Some(fiction) = $book_source::new($url) {
            return Box::new(fiction);
        }
    }};
}

pub fn from_url(url: &str) -> Box<dyn Download> {
    try_source!(RoyalRoad, url);
    #[cfg(feature = "fanficfare")]
    try_source!(FanFicFare, url);
    Box::new(Unsupported::from_url(url))
}

#[expect(clippy::map_unwrap_or)]
pub fn from_path(path: &Path) -> Box<dyn Download> {
    get_url(path)
        .map(|url| from_url(&url))
        .unwrap_or_else(|| Box::new(Unsupported::from_path(path)))
}

pub fn get_url(path: &Path) -> Option<String> {
    EpubDoc::new(path).ok().and_then(|e| e.mdata("source"))
}

pub struct Unsupported {
    message: String,
}

impl Unsupported {
    fn from_url(url: &str) -> Self {
        Self {
            message: format!("Unsupported url ({url})"),
        }
    }

    fn from_path(path: &Path) -> Self {
        Self {
            message: format!(
                "Path does not lead to an e-book with a supported url ({})",
                path.to_string_lossy()
            ),
        }
    }
}

impl Download for Unsupported {
    fn get_title(&self, _path: &Path) -> String {
        self.message.clone()
    }

    fn create(&self, _dir: &Path, _filename: Option<&str>, _url: &str) -> Result<String> {
        bail!("This webnovel does not contain a supported source URL")
    }

    fn update(&self, _path: &Path) -> UpdateResult {
        UpdateResult::Unsupported
    }
}
