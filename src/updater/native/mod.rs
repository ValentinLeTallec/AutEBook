use std::path::Path;
use std::{collections::HashSet, ffi::OsStr};

use ::epub::doc::EpubDoc;
use cache::Cache;
use epub::Book;
use eyre::{eyre, OptionExt, Result};
use url::Url;

use crate::{get_progress_bar, MULTI_PROGRESS};

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

        let (book, _) = get_book(id)?;
        let outfile = epub::write(&book, filename.and_then(|f| f.to_str()).map(String::from))?;

        let file_path = dir.join(outfile);
        Ok(crate::Book::new(&file_path))
    }

    fn update(&self, path: &Path) -> UpdateResult {
        do_update(path).unwrap_or_else(UpdateResult::Error)
    }
}

fn get_book(id: u32) -> eyre::Result<(Book, UpdateResult)> {
    // Do the initial metadata fetch of the book.
    let mut fetched_book = Book::new(id)?;

    // Check the cache.
    let mut current_book =
        Cache::read_book(id)?.unwrap_or_else(|| fetched_book.clone_without_chapters());

    // Remove existing, non-updated chapters
    fetched_book
        .chapters
        .retain(|e| !current_book.chapters.contains(e));

    let nb_new_chapter = u16::try_from(fetched_book.chapters.len()).map_err(|_| {
        eyre!("There is way too many new chapters (more than 50_000), something probably got wrong")
    })?;
    let bar = MULTI_PROGRESS.add(get_progress_bar(nb_new_chapter.into(), 5));
    bar.set_prefix(current_book.title.clone());

    // Dertermine chapters which already exist but have been updated
    // (same identifier, different date_published]
    let updated_chapter_ids: HashSet<_> = fetched_book
        .chapters
        .iter()
        .filter(|c| {
            current_book
                .chapters
                .iter()
                .any(|e| e.identifier.eq(&c.identifier))
        })
        .map(|c| c.identifier.clone())
        .collect();

    // Update them in the current book
    current_book
        .chapters
        .iter_mut()
        .filter(|c| updated_chapter_ids.contains(&c.identifier))
        .for_each(|c| {
            if let Err(e) = c.update_chapter_content() {
                tracing::warn!("Could not download chapter '{}' : {}", c.title, e);
            };
            bar.inc(1);
        });

    // Remove those updated chapters, leaving only new chapters
    fetched_book
        .chapters
        .retain(|c| !updated_chapter_ids.contains(&c.identifier));

    // Proceed to deal with new chapters
    fetched_book
        .chapters
        .iter_mut()
        .enumerate()
        .for_each(|(index, chapter)| {
            tracing::info!(
                "Downloading chapter '{}' ({} of {})",
                chapter.title,
                index + 1,
                nb_new_chapter
            );
            if let Err(e) = chapter.update_chapter_content() {
                tracing::warn!("Could not download chapter '{}' : {}", chapter.title, e);
            };
            bar.inc(1);
        });
    bar.finish();

    current_book.chapters.append(&mut fetched_book.chapters);

    // Update the cover URL and resave to cache.
    current_book.cover_url = fetched_book.cover_url;
    Cache::write_book(&current_book)?;

    Ok((
        current_book,
        if nb_new_chapter > 0 {
            UpdateResult::Updated(nb_new_chapter)
        } else {
            UpdateResult::UpToDate
        },
    ))
}

fn do_update(path: &Path) -> eyre::Result<UpdateResult> {
    let url = EpubDoc::new(path)?
        .mdata("source")
        .ok_or_eyre("Could not find url")?;
    let url = Url::parse(&url)?;
    let id = get_id_from_url(&url)?;

    let (book, result) = get_book(id)?;
    epub::write(&book, path.to_str().map(String::from))?;
    Ok(result)
}

fn get_id_from_url(url: &Url) -> Result<u32, eyre::Error> {
    let id = url
        .path_segments()
        .and_then(|mut s| s.nth(1))
        .and_then(|f| f.parse().ok())
        .ok_or_else(|| eyre!("Invalid book URL: {url}"))?;
    Ok(id)
}
