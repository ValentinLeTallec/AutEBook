#![allow(unused)]
// #[no_panic]
mod book;
mod source;

use std::fs;
// use std::path::Path;
use book::Book;
use walkdir::WalkDir;

const PATH: &str = "/home/valentin/Dropbox/Applications/Dropbox PocketBook/Lu";
const EPUB: &str = "epub";

fn main() {
    let paths = fs::read_dir(PATH).unwrap();

    let mut books = Vec::new();

    for entry in WalkDir::new(PATH)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map_or(false, |v| v == EPUB))
    {
        books.push(Book::new(entry.path()));
        // Book::new(entry.path()).print_path();

        // println!("{}", entry.path().display());
        // let book = Book::new(entry.path());
    }
    post_action();
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
