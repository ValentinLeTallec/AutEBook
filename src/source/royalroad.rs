use super::Source;
use crate::updater::Native;
use crate::updater::WebNovel;
use lazy_regex::regex;

#[derive(Debug, PartialEq, Eq)]
pub struct RoyalRoad {
    id: u32,
}

impl Source for RoyalRoad {
    fn get_updater(&self) -> Option<Box<dyn WebNovel>> {
        Some(Box::new(Native::new()))
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

    #[test]
    fn test_new() {
        let source = RoyalRoad::new("https://www.royalroad.com/fiction/36049/the-primal-hunter");
        assert!(source.is_some());
        let source = RoyalRoad::new("https://www.df.com/fiction/36049/the-primal-hunter");
        assert!(source.is_none());
    }
}
