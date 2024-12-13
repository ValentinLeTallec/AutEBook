use std::path::PathBuf;

use bytes::Bytes;
use eyre::eyre;

use crate::updater::native::epub::Book;

pub struct Cache;
impl Cache {
    fn cache_path() -> eyre::Result<PathBuf> {
        let home_dir = dirs::home_dir().ok_or_else(|| eyre!("No home directory"))?;
        let cache_dir = home_dir.join(".cache/rr-to-epub");
        std::fs::create_dir_all(&cache_dir)?;
        Ok(cache_dir)
    }

    pub fn write_inline_image(book: &Book, filename: &str, image: &[u8]) -> eyre::Result<()> {
        let cache_dir = Self::cache_path()?.join(book.id.to_string());
        std::fs::create_dir_all(&cache_dir)?;

        // Write the image to the cache.
        let cache_file = cache_dir.join(filename);
        std::fs::write(cache_file, image)?;
        Ok(())
    }

    pub fn read_inline_image(book: &Book, filename: &str) -> eyre::Result<Option<Bytes>> {
        let cache_dir = Self::cache_path()?;
        let cache_file = cache_dir.join(book.id.to_string()).join(filename);
        if !cache_file.exists() {
            return Ok(None);
        }
        let contents = std::fs::read(cache_file)?;
        Ok(Some(contents.into()))
    }
}
