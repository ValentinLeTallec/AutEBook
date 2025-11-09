use crate::{updater::native::book::Book, ErrorPrint, MULTI_PROGRESS};

pub type RoyalRoad = Book;

impl RoyalRoad {
    pub fn new(fiction_url: &str) -> Option<Self> {
        if fiction_url.starts_with("https://www.royalroad.com/fiction/") {
            // Do the initial metadata fetch of the book.
            Self::fetch_without_chapter_content(fiction_url)
                .inspect_err(|e| MULTI_PROGRESS.eprintln(e))
                .ok()
        } else {
            None
        }
    }
}
