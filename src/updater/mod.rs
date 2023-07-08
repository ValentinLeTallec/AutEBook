mod fanficfare;
pub use fanficfare::FanFicFare;
use std::path::Path;

#[derive(Debug)]
pub enum UpdateResult {
    Unsupported,
    UpToDate,
    Updated(u16),
    Skipped,
    MoreChapterThanSource(u16),
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum CreationResult {
    Created(Box<Path>),
    CouldNotCreate,
    CreationNotSupported,
}

pub trait Update {
    fn new() -> Self
    where
        Self: Sized;
    fn create(&self, path: &Path, url: &str) -> CreationResult;
    fn update(&self, path: &Path) -> UpdateResult;
}
