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

pub fn get(url: &str) -> Box<dyn Source> {
    if let Some(fiction) = RoyalRoad::new(url) {
        return Box::new(fiction);
    }
    #[cfg(feature = "fanficfare")]
    if let Some(fiction) = FanFicFareCompatible::new(url) {
        return Box::new(fiction);
    }
    Box::new(Unsupported {})
}
