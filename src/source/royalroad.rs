use chrono::{DateTime, Utc};
use scraper::Html;

use crate::parsing_utils::QuickSelect;
use crate::{lazy_selectors, request, ErrorPrint, MULTI_PROGRESS};

lazy_selectors! {
    TITLE_SELECTOR: "title";
    TIME_PUBLISHED_SELECTOR: "pubDate";
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

        let rss_xml = match request::get_text(&rss_url) {
            Ok(rss_xml) => rss_xml,
            Err(error) => {
                MULTI_PROGRESS.eprintln(&error);
                return None;
            }
        };

        let parsed = Html::parse_fragment(&rss_xml);
        let title = parsed.get_inner_html_of(&TITLE_SELECTOR);

        let last_time_published = parsed
            .select(&TIME_PUBLISHED_SELECTOR)
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
