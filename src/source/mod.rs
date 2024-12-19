mod royalroad;
use std::path::Path;

use epub::doc::EpubDoc;
use royalroad::RoyalRoad;

use crate::updater::Download;
#[cfg(feature = "fanficfare")]
use crate::updater::FanFicFare;

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

#[allow(clippy::map_unwrap_or)]
pub fn from_path(path: &Path) -> Box<dyn Download> {
    EpubDoc::new(path)
        .ok()
        .and_then(|e| e.mdata("source"))
        .map(|url| from_url(&url))
        .unwrap_or_else(|| Box::new(Unsupported::from_path(path)))
}

pub struct Unsupported {
    url: Option<String>,
    message: String,
}

impl Unsupported {
    fn from_url(url: &str) -> Self {
        Self {
            url: Some(url.to_string()),
            message: format!("Unsupported url ({url})"),
        }
    }

    fn from_path(path: &Path) -> Self {
        Self {
            url: None,
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

    fn get_url(&self) -> String {
        self.url.clone().unwrap_or_default()
    }
}
