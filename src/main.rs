#![warn(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo,
    clippy::unwrap_used,
    clippy::expect_used,
    // clippy::missing_docs_in_private_items,
    clippy::wildcard_enum_match_arm,
    clippy::use_debug
)]

mod book;
mod source;
mod updater;

use crate::book::Book;
use crate::updater::UpdateResult;

use clap::{CommandFactory, Parser, Subcommand};
use colorful::Colorful;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::fs;
use std::path::Path;
use std::time::SystemTime;
use walkdir::WalkDir;

const EPUB: &str = "epub";

/// A small utility used to update books by levraging `FanFicFare`
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None, propagate_version = true)]
struct Args {
    #[clap(subcommand)]
    subcommand: Commands,

    /// Path to the work directory
    #[clap(short, long, default_value = "./")]
    dir: String,

    /// Number of threads to use
    #[clap(short, long, default_value_t = 8)]
    nb_threads: usize,
}
#[derive(Subcommand, Debug)]
enum Commands {
    /// Update specific books, based on path(s) given,
    /// if no path is given will update the work directory
    Update { paths: Vec<String> },

    /// Generate a SHELL completion script and print to stdout
    Completions { shell: clap_complete::Shell },
}

fn main() {
    let args = Args::parse();
    setup_nb_threads(args.nb_threads);
    let work_dir = Path::new(&args.dir);
    let now = SystemTime::now();

    match args.subcommand {
        Commands::Update { paths } => {
            println!(
                "Updating books in '{}' using {} workers\n",
                &args.dir, args.nb_threads
            );
            let book_files: Vec<walkdir::DirEntry> = if paths.is_empty() {
                get_book_files(work_dir)
            } else {
                paths
                    .into_iter()
                    .flat_map(|p| get_book_files(Path::new(&p)))
                    .collect()
            };
            update_books(&book_files);

            if let Ok(dt) = now.elapsed() {
                println!("Time elasped : {}s", dt.as_secs());
            }
        }
        Commands::Completions { shell } => clap_complete::generate(
            shell,
            &mut Args::command(),
            "autebooks",
            &mut std::io::stdout(),
        ),
    }
    post_action(work_dir);
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
    let template_progress = ProgressStyle::with_template(
        "\n{prefix}\n[{elapsed}/{duration}] {wide_bar} {pos:>3}/{len:3} ({percent}%)\n{msg}",
    )
    .unwrap_or_else(|err| {
        eprintln!("{err}");
        ProgressStyle::default_bar()
    });
    bar.set_style(template_progress);
    book_files
        .par_iter()
        .map(|e| Book::new(e.path()).update())
        .inspect(|_| bar.inc(1))
        .inspect(|b_res| bar.set_prefix(b_res.name.clone()))
        .for_each(|b_res| match b_res.result {
            UpdateResult::Updated(n) => {
                let nb = format!("[{:>4}]", format!("+{}", n)).green().bold();
                bar.println(format!("{} {}\n", nb, b_res.name));
            }
            UpdateResult::MoreChapterThanSource(n) => {
                let nb = format!("[{:>4}]", format!("-{}", n)).red().bold();
                bar.println(format!("{} {}\n", nb, b_res.name));
            }
            UpdateResult::Skipped => {
                let prefix = String::from("[Skip]").blue().bold();
                bar.println(format!("{} {}\n", prefix, b_res.name));
            }
            UpdateResult::Unsupported | UpdateResult::UpToDate => (),
        });
}

fn get_book_files(path: &Path) -> Vec<walkdir::DirEntry> {
    WalkDir::new(path)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map_or(false, |v| v == EPUB))
        .collect()
}

fn post_action(path: &Path) {
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
