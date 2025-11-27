use std::collections::HashSet;
use std::path::Path;

use crate::{get_progress_bar, updater::book::Chapter, ErrorPrint, MULTI_PROGRESS};
use ::epub::doc::EpubDoc;
use book::Book;
use eyre::{eyre, Error, Result};

pub mod book;
mod cache;
mod epub;
mod image;

#[derive(Debug)]
pub enum UpdateResult {
    Unsupported,
    UpToDate,
    Updated(u16),
    #[cfg(feature = "fanficfare")]
    Skipped,
    #[cfg(feature = "fanficfare")]
    MoreChapterThanSource(u16),
    Error(Error),
}

impl From<Result<Self>> for UpdateResult {
    fn from(value: Result<Self>) -> Self {
        match value {
            Ok(v) => v,
            Err(error) => Self::Error(error),
        }
    }
}

pub trait WebnovelProvider {
    fn get_title(&self, path: &Path) -> String {
        EpubDoc::new(path)
            .ok()
            .and_then(|e| e.mdata("title"))
            .unwrap_or_else(|| format!("{} (No Title)", path.to_string_lossy()))
    }
    fn create(&self, dir: &Path, filename: Option<&str>, url: &str) -> Result<String>;
    fn update(&self, path: &Path) -> UpdateResult;
}

pub trait WebnovelSource {
    fn get_title(&self, _path: &Path) -> String;

    fn get_url(&self) -> String;

    fn already_up_to_date(&self, _current_book: Option<&Book>) -> bool {
        false
    }

    fn fetch_without_chapter_content(&self) -> Result<Book>;

    fn update_chapter_content(&self, chapter: &mut Chapter) -> Result<()>;
}

impl<S: WebnovelSource> WebnovelProvider for S {
    fn get_title(&self, path: &Path) -> String {
        self.get_title(path)
    }

    fn create(&self, dir: &Path, filename: Option<&str>, url: &str) -> Result<String> {
        let outfile = filename
            .map(|f| dir.join(f))
            .map(|p| p.to_string_lossy().to_string());

        get_book(self, None)
            .and_then(|(book, _)| epub::write(&book, outfile).map(|()| book.title))
            .map_err(|e| eyre!("{e} for url {url}"))
    }

    fn update(&self, path: &Path) -> UpdateResult {
        // Check the cache.
        let current_book = Book::from_path(path).ok();
        if self.already_up_to_date(current_book.as_ref()) {
            return UpdateResult::UpToDate;
        }

        get_book(self, current_book)
            .and_then(|(book, result)| {
                if let UpdateResult::Updated(_) = result {
                    let outfile = path.to_str().map(String::from);
                    epub::write(&book, outfile).map(|()| result)
                } else {
                    Ok(result)
                }
            })
            .map_err(|e| eyre!("{e} for file {}", path.to_string_lossy()))
            .into()
    }
}

fn get_book<S: WebnovelSource + ?Sized>(
    webnovel_source: &S,
    current_book: Option<Book>,
) -> Result<(Book, UpdateResult)> {
    // Do the initial metadata fetch of the book.
    let mut fetched_book = webnovel_source
        .fetch_without_chapter_content()
        .inspect_err(|e| MULTI_PROGRESS.eprintln(e))?;

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
            if let Err(e) = webnovel_source.update_chapter_content(chapter) {
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
