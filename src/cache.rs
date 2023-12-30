use std::path::PathBuf;

use bytes::Bytes;

use crate::epub::Book;

pub struct Cache;
impl Cache {
    fn cache_path() -> eyre::Result<PathBuf> {
        let home_dir = dirs::home_dir().ok_or(eyre::eyre!("No home directory"))?;
        let cache_dir = home_dir.join(".cache/rr-to-epub");
        std::fs::create_dir_all(&cache_dir)?;
        Ok(cache_dir)
    }
    pub fn write_book(book: &Book) -> eyre::Result<()> {
        let cache_dir = Self::cache_path()?.join(book.id.to_string());
        std::fs::create_dir_all(&cache_dir)?;

        // Write the book without the cover.
        let mut cloned_book = book.clone();
        cloned_book.cover = None;
        let cache_file = cache_dir.join("book.json");
        let contents = serde_json::to_string(&cloned_book)?;
        std::fs::write(cache_file, contents)?;
        Ok(())
    }
    pub fn read_book(id: u32) -> eyre::Result<Option<Book>> {
        let cache_dir = Self::cache_path()?;
        let cache_file = cache_dir.join(id.to_string()).join("book.json");
        if !cache_file.exists() {
            return Ok(None);
        }
        let contents = std::fs::read_to_string(cache_file)?;
        let book: Result<Book, _> = serde_json::from_str(&contents);
        let book = match book {
            Ok(book) => book,
            Err(err) => {
                tracing::error!("Failed to parse book from cache: {:?}", err);
                return Ok(None);
            }
        };
        Ok(Some(book))
    }
    pub fn write_inline_image(book: &Book, url: &str, image: &[u8]) -> eyre::Result<()> {
        let cache_dir = Self::cache_path()?.join(book.id.to_string()).join("images");
        std::fs::create_dir_all(&cache_dir)?;
        let filename = url.split('/').last().unwrap();

        // Write the image to the cache.
        let cache_file = cache_dir.join(filename);
        std::fs::write(cache_file, image)?;
        Ok(())
    }
    pub fn read_inline_image(book: &Book, url: &str) -> eyre::Result<Option<Bytes>> {
        let cache_dir = Self::cache_path()?;
        let filename = url.split('/').last().unwrap();
        let cache_file = cache_dir
            .join(book.id.to_string())
            .join("images")
            .join(filename);
        if !cache_file.exists() {
            return Ok(None);
        }
        let contents = std::fs::read(cache_file)?;
        Ok(Some(contents.into()))
    }
}
