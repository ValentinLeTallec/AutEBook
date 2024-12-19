pub mod native;

use epub::doc::EpubDoc;
use eyre::{bail, eyre, Error, Result};
use std::{ffi::OsStr, fs, path::Path};

#[derive(Debug)]
pub enum UpdateResult {
    Unsupported,
    UpToDate,
    Updated(u16),
    Skipped,
    MoreChapterThanSource(u16),
    Error(Error),
}

type DisplayName = String;

pub trait Download {
    fn get_url(&self) -> String;

    fn get_title(&self, path: &Path) -> String {
        EpubDoc::new(path)
            .ok()
            .and_then(|e| e.mdata("title"))
            .unwrap_or_else(|| format!("{} (No Title)", path.to_string_lossy()))
    }

    fn create(&self, _dir: &Path, _filename: Option<&OsStr>, _url: &str) -> Result<DisplayName> {
        bail!("This webnovel does not contain a supported source URL")
    }

    fn update(&self, path: &Path) -> UpdateResult {
        self.do_update(path).unwrap_or_else(UpdateResult::Error)
    }

    fn do_update(&self, _path: &Path) -> Result<UpdateResult> {
        Ok(UpdateResult::Unsupported)
    }

    fn stash_and_recreate(&self, book: &Path, stash_folder: &Path) -> Result<()> {
        let parent_dir = book
            .parent()
            .ok_or_else(|| eyre!("Could not retrieve the book's parent directory."))?;

        let original_filename = book
            .file_name()
            .ok_or_else(|| eyre!("Could not retrieve the book's filename."))?
            .to_owned();

        // Stashing of the current instance of the book in an sub-directory
        let timestamp = chrono::Utc::now().format("_%Y-%m-%d_%Hh%M").to_string();
        let extension = book
            .extension()
            .ok_or_else(|| eyre!("Could not retrieve the book's extension."))?;

        let mut stashed_filename = book
            .file_stem()
            .ok_or_else(|| eyre!("Could not retrieve the book's filename."))?
            .to_owned();
        stashed_filename.push(timestamp);
        stashed_filename.push(".");
        stashed_filename.push(extension);

        fs::create_dir_all(stash_folder)?;
        fs::rename(book, stash_folder.join(stashed_filename))?;

        // Creation of the new instance of the book
        self.create(parent_dir, Some(&original_filename), &self.get_url())?;
        Ok(())
    }
}
