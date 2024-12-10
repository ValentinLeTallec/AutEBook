#[cfg(feature = "fanficfare")]
mod fanficfare;
mod royalroad;
use crate::updater::WebNovel;

#[cfg(feature = "fanficfare")]
use self::fanficfare::FanFicFareCompatible;
use self::royalroad::RoyalRoad;

pub trait Source {
    fn new(url: &str) -> Option<Self>
    where
        Self: Sized;
    fn get_updater(&self) -> Option<Box<dyn WebNovel>> {
        None
    }
}

pub struct Unsupported;
impl Source for Unsupported {
    fn new(_url: &str) -> Option<Self> {
        None
    }
}

macro_rules! try_source {
    ($book_source:ident, $url:expr) => {{
        if let Some(fiction) = $book_source::new($url) {
            return Box::new(fiction);
        }
    }};
}

pub fn get(url: &str) -> Box<dyn Source> {
    try_source!(RoyalRoad, url);
    #[cfg(feature = "fanficfare")]
    try_source!(FanFicFareCompatible, url);
    Box::new(Unsupported {})
}
