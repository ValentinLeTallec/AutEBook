use super::cache::Cache;
use super::image;
use crate::parsing_utils::QuickSelect;
use crate::{lazy_selectors, request};
use crate::{ErrorPrint, MULTI_PROGRESS};

use chrono::{DateTime, Utc};
use derive_more::derive::Debug;
use epub::doc::EpubDoc;
use eyre::{eyre, Result};
use scraper::Html;
use std::path::Path;
use url::Url;
use uuid::Uuid;

lazy_selectors! {
    EPUB_CHAPTER_TITLE_SELECTOR: "title";

    EPUB_META_CHAPTER_URL_SELECTOR: "meta[name=chapterurl]";
    EPUB_META_DATE_PUBLISHED_SELECTOR: "meta[name=published]";
    EPUB_META_GENERATOR_SELECTOR: "meta[name=generator]";

    EPUB_CHAPTER_CONTENT_SELECTOR: ".chapter-content";
    EPUB_AUTHORS_NOTE_START_SELECTOR: ".authors-note-start";
    EPUB_AUTHORS_NOTE_END_SELECTOR: ".authors-note-end";

    EPUB_FANFICFARE_CHAPTER_CONTENT_SELECTOR: "body";
    EPUB_FANFICFARE_AUTHORS_NOTE_SELECTOR: ".author-note-portlet";
}

#[derive(Default, Clone, Debug)]
pub struct Book {
    pub id: String,
    pub url: String,
    pub title: String,
    pub author: String,
    #[debug("{description:50?}")]
    pub description: String,
    pub date_published: DateTime<Utc>,
    pub cover_url: String,
    pub chapters: Vec<Chapter>,
}

impl Book {
    pub fn from_path(path: &Path) -> Result<Self> {
        let now = chrono::Utc::now();
        let mut epub_doc = EpubDoc::new(path)?;
        let url = epub_doc.mdata("source").unwrap_or_default();
        let mut book = Self {
            id: epub_doc
                .mdata("identifier")
                .unwrap_or_else(|| Uuid::new_v4().to_string()),
            url,
            title: epub_doc.mdata("title").unwrap_or_default(),
            author: epub_doc.mdata("creator").unwrap_or_default(),
            description: epub_doc.mdata("description").unwrap_or_default(),
            date_published: epub_doc
                .mdata("date")
                .and_then(|d| DateTime::parse_from_rfc3339(&d).ok())
                .map(|d| d.to_utc())
                .unwrap_or(now),
            cover_url: String::new(),
            chapters: Vec::new(),
        };

        let image_filenames_and_ids: Vec<_> = epub_doc
            .resources
            .iter()
            .filter(|(_id, (_path, mime))| mime.starts_with("image"))
            .filter_map(|(id, (path, _mime))| {
                path.file_name()
                    .map(|p| p.to_string_lossy().to_string())
                    .map(|p| (id.clone(), p))
            })
            .collect();

        image_filenames_and_ids
            .iter()
            .filter_map(|(id, filename)| epub_doc.get_resource(id).map(|(i, _)| (filename, i)))
            .for_each(|(filename, image)| {
                if let Err(e) = Cache::write_inline_image(&book, filename, &image) {
                    MULTI_PROGRESS.eprintln(&e);
                }
            });

        while epub_doc.go_next() {
            let identifier = epub_doc
                .get_current_id()
                .map(|s| s.replace(".xhtml", ""))
                .unwrap_or_default();

            if identifier == "nav" {
                continue;
            }

            let xhtml = epub_doc
                .get_current_str()
                .map(|(content, _mime)| content)
                .unwrap_or_default();

            book.chapters
                .push(Chapter::extract_from_epub(&identifier, &xhtml, now));
        }
        Ok(book)
    }

    pub fn clone_without_chapters(&self) -> Self {
        Self {
            id: self.id.clone(),
            url: self.url.clone(),
            title: self.title.clone(),
            author: self.author.clone(),
            description: self.description.clone(),
            date_published: self.date_published.clone(),
            cover_url: self.cover_url.clone(),
            chapters: Vec::new(),
        }
    }

    pub fn download_image(&self, url: &str, filename: &str) -> Result<Vec<u8>> {
        // If the image is in the cache, directly use it.
        if let Some(image) = Cache::read_inline_image(self, filename)? {
            return Ok(image.into());
        }

        let image = request::get_bytes(url)?;

        let buffer = image::resize(image).map_err(|err| eyre!("{err} URL: {url}"))?;

        // Save the image in the cache.
        Cache::write_inline_image(self, filename, &buffer)?;

        Ok(buffer)
    }
}

#[derive(Default, Clone, Debug)]
pub struct Chapter {
    pub identifier: String,
    pub date_published: DateTime<Utc>,
    pub title: String,
    pub url: String,

    #[debug("{:?}", content.as_ref().map(|s| format!("{s:.100}")))]
    pub content: Option<String>,
    #[debug("{:?}", authors_note_start.as_ref().map(|s| format!("{s:.100}")))]
    pub authors_note_start: Option<String>,
    #[debug("{:?}", authors_note_end.as_ref().map(|s| format!("{s:.100}")))]
    pub authors_note_end: Option<String>,
}

impl PartialEq for Chapter {
    fn eq(&self, other: &Self) -> bool {
        self.identifier.eq(&other.identifier)
    }
}
impl Eq for Chapter {}
impl Chapter {
    pub fn extract_from_epub(file_identifier: &str, xhtml: &str, now: DateTime<Utc>) -> Self {
        let parsed = Html::parse_document(xhtml);

        let title = parsed
            .get_inner_html_of(&EPUB_CHAPTER_TITLE_SELECTOR)
            .unwrap_or_default();

        let url = parsed
            .get_meta_content_of(&EPUB_META_CHAPTER_URL_SELECTOR)
            .unwrap_or_default();

        let date_published = parsed
            .get_meta_content_of(&EPUB_META_DATE_PUBLISHED_SELECTOR)
            .and_then(|d| DateTime::parse_from_rfc3339(&d).ok())
            .map(|d| d.to_utc())
            .unwrap_or(now);

        let identifier: String = Url::parse(&url)
            .ok()
            .and_then(|url| {
                url.path_segments()
                    .and_then(|mut x| x.nth(4).map(ToString::to_string))
            })
            .unwrap_or_else(|| file_identifier.to_string());

        let was_generated_with_native_updater = parsed
            .get_meta_content_of(&EPUB_META_GENERATOR_SELECTOR)
            .is_some_and(|e| e == "autebook");

        let (content, authors_note_start, authors_note_end) = if was_generated_with_native_updater {
            (
                parsed.get_inner_html_of(&EPUB_CHAPTER_CONTENT_SELECTOR),
                parsed.get_inner_html_of(&EPUB_AUTHORS_NOTE_START_SELECTOR),
                parsed.get_inner_html_of(&EPUB_AUTHORS_NOTE_END_SELECTOR),
            )
        } else {
            extract_from_fanficfare_generated_chapter(&parsed, &title)
        };

        Self {
            identifier,
            date_published,
            title,
            url,
            content,
            authors_note_start,
            authors_note_end,
        }
    }
}

/// This fonction allows us to have a compatibility layer over chapters generated by Fanficfare
/// returns `(content, authors_note_start, authors_note_end)`
fn extract_from_fanficfare_generated_chapter(
    parsed: &Html,
    title: &str,
) -> (Option<String>, Option<String>, Option<String>) {
    let mut notes_iter = parsed.select(&EPUB_FANFICFARE_AUTHORS_NOTE_SELECTOR).rev();
    // To simplify processing if there is only one note, we put it at the end
    let authors_note_end = notes_iter.next().map(|e| e.inner_html());
    let authors_note_start = notes_iter.next().map(|e| e.inner_html());

    let content = parsed
        .get_inner_html_of(&EPUB_FANFICFARE_CHAPTER_CONTENT_SELECTOR)
        .map(|e| e.replace(&format!("<h3 class=\"fff_chapter_title\">{title}</h3>"), ""))
        .map(|e| {
            authors_note_start
                .as_ref()
                .map_or(e.clone(), |note| e.replace(note, ""))
        })
        .map(|e| {
            authors_note_end
                .as_ref()
                .map_or(e.clone(), |note| e.replace(note, ""))
        });
    (content, authors_note_start, authors_note_end)
}
