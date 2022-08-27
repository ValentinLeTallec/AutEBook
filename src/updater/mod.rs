mod fanficfare;
pub use fanficfare::FanFicFare;
use std::path::Path;

#[derive(Debug)]
pub enum UpdateResult {
    NotSupported,
    UpToDate,
    Updated(u16),
    Skipped,
    MoreChapterThanSource(u16),
}

pub trait Update {
    fn new() -> Self
    where
        Self: Sized;
    fn update(&self, path: &Path) -> UpdateResult;
}
