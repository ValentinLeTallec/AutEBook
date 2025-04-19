use std::path::Path;
use std::{collections::HashSet, ffi::OsStr};

use crate::{get_progress_bar, ErrorPrint, MULTI_PROGRESS};
use ::epub::doc::EpubDoc;
use book::Book;
use eyre::{eyre, OptionExt, Result};

use super::{Download, UpdateResult};

pub mod book;
mod cache;
mod epub;
mod image;
mod request;

impl Download for Book {
    fn get_title(&self, _path: &Path) -> String {
        self.title.clone()
    }

    #[cfg(feature = "fanficfare")]
    fn get_url(&self) -> String {
        self.url.clone()
    }

    fn create(&self, dir: &Path, filename: Option<&OsStr>, url: &str) -> Result<String> {
        let outfile = filename
            .and_then(|f| f.to_str())
            .map(|f| dir.join(f))
            .map(|p| p.to_string_lossy().to_string());

        get_book(url, None)
            .and_then(|(book, _)| epub::write(&book, outfile).map(|()| book.title))
            .map_err(|e| eyre!("{e} for url {url}"))
    }

    fn update(&self, path: &Path) -> UpdateResult {
        EpubDoc::new(path)
            .map_err(|e| eyre!("{e}"))
            .and_then(|e| e.mdata("source").ok_or_eyre("Could not find url"))
            .and_then(|url| get_book(&url, Some(path)))
            .and_then(|(book, result)| {
                if let UpdateResult::Updated(_) = result {
                    let outfile = path.to_str().map(String::from);
                    epub::write(&book, outfile).map(|()| result)
                } else {
                    Ok(result)
                }
            })
            .map_err(|e| eyre!("{e} for file {}", path.to_string_lossy()))
            .unwrap_or_else(UpdateResult::Error)
    }
}

fn get_book(url: &str, path: Option<&Path>) -> Result<(Book, UpdateResult)> {
    // Do the initial metadata fetch of the book.
    let mut fetched_book = Book::fetch_without_chapter_content(url)?;

    // Check the cache.
    let mut current_book = path
        .and_then(|path| Book::from_path(path).ok())
        .unwrap_or_else(|| fetched_book.clone_without_chapters());

    // Determine chapters which already exist but have been updated
    // (same identifier, newer date_published)
    let mut chapter_to_update_ids: HashSet<_> = fetched_book
        .chapters
        .iter()
        .filter(|fetched| {
            current_book.chapters.iter().any(|current| {
                current.identifier.eq(&fetched.identifier)
                    && fetched.date_published > current.date_published
            })
        })
        .map(|c| c.identifier.clone())
        .collect();

    // Determine new chapters
    fetched_book
        .chapters
        .retain(|e| !current_book.chapters.contains(e));

    for c in &fetched_book.chapters {
        chapter_to_update_ids.insert(c.identifier.clone());
    }

    // Add new chapters to the current book
    current_book.chapters.append(&mut fetched_book.chapters);

    let nb_new_chapter = u16::try_from(chapter_to_update_ids.len()).map_err(|_| {
        eyre!("There is way too many new chapters (more than 50_000), something probably got wrong")
    })?;

    let bar = MULTI_PROGRESS.add(get_progress_bar(nb_new_chapter.into(), 5));
    bar.set_prefix(current_book.title.clone());

    // Update them in the current book
    current_book
        .chapters
        .iter_mut()
        .filter(|c| chapter_to_update_ids.contains(&c.identifier))
        .for_each(|chapter| {
            if let Err(e) = chapter.update_chapter_content() {
                bar.eprintln(&eyre!(
                    "Could not download chapter '{}' : {}",
                    chapter.title,
                    e
                ));
            };
            bar.inc(1);
        });
    bar.finish_and_clear();

    // Remove empty chapters
    current_book.chapters.retain(|c| c.content.is_some());

    // Update the cover URL and resave to cache.
    current_book.cover_url = fetched_book.cover_url;

    Ok((
        current_book,
        if nb_new_chapter > 0 {
            UpdateResult::Updated(nb_new_chapter)
        } else {
            UpdateResult::UpToDate
        },
    ))
}
