mod royalroad;
use crate::updater::Update;
use epub::doc::EpubDoc;
use std::path::Path;

use self::royalroad::RoyalRoad;

pub trait Source {
    fn new(url: &str) -> Option<Self>
    where
        Self: Sized;
    fn get_updater(&self) -> Option<Box<dyn Update>> {
        None
    }
    fn get_syndication_url(&self) -> Option<String> {
        None
    }
}

pub struct UnsupportedSource {}
impl Source for UnsupportedSource {
    fn new(_url: &str) -> Option<Self> {
        None
    }
}

pub fn get(path: &Path) -> Box<dyn Source> {
    if let Some(url) = &get_source_url_from_epub(path) {
        if let Some(fiction) = RoyalRoad::new(url) {
            return Box::new(fiction);
        }
    }
    Box::new(UnsupportedSource {})
}

fn get_source_url_from_epub(path: &Path) -> Option<String> {
    EpubDoc::new(path).ok()?.mdata("source")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_source_url_from_epub_ok() {
        let path = Path::new("./tests/ressources/Zogarth - The Primal Hunter.epub");
        assert_eq!(
            Some(String::from("https://www.royalroad.com/fiction/36049")),
            get_source_url_from_epub(path)
        );
    }

    #[test]
    fn test_get_source_url_from_epub_ko() {
        assert_eq!(None, get_source_url_from_epub(Path::new("./Cargo.toml")));
        assert_eq!(
            None,
            get_source_url_from_epub(Path::new("./file_that_do_not_exist"))
        );
    }
}
