#![allow(unused)]
// #[no_panic]
mod book;
mod source;

use std::fs;
// use std::path::Path;
use book::Book;
use clap::Parser;
use std::fs;
use std::time::{Duration, SystemTime};
use walkdir::WalkDir;

// const DEFAULT_PATH: &str = "/home/valentin/temp/here";
const DEFAULT_PATH: &str = "/home/valentin/Dropbox/Applications/Dropbox PocketBook";
const EPUB: &str = "epub";

/// A small utility used to update books by levraging FanFicFare
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Path to the book to update
    #[clap(short, long)]
    path: Option<String>,

    /// Path to the directory to update
    #[clap(short, long, default_value = DEFAULT_PATH)]
    dir: String,

    /// Number of times to greet
    #[clap(short, long, default_value_t = 1)]
    count: u8,
}

fn main() {
    let args = Args::parse();
    println!("{:?}", args);
    let now = SystemTime::now();

    let books = get_books(&args.dir);
    books.into_iter().for_each(|b| b.update());
    post_action(&args.dir);

    if let Ok(dt) = now.elapsed() {
        println!("Time elasped : {}s", dt.as_secs())
    }
}

fn get_books(path: &str) -> Vec<Book> {
    WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map_or(false, |v| v == EPUB))
        .map(|e| Book::new(e.path()))
        .collect()
}

fn post_action(path: &str) {
    // Remove empty files
    WalkDir::new(path)
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
