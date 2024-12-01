use std::ffi::OsStr;
use std::path::Path;

use ::epub::doc::EpubDoc;
use cache::Cache;
use color_eyre::eyre::{self, eyre, Result};
use epub::Book;
use url::Url;

use super::{UpdateResult, WebNovel};

mod cache;
mod epub;
mod image;
mod xml_ext;

pub struct Native;

impl WebNovel for Native {
    fn new() -> Self {
        Self {}
    }
    fn create(&self, dir: &Path, filename: Option<&OsStr>, url: &str) -> Result<crate::Book> {
        let url = Url::parse(url)?;
        let id = get_id_from_url(&url)?;

        let (book, _) = get_book(id, false)?;
        let outfile = epub::write(&book, filename.and_then(|f| f.to_str()).map(String::from))?;

        let file_path = dir.join(outfile);
        Ok(crate::Book::new(&file_path))
    }

    fn update(&self, path: &Path) -> UpdateResult {
        do_update(path).unwrap_or(UpdateResult::Unsupported)
    }
}

fn get_book(id: u32, ignore_cache: bool) -> eyre::Result<(Book, UpdateResult)> {
    // Do the initial metadata fetch of the book.
    let mut book = Book::new(id)?;

    // Check the cache.
    let cached = Cache::read_book(id)?;
    if let Some(mut cached) = cached {
        // Compare cached and fetched to see if any chapters are out-of-date.
        let mut should_update = ignore_cache;
        for chapter in &book.chapters {
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

            #[allow(clippy::cast_possible_truncation)]
            let nb_new_chapter = (book.chapters.len() - cached.chapters.len()) as u16;

            // Save back to cache.
            Cache::write_book(&book)?;

            Ok((book, UpdateResult::Updated(nb_new_chapter)))
        } else {
            // Just update the cover URL and resave to cache.
            cached.cover_url = book.cover_url;
            Cache::write_book(&cached)?;

            Ok((cached, UpdateResult::UpToDate))
        }
    } else {
        // Load book chapters.
        book.update_chapter_content()?;

        // Write book to cache.
        Cache::write_book(&book)?;

        #[allow(clippy::cast_possible_truncation)]
        let nb_new_chapter = book.chapters.len() as u16;

        // Return book.
        Ok((book, UpdateResult::Updated(nb_new_chapter)))
    }
}

fn do_update(path: &Path) -> Option<UpdateResult> {
    let url = EpubDoc::new(path).ok()?.mdata("source")?;
    let url = Url::parse(&url).ok()?;
    let id = get_id_from_url(&url).ok()?;

    let (book, result) = get_book(id, false).ok()?;
    epub::write(&book, path.to_str().map(String::from)).ok()?;
    Some(result)
}

fn get_id_from_url(url: &Url) -> Result<u32, eyre::Error> {
    let id = url
        .path_segments()
        .and_then(|mut s| s.nth(1))
        .and_then(|f| f.parse().ok())
        .ok_or_else(|| eyre!(" Invalid book URL: {url}"))?;
    Ok(id)
}
