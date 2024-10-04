mod royalroad;
use crate::updater::WebNovel;

use self::royalroad::RoyalRoad;

pub trait Source {
    fn new(url: &str) -> Option<Self>
    where
        Self: Sized;
    fn get_updater(&self) -> Option<Box<dyn WebNovel>> {
        None
    }
    fn get_syndication_url(&self) -> Option<String> {
        None
    }
}

pub struct Unsupported;
impl Source for Unsupported {
    fn new(_url: &str) -> Option<Self> {
        None
    }
}

pub fn get(url: &str) -> Box<dyn Source> {
    if let Some(fiction) = RoyalRoad::new(url) {
        return Box::new(fiction);
    }
    Box::new(Unsupported {})
}
