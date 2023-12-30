use crate::epub::Book;

pub struct RoyalRoadApi;

impl RoyalRoadApi {
    pub fn new() -> Self {
        Self
    }
    pub async fn get_book(&self, id: u32, ignore_cache: bool) -> eyre::Result<Book> {
        // Do the initial metadata fetch of the book.
        let mut book = Book::new(id).await?;

        // Update the cover.
        tracing::info!("Updating cover.");
        book.update_cover().await?;

        // Check the cache.
        let cached = self.read_from_cache(id);
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

                // Always update the cover in the cache.
                cached.cover_url = book.cover_url;
                cached.cover = book.cover;

                if should_update {
                    // There is at least one out-of-date chapter, update the chapters.
                    cached.update_chapter_content().await?;

                    // Save back to cache.
                    self.save_to_cache(&cached)?;

                    Ok(cached)
                } else {
                    // Just return the cached book.
                    tracing::info!("Chapter content already up to date, not redownloading.");
                    Ok(cached)
                }
            }
            None => {
                // Load book chapters.
                book.update_chapter_content().await?;

                // Write book to cache.
                self.save_to_cache(&book)?;

                // Return book.
                Ok(book)
            }
        }
    }

    fn read_from_cache(&self, id: u32) -> Option<Book> {
        let home_dir = dirs::home_dir();
        let Some(home_dir) = home_dir.as_ref() else {
            return None;
        };
        let cache_dir = home_dir.join(".cache/rr-to-epub");
        let cache_file = cache_dir.join(format!("{}.json", id));
        if !cache_file.exists() {
            return None;
        }
        let contents = std::fs::read_to_string(cache_file);
        let Ok(contents) = contents else {
            return None;
        };
        let book: Result<Book, _> = serde_json::from_str(&contents);
        let Ok(book) = book else {
            return None;
        };
        Some(book)
    }
    fn save_to_cache(&self, book: &Book) -> eyre::Result<()> {
        let home_dir = dirs::home_dir().ok_or(eyre::eyre!("No home directory"))?;
        let cache_dir = home_dir.join(".cache/rr-to-epub");
        std::fs::create_dir_all(&cache_dir)?;

        // Write the book without the cover.
        let mut cloned_book = book.clone();
        cloned_book.cover = None;
        let cache_file = cache_dir.join(format!("{}.json", book.id));
        let contents = serde_json::to_string(&cloned_book)?;
        std::fs::write(cache_file, contents)?;

        Ok(())
    }
}
