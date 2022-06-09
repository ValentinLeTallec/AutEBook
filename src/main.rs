#![allow(unused)]
// #[no_panic]
mod book;
mod source;

use std::fs;
// use std::path::Path;
use book::Book;
use std::time::{Duration, SystemTime};
use walkdir::WalkDir;

const PATH: &str = "/home/valentin/temp/here";
// const PATH: &str = "/home/valentin/Dropbox/Applications/Dropbox PocketBook";
const EPUB: &str = "epub";

fn get_books(path: &str) -> Vec<Book> {
    WalkDir::new(PATH)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map_or(false, |v| v == EPUB))
        .map(|e| Book::new(e.path()))
        .collect()
}

fn main() {
    let now = SystemTime::now();
    let paths = fs::read_dir(PATH).unwrap();

    let books = get_books(PATH);
    books.into_iter().for_each(|b| b.update());
    post_action();

    match now.elapsed() {
        Ok(elapsed) => {
            println!("Time elasped : {}s", elapsed.as_secs());
        }
        Err(e) => {
            println!("Error: {e:?}");
        }
    }
}

fn post_action() {
    // Remove empty files
    WalkDir::new(PATH)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.metadata().map(|m| m.len() == 0).unwrap_or(false)) // File is empty
        .for_each(|f| {
            fs::remove_file(f.path()).expect(&format!(
                "{} is empty but could not be deleted",
                f.path().display()
            ))
        })
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
