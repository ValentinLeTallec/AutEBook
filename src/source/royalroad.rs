use super::Source;
use crate::updater::FanFicFare;
use crate::updater::Update;
use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    static ref FICTION_URL_PATTERN: Regex =
        Regex::new(r"^https://www\.royalroad\.com/fiction/(\d+)(/.{0,100})?$").unwrap();
}

#[derive(Debug, PartialEq)]
pub struct RoyalRoad {
    id: u32,
}

impl Source for RoyalRoad {
    fn get_syndication_url(&self) -> Option<String> {
        Some(format!(
            "https://www.royalroad.com/fiction/syndication/{}",
            self.id
        ))
    }
    fn get_updater(&self) -> Option<Box<dyn Update>> {
        Some(Box::new(FanFicFare::new()))
    }

    fn new(fiction_url: &str) -> Option<Self> {
        let captures = FICTION_URL_PATTERN.captures(fiction_url)?;
        let id = captures[1].parse::<u32>().ok()?;
        Some(Self { id })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let source = RoyalRoad::new("https://www.royalroad.com/fiction/36049/the-primal-hunter");
        assert_ne!(source, None);
        let source = RoyalRoad::new("https://www.df.com/fiction/36049/the-primal-hunter");
        assert_eq!(source, None);
    }

    #[test]
    fn test_royalroad_long_url() {
        let source =
            RoyalRoad::new("https://www.royalroad.com/fiction/36049/the-primal-hunter").unwrap();
        let rss_feed = "https://www.royalroad.com/fiction/syndication/36049";
        assert_eq!(source.get_syndication_url(), Some(rss_feed.to_string()));
    }

    #[test]
    fn test_royalroad_short_url() {
        let source = RoyalRoad::new("https://www.royalroad.com/fiction/36049").unwrap();
        let rss_feed = "https://www.royalroad.com/fiction/syndication/36049";
        assert_eq!(source.get_syndication_url(), Some(rss_feed.to_string()));
    }
}
