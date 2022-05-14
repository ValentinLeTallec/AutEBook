pub mod royalroad;
use royalroad::RoyalRoad;

pub trait Syndication {
    fn get_syndication_url(&self) -> String;
}

pub trait FanFicFare {
    fn relates_to(&self, url: &str) -> bool;
}

pub fn get(url: &str) -> Option<Box<dyn FanFicFare>> {
    // match url {
    //     RoyalRoad::relates_to(u) => RoyalRoad::new(u),
    //     _ => false
    // }
    Some(Box::new(RoyalRoad::new(url)))
}
