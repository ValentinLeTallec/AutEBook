use chrono::{DateTime, Utc};
use derive_more::derive::Debug;
use eyre::{eyre, Context, Result};
use lazy_regex::regex;
use scraper::Html;
use serde::{Deserialize, Serialize};
use std::path::Path;
use url::Url;
use uuid::Uuid;

use crate::parsing_utils::QuickSelect;
use crate::updater::book::{Book, Chapter};
use crate::updater::WebnovelSource;
use crate::{lazy_selectors, request, ErrorPrint, MULTI_PROGRESS};

lazy_selectors! {
    RSS_TITLE_SELECTOR: "title";
    RSS_TIME_PUBLISHED_SELECTOR: "pubDate";

    CONTENT_SELECTOR: ".chapter-inner.chapter-content";

    // Strange selectors are because RR doesn't have a way to tell if the author's note is
    // at the start or the end in the HTML.
    AUTHORS_NOTE_START_SELECTOR: "hr + .portlet > .author-note";
    AUTHORS_NOTE_END_SELECTOR: "div + .portlet > .author-note";

    TITLE_SELECTOR: "h1";
    AUTHOR_SELECTOR: "h4 a";
    DESCRIPTION_SELECTOR: ".description > .hidden-content";
    WATERMARK_SELECTOR: "[class^=cj],[class^=cm]";
}

#[derive(Debug)]
pub struct RoyalRoad {
    pub title: String,
    pub url: String,
    pub last_time_published: DateTime<Utc>,
}

impl RoyalRoad {
    pub fn new(fiction_url: &str) -> Option<Self> {
        let royalroyal_url = "https://www.royalroad.com/fiction/";
        let rss_url = "https://www.royalroad.com/fiction/syndication/";

        if !fiction_url.starts_with(royalroyal_url) {
            return None;
        }
        let rss_url = fiction_url.replace(royalroyal_url, rss_url);

        let rss_xml = match request::get_text(&rss_url).wrap_err_with(|| {
            format!("Could not check RoyalRoad for updates from url : {royalroyal_url}")
        }) {
            Ok(rss_xml) => rss_xml,
            Err(error) => {
                MULTI_PROGRESS.eprintln(&error);
                return None;
            }
        };

        let parsed = Html::parse_fragment(&rss_xml);
        let title = parsed.get_inner_html_of(&RSS_TITLE_SELECTOR);

        let last_time_published = parsed
            .select(&RSS_TIME_PUBLISHED_SELECTOR)
            .map(|e| e.inner_html())
            .filter_map(|value| DateTime::parse_from_rfc2822(&value).ok())
            .max()
            .map(|e| e.to_utc());

        title
            .zip(last_time_published)
            .map(|(title, last_time_published)| Self {
                title,
                url: fiction_url.to_owned(),
                last_time_published,
            })
    }
}

impl WebnovelSource for RoyalRoad {
    fn get_title(&self, _path: &Path) -> String {
        self.title.clone()
    }

    fn get_url(&self) -> String {
        self.url.clone()
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

    fn fetch_without_chapter_content(&self) -> Result<Book> {
        let url = &self.get_url();

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

        Ok(Book {
            id: get_id_from_url(url),
            url: url.to_string(),
            cover_url: cover,
            title,
            author,
            description,
            date_published: chapters
                .iter()
                .map(|e| e.date_published)
                .min()
                .unwrap_or_else(Utc::now),
            chapters,
        })
    }

    fn update_chapter_content(&self, chapter: &mut Chapter) -> Result<()> {
        if chapter.content.is_some() {
            return Ok(());
        }

        let text = request::get_text(&chapter.url)?;

        let mut parsed = Html::parse_document(&text);

        remove_royal_road_warnings(&mut parsed);

        // Parse content.
        chapter.content = parsed.get_inner_html_of(&CONTENT_SELECTOR);

        // Parse starting author note.
        chapter.authors_note_start = parsed.get_inner_html_of(&AUTHORS_NOTE_START_SELECTOR);

        // Parse ending author note.
        chapter.authors_note_end = parsed.get_inner_html_of(&AUTHORS_NOTE_END_SELECTOR);

        Ok(())
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

fn get_id_from_url(url: &str) -> String {
    Url::parse(url)
        .ok()
        .and_then(|url| {
            url.path_segments()
                .and_then(|mut e| e.nth(1))
                .map(ToString::to_string)
        })
        .unwrap_or_else(|| Uuid::new_v4().to_string())
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
