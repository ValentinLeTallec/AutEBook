use crate::{cache::Cache, epub::Book};

pub struct RoyalRoadApi;

impl RoyalRoadApi {
    pub fn new() -> Self {
        Self
    }
    pub fn get_book(&self, id: u32, ignore_cache: bool) -> eyre::Result<Book> {
        // Do the initial metadata fetch of the book.
        let mut book = Book::new(id)?;

        // Update the cover.
        tracing::info!("Updating cover.");
        book.update_cover()?;

        // Check the cache.
        let cached = Cache::read_book(id)?;
        match cached {
            Some(mut cached) => {
                // Compare cached and fetched to see if any chapters are out-of-date.
                let mut should_update = ignore_cache;
                for chapter in book.chapters.iter() {
                    if let Some(cached) = cached.chapters.iter().find(|c| c.url == chapter.url) {
                        if cached.date != chapter.date {
                            should_update = true;
                            break;
                        }
                    } else {
                        should_update = true;
                        break;
                    }
                }

                if should_update {
                    // There is at least one out-of-date chapter, update the chapters.
                    book.update_chapter_content()?;

                    // Save back to cache.
                    Cache::write_book(&book)?;

                    Ok(book)
                } else {
                    // Just update the cover URL and resave to cache.
                    cached.cover_url = book.cover_url;
                    Cache::write_book(&cached)?;

                    Ok(cached)
                }
            }
            None => {
                // Load book chapters.
                book.update_chapter_content()?;

                // Write book to cache.
                Cache::write_book(&book)?;

                // Return book.
                Ok(book)
            }
        }
    }
}
