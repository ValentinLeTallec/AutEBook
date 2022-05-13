use super::{FanFicFare, Syndication};

pub struct RoyalRoad {
    id: u16,
}
impl RoyalRoad {
    pub fn new(fiction_url: &str) -> RoyalRoad {
        let id: u16 = 52639;
        RoyalRoad { id }
    }
}
impl Syndication for RoyalRoad {
    fn get_syndication_url(&self) -> String {
        format!("https://www.royalroad.com/fiction/syndication/{}", self.id)
    }
}
impl FanFicFare for RoyalRoad {
    fn relates_to(url: &str) -> bool {
        url.starts_with("https://www.royalroad.com")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_royalroad() {
        let source = RoyalRoad::new(" https://www.royalroad.com/fiction/36049/the-primal-hunter");
        let rss_feed = "https://www.royalroad.com/fiction/syndication/36049";
        assert_eq!(source.get_syndication_url(), rss_feed);
    }
}
