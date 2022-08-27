// #![warn(unused_extern_crates)]
mod book;
mod source;
mod updater;

use crate::book::Book;
use crate::updater::UpdateResult;

use clap::Parser;
use colorful::Colorful;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::fs;
use std::time::SystemTime;
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
    setup_nb_threads(args.nb_threads);

    println!(
        "Updating books in '{}' using {} workers\n",
        &args.dir, args.nb_threads
    );

    let now = SystemTime::now();
    let book_files = get_book_files(&args.dir);
    // book_files.sort_by(|a, b| a.metadata().unwrap().modified)
    update_books(&book_files);
    post_action(&args.dir);

    if let Ok(dt) = now.elapsed() {
        println!("Time elasped : {}s", dt.as_secs());
    }
}

fn setup_nb_threads(nb_threads: usize) {
    let custom_rayon_conf = rayon::ThreadPoolBuilder::new()
        .num_threads(nb_threads)
        .build_global();
    if custom_rayon_conf.is_err() {
        eprintln!(
            "Could not use custom number of threads ({}), default number ({}) was used",
            nb_threads,
            rayon::current_num_threads()
        );
    }
}

fn update_books(book_files: &[walkdir::DirEntry]) {
    let bar = ProgressBar::new(book_files.len() as u64);
    bar.set_style(
        ProgressStyle::default_bar().template(
            "\n{prefix}\n[{elapsed}/{duration}] {wide_bar:white/orange} {pos:>3}/{len:3} ({percent}%)\n{msg}",
        ),
    );
    book_files
        .par_iter()
        .map(|e| Book::new(e.path()).update())
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
            UpdateResult::Skipped => {
                let prefix = format!("[Skip]").blue().bold();
                bar.println(format!("{} {}\n", prefix, b_res.name));
            }
            _ => (),
        })
        .count();
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
                eprintln!("{} is empty but could not be deleted", f.path().display());
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
