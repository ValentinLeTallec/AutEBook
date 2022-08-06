// #[no_panic]
mod book;
mod source;
mod updater;

use crate::book::Book;
use crate::updater::UpdateResult;

use clap::Parser;
use colorful::Colorful;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs;
use std::sync::mpsc::channel;
use std::time::SystemTime;
use threadpool::ThreadPool;
use walkdir::WalkDir;

const DEFAULT_PATH: &str = ".";
const EPUB: &str = "epub";

/// A small utility used to update books by levraging `FanFicFare`
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
    #[clap(short, long, default_value_t = 8)]
    nb_threads: usize,
}

fn main() {
    let args = Args::parse();
    // println!("{:?}", args);
    println!(
        "Updating books ({}) using {} works\n",
        &args.dir, args.nb_threads
    );
    let now = SystemTime::now();

    let book_files = get_book_files(&args.dir);
    let nb_threads = args.nb_threads;
    update_books(book_files, nb_threads);
    post_action(&args.dir);

    if let Ok(dt) = now.elapsed() {
        println!("Time elasped : {}s", dt.as_secs());
    }
}

fn update_books(book_files: Vec<walkdir::DirEntry>, nb_threads: usize) -> Vec<book::BookResult> {
    let nb_books = book_files.len();

    let pool = ThreadPool::new(nb_threads);
    let (sender, receiver) = channel();
    let bar = ProgressBar::new(nb_books as u64);

    bar.set_style(
        ProgressStyle::default_bar().template(
            "{prefix}\n[{elapsed}/{duration}] {wide_bar:white/orange} {pos:>3}/{len:3} ({percent}%)\n{msg}",
        ),
    );

    for e in book_files {
        let sender = sender.clone();
        pool.execute(move || {
            sender
                .send(Book::new(e.path()).update())
                .expect("channel will be there waiting for the pool");
        });
    }

    receiver
        .iter()
        .inspect(|_| bar.inc(1))
        .inspect(|b_res| bar.set_prefix(b_res.name.clone()))
        .inspect(|b_res| match b_res.result {
            UpdateResult::Updated(n) => {
                let nb = format!("[{:>4}]", format!("+{}", n)).green().bold();
                bar.println(format!("{} {}\n", nb, b_res.name));
            }
            UpdateResult::MoreChapterThanSource(n) => {
                let nb = format!("[{:>4}]", format!("-{}", n)).red().bold();
                bar.println(format!("{} {}\n", nb, b_res.name));
            }
            _ => (),
        })
        .take(nb_books)
        .collect()
}

fn get_book_files(path: &str) -> Vec<walkdir::DirEntry> {
    WalkDir::new(path)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map_or(false, |v| v == EPUB))
        .collect()
}

fn post_action(path: &str) {
    // Remove empty files
    WalkDir::new(path)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.metadata().map(|m| m.len() == 0).unwrap_or(false)) // File is empty
        .for_each(|f| {
            fs::remove_file(f.path()).unwrap_or_else(|_| {
                panic!("{} is empty but could not be deleted", f.path().display());
            });
        });
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
