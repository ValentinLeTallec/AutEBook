use super::cache::Cache;
use super::image;
use super::request;
use crate::{ErrorPrint, MULTI_PROGRESS};

use chrono::{DateTime, Utc};
use derive_more::derive::Debug;
use epub::doc::EpubDoc;
use eyre::{eyre, Result};
use lazy_regex::regex;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::path::Path;
use url::Url;

/// Declare selectors that are only initialised once and add tests to ensure they can be safely unwraped
/// The syntax is `SELECTOR_NAME: "selector";`
#[macro_export]
macro_rules! lazy_selectors {
    ( $( $selector_name:ident: $selector:expr; )+ ) => {
        $(
        static $selector_name: std::sync::LazyLock<scraper::Selector> =
            std::sync::LazyLock::new(|| scraper::Selector::parse($selector)
                .expect("One of the lazy selectors failed, run `cargo test` to find out which"));
        )*

        #[cfg(test)]
        mod lazy_selectors_autotest {
            $(
                /// Ensure the selector can be unwraped safely
                #[test]
                #[allow(non_snake_case)]
                fn $selector_name() {
                    assert!(scraper::Selector::parse(&$selector).is_ok());
                }
            )*
        }
    };
}

lazy_selectors! {
    CONTENT_SELECTOR: ".chapter-inner.chapter-content";

    // Strange selectors are because RR doesn't have a way to tell if the author's note is
    // at the start or the end in the HTML.
    AUTHORS_NOTE_START_SELECTOR: "hr + .portlet > .author-note";
    AUTHORS_NOTE_END_SELECTOR: "div + .portlet > .author-note";

    TITLE_SELECTOR: "h1";
    AUTHOR_SELECTOR: "h4 a";
    DESCRIPTION_SELECTOR: ".description > .hidden-content";
    WATERMARK_SELECTOR: "[class^=cj],[class^=cm]";

    TITLE_ELEMENT_SELECTOR: "title";
    BODY_ELEMENT_SELECTOR: "body";

    EPUB_META_CHAPTER_URL_SELECTOR: "meta[name=chapterurl]";
    EPUB_META_DATE_PUBLISHED_SELECTOR: "meta[name=published]";
    EPUB_META_GENERATOR_SELECTOR: "meta[name=generator]";

    EPUB_CHAPTER_CONTENT_SELECTOR: ".chapter-content";
    EPUB_AUTHORS_NOTE_START_SELECTOR: ".authors-note-start";
    EPUB_AUTHORS_NOTE_END_SELECTOR: ".authors-note-end";
    EPUB_FANFICFARE_AUTHORS_NOTE_SELECTOR: ".author-note-portlet";
}

#[derive(Default, Clone, Debug)]
pub struct Book {
    pub id: u32,
    pub url: String,
    pub title: String,
    pub author: String,
    #[debug("{description:50?}")]
    pub description: String,
    pub date_published: String,
    pub cover_url: String,
    pub chapters: Vec<Chapter>,
}
impl Book {
    pub fn fetch_without_chapter_content(url: &str) -> Result<Self> {
        // Cover in script tag: window.fictionCover = "...";
        let cover_regex = regex!(r#"window\.fictionCover = "(.*)";"#);
        // Chapters array in script tag: window.chapters = [...];
        let chapters_regex = regex!(r"window\.chapters = (\[.*]);");

        let response = request::get_text(url)?;

        // Parse book metadata.
        let parsed = Html::parse_document(&response);
        let title = parsed
            .get_inner_html_of(&TITLE_SELECTOR)
            .ok_or_else(|| eyre!("No title found"))?;

        let author = parsed
            .get_inner_html_of(&AUTHOR_SELECTOR)
            .unwrap_or_else(|| String::from("<unknown>"));

        let description = parsed
            .get_inner_html_of(&DESCRIPTION_SELECTOR)
            .unwrap_or_default();

        // Parse chapter metadata.
        let cover = cover_regex
            .captures(&response)
            .ok_or_else(|| eyre!("No cover found"))?[1]
            .to_string();
        let chapters = chapters_regex
            .captures(&response)
            .ok_or_else(|| eyre!("No chapters found"))?[1]
            .to_string();
        let chapters: Vec<Chapter> = serde_json::from_str::<Vec<RoyalRoadChapter>>(&chapters)?
            .iter()
            .map(RoyalRoadChapter::to_chapter)
            .collect();

        Ok(Self {
            id: Self::get_id_from_url(url)?,
            url: url.to_string(),
            cover_url: cover,
            title,
            author,
            description,
            date_published: chapters
                .first()
                .ok_or_else(|| eyre!("No chapter"))?
                .date_published
                .to_rfc3339(),
            chapters,
        })
    }

    pub fn from_path(path: &Path) -> Result<Self> {
        let now = chrono::Utc::now();
        let mut epub_doc = EpubDoc::new(path)?;
        let url = epub_doc.mdata("source").unwrap_or_default();
        let mut book = Self {
            id: Self::get_id_from_url(&url)?,
            url,
            title: epub_doc.mdata("title").unwrap_or_default(),
            author: epub_doc.mdata("creator").unwrap_or_default(),
            description: epub_doc.mdata("description").unwrap_or_default(),
            date_published: epub_doc.mdata("date").unwrap_or_else(|| now.to_rfc3339()),
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
            id: self.id,
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

    fn get_id_from_url(url: &str) -> Result<u32, eyre::Error> {
        let url = Url::parse(url)?;
        let id = url
            .path_segments()
            .and_then(|mut s| s.nth(1))
            .and_then(|f| f.parse().ok())
            .ok_or_else(|| eyre!("Invalid book URL: {url}"))?;
        Ok(id)
    }
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct RoyalRoadChapter {
    pub id: u32,
    pub order: u32,
    pub date: DateTime<Utc>,
    pub title: String,
    pub url: String,
}
impl RoyalRoadChapter {
    pub fn to_chapter(&self) -> Chapter {
        Chapter {
            identifier: self.id.to_string(),
            date_published: self.date,
            title: self.title.clone(),
            url: format!("https://www.royalroad.com{}", self.url),
            content: None,
            authors_note_start: None,
            authors_note_end: None,
        }
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
            .get_inner_html_of(&TITLE_ELEMENT_SELECTOR)
            .unwrap_or_default();

        let url = parsed
            .get_meta_content_of(&EPUB_META_CHAPTER_URL_SELECTOR)
            .unwrap_or_default();

        let date_published = parsed
            .get_meta_content_of(&EPUB_META_DATE_PUBLISHED_SELECTOR)
            .and_then(|d| DateTime::parse_from_rfc3339(&d).ok())
            .unwrap_or_else(|| now.into())
            .into();

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

    pub fn update_chapter_content(&mut self) -> Result<()> {
        if self.content.is_some() {
            return Ok(());
        }

        let text = request::get_text(&self.url)?;

        let mut parsed = Html::parse_document(&text);

        remove_royal_road_warnings(&mut parsed);

        // Parse content.
        self.content = parsed.get_inner_html_of(&CONTENT_SELECTOR);

        // Parse starting author note.
        self.authors_note_start = parsed.get_inner_html_of(&AUTHORS_NOTE_START_SELECTOR);

        // Parse ending author note.
        self.authors_note_end = parsed.get_inner_html_of(&AUTHORS_NOTE_END_SELECTOR);

        Ok(())
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
        .get_inner_html_of(&BODY_ELEMENT_SELECTOR)
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

/// Remove royalroad warnings
/// Please don't use this tool to re-publish authors' works without their permission.
fn remove_royal_road_warnings(parsed: &mut Html) {
    let bad_paragraphs = parsed
        .select(&WATERMARK_SELECTOR)
        .filter(|e| e.inner_html().len() < 200)
        .map(|e| e.id())
        .collect::<Vec<_>>();

    for id in bad_paragraphs {
        if let Some(mut node) = parsed.tree.get_mut(id) {
            node.detach();
        }
    }
}

trait QuickSelect {
    fn get_inner_html_of(&self, selector: &Selector) -> Option<String>;
    fn get_meta_content_of(&self, selector: &Selector) -> Option<String>;
}
impl QuickSelect for Html {
    fn get_inner_html_of(&self, selector: &Selector) -> Option<String> {
        self.select(selector)
            .next()
            .map(|element| element.inner_html())
            .filter(|s| !s.is_empty())
    }
    fn get_meta_content_of(&self, selector: &Selector) -> Option<String> {
        self.select(selector)
            .next()
            .and_then(|e| e.attr("content"))
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
    }
}
