use std::ffi::OsStr;
use std::path::Path;

use crate::updater::native::api::RoyalRoadApi;
use crate::updater::native::epub::write_epub;
use ::epub::doc::EpubDoc;
use color_eyre::eyre::Result;
use color_eyre::Section;
use url::Url;

use super::{UpdateResult, WebNovel};

mod api;
mod cache;
mod epub;
mod image;
mod xml_ext;

pub struct Native;

impl Native {
    fn do_update(path: &Path) -> Option<UpdateResult> {
        let url = EpubDoc::new(path).ok()?.mdata("source")?;
        let url = Url::parse(&url).ok()?;
        let id = url
            .path_segments()
            .and_then(|mut s| s.nth(1))
            .unwrap()
            .parse()
            .ok()?;
        let api = RoyalRoadApi::new();

        let book = api.get_book(id, false).ok()?;
        write_epub(&book, path.to_str().map(|f| String::from(f))).ok()?;
        Some(UpdateResult::Updated(0))
    }
}

impl WebNovel for Native {
    fn new() -> Self {
        Self {}
    }
    fn create(&self, dir: &Path, filename: Option<&OsStr>, url: &str) -> Result<crate::Book> {
        let url = Url::parse(&url)?;
        let id = url
            .path_segments()
            .and_then(|mut s| s.nth(1))
            .unwrap()
            .parse()
            .with_note(|| format!("Invalid book URL: {}", url))?;

        let api = RoyalRoadApi::new();
        let book = api.get_book(id, false)?;
        let outfile = write_epub(
            &book,
            filename.and_then(|f| f.to_str()).map(|f| String::from(f)),
        )?;

        let file_path = dir.join(outfile);
        Ok(crate::Book::new(&file_path))
    }

    fn update(&self, path: &Path) -> UpdateResult {
        Self::do_update(path).unwrap_or(UpdateResult::Unsupported)
    }
}
