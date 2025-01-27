use crate::updater::native::book::Book;

pub type RoyalRoad = Book;

impl RoyalRoad {
    pub fn new(fiction_url: &str) -> Option<Self> {
        if fiction_url.starts_with("https://www.royalroad.com/fiction/") {
            Self::fetch_without_chapter_content(fiction_url).ok()
        } else {
            None
        }
    }
}
