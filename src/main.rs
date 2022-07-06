#![allow(unused)]
// #[no_panic]
mod book;
mod source;

use crate::book::Book;
use crate::source::UpdateResult;
use clap::Parser;
use indicatif::ProgressBar;
use indicatif::ProgressIterator;
use indicatif::ProgressStyle;
use std::fs;
use std::sync::mpsc::channel;
use std::time::{Duration, SystemTime};
use threadpool::ThreadPool;
use walkdir::WalkDir;

const DEFAULT_PATH: &str = "/home/valentin/temp/here";
// const DEFAULT_PATH: &str = "/home/valentin/Dropbox/Applications/Dropbox PocketBook";
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

    /// Number of threads to use to update the books
    #[clap(short, long, default_value_t = 4)]
    nb_threads: usize,
}

fn main() {
    let args = Args::parse();
    println!("{:?}", args);
    let now = SystemTime::now();

    let book_files = get_book_files(&args.dir);
    update_all_books(&args, book_files);
    post_action(&args.dir);

    if let Ok(dt) = now.elapsed() {
        println!("Time elasped : {}s", dt.as_secs())
    }
}

fn update_all_books(args: &Args, book_files: Vec<walkdir::DirEntry>) {
    let nb_workers = args.nb_threads;
    let nb_books = book_files.len();

    let pool = ThreadPool::new(nb_workers);
    let (sender, receiver) = channel();
    let bar = ProgressBar::new(nb_books as u64);

    bar.set_style(
        ProgressStyle::default_bar().template(
            "{prefix}\n[{elapsed}/{duration}] {wide_bar:white/orange} {pos:>3}/{len:3} ({percent}%)\n{msg}",
        ),
    );

    book_files.into_iter().for_each(|e| {
        let sender = sender.clone();
        pool.execute(move || {
            sender
                .send(Book::new(e.path()).update())
                .expect("channel will be there waiting for the pool");
            // bar.inc(1);
        });
    });

    let messages: Vec<&str> = Vec::new();

    let map: Vec<book::BookResult> = receiver
        .iter()
        .inspect(|_| bar.inc(1))
        .inspect(|b_res| bar.set_prefix(b_res.name.clone()))
        .inspect(|b_res| {
            if let UpdateResult::Updated(n) = b_res.result {
                // messages.push(format!("[+{}] {}\n", n, b_res.name).as_str().clone());
                bar.set_message(format!("[+{}] {}", n, b_res.name))
            }
        })
        // .inspect(|b_res| bar.set_message(messages.get_muts))
        .take(nb_books)
        .collect();
    // println!("{:#?}", map);
}

fn get_book_files(path: &str) -> Vec<walkdir::DirEntry> {
    WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map_or(false, |v| v == EPUB))
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
