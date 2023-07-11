use super::Source;
use crate::updater::FanFicFare;
use crate::updater::WebNovel;
use lazy_regex::regex;

#[derive(Debug, PartialEq, Eq)]
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
    fn get_updater(&self) -> Option<Box<dyn WebNovel>> {
        Some(Box::new(FanFicFare::new()))
    }

    fn new(fiction_url: &str) -> Option<Self> {
        let fiction_url_pattern =
            regex!(r"^https://www\.royalroad\.com/fiction/(\d+)(/.{0,100})?$");
        let captures = fiction_url_pattern.captures(fiction_url)?;
        let id = captures[1].parse::<u32>().ok()?;
        Some(Self { id })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn new_royal_road(fiction_url: &str) -> RoyalRoad {
        let source = RoyalRoad::new(fiction_url);
        source.unwrap_or_else(|| {
            panic!(" The url `{fiction_url}` could not be recognised as valid for RoyalRoad")
        })
    }

    #[test]
    fn test_new() {
        let source = RoyalRoad::new("https://www.royalroad.com/fiction/36049/the-primal-hunter");
        assert!(source.is_some());
        let source = RoyalRoad::new("https://www.df.com/fiction/36049/the-primal-hunter");
        assert!(source.is_none());
    }

    #[test]
    fn test_royalroad_long_url() {
        let source = new_royal_road("https://www.royalroad.com/fiction/36049/the-primal-hunter");
        let rss_feed = "https://www.royalroad.com/fiction/syndication/36049";
        assert_eq!(source.get_syndication_url(), Some(rss_feed.to_string()));
    }

    #[test]
    fn test_royalroad_short_url() {
        let source = new_royal_road("https://www.royalroad.com/fiction/36049");
        let rss_feed = "https://www.royalroad.com/fiction/syndication/36049";
        assert_eq!(source.get_syndication_url(), Some(rss_feed.to_string()));
    }
}
