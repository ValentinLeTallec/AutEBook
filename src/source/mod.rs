mod royalroad;
use epub::doc::EpubDoc;
use royalroad::RoyalRoad;
use std::path::Path;

pub trait Syndication {
    fn get_syndication_url(&self) -> String;
}

#[derive(Debug)]
pub enum UpdateResult {
    NotSupported,
    UpToDate,
    Updated(u16),
    MoreChapterThanSource(u16),
}

// impl UpdateResult {
//     fn nb_of_new_chapter(epub_nb: Option<&str>, url_nb: Option<&str>) -> Option<u16> {
//         Some(url_nb?.parse::<u16>().ok()? - epub_nb?.parse::<u16>().ok()?)
//     }
// }

pub trait FanFicFare {
    fn new(url: &str) -> Option<Self>
    where
        Self: Sized;
}

pub fn get(path: &Path) -> Option<Box<dyn FanFicFare>> {
    let url = &get_source_url_from_epub(path)?;
    if let Some(fiction) = RoyalRoad::new(url) {
        return Some(Box::new(fiction));
    }
    None
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
