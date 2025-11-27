use std::collections::HashSet;
use std::path::Path;

use crate::{get_progress_bar, ErrorPrint, MULTI_PROGRESS};
use book::Book;
use eyre::{eyre, Result};

use super::{Download, UpdateResult};
use crate::source::royalroad::RoyalRoad;

pub mod book;
mod cache;
mod epub;
mod image;

impl Download for RoyalRoad {
    fn get_title(&self, _path: &Path) -> String {
        self.title.clone()
    }

    fn already_up_to_date(&self, current_book: Option<&Book>) -> bool {
        current_book.as_ref().is_some_and(|b| {
            b.chapters
                .iter()
                .map(|e| e.date_published)
                .max()
                .is_some_and(|max| max >= self.last_time_published)
        })
    }

    fn create(&self, dir: &Path, filename: Option<&str>, url: &str) -> Result<String> {
        let outfile = filename
            .map(|f| dir.join(f))
            .map(|p| p.to_string_lossy().to_string());

        get_book(url, None)
            .and_then(|(book, _)| epub::write(&book, outfile).map(|()| book.title))
            .map_err(|e| eyre!("{e} for url {url}"))
    }

    fn update(&self, path: &Path) -> UpdateResult {
        // Check the cache.
        let current_book = Book::from_path(path).ok();
        if self.already_up_to_date(current_book.as_ref()) {
            return UpdateResult::UpToDate;
        }

        get_book(&self.url, current_book)
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

fn get_book(url: &str, current_book: Option<Book>) -> Result<(Book, UpdateResult)> {
    // Do the initial metadata fetch of the book.
    let mut fetched_book =
        Book::fetch_without_chapter_content(url).inspect_err(|e| MULTI_PROGRESS.eprintln(e))?;

    let mut current_book = current_book.unwrap_or_else(|| fetched_book.clone_without_chapters());

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
            }
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
