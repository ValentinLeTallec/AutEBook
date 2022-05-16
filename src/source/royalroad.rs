use super::{FanFicFare, Syndication};
use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    static ref FICTION_URL_PATTERN: Regex =
        Regex::new(r"^https://www\.royalroad\.com/fiction/(\d+)(/.{0,100})?$").unwrap();
}

#[derive(Debug, PartialEq)]
pub struct RoyalRoad {
    id: u16,
}

impl Syndication for RoyalRoad {
    fn get_syndication_url(&self) -> String {
        format!("https://www.royalroad.com/fiction/syndication/{}", self.id)
    }
}

impl FanFicFare for RoyalRoad {
    fn new(fiction_url: &str) -> Option<RoyalRoad> {
        let captures = FICTION_URL_PATTERN.captures(fiction_url)?;
        let id = &captures[1].parse::<u16>().ok()?;
        Some(RoyalRoad { id: *id })
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
    use super::*;

    #[test]
    fn test_royalroad_long_url() {
        let source =
            RoyalRoad::new("https://www.royalroad.com/fiction/36049/the-primal-hunter").unwrap();
        let rss_feed = "https://www.royalroad.com/fiction/syndication/36049";
        assert_eq!(source.get_syndication_url(), rss_feed);
    }

    #[test]
    fn test_royalroad_short_url() {
        let source = RoyalRoad::new("https://www.royalroad.com/fiction/36049").unwrap();
        let rss_feed = "https://www.royalroad.com/fiction/syndication/36049";
        assert_eq!(source.get_syndication_url(), rss_feed);
    }
}
