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
#![allow(clippy::multiple_crate_versions)]
mod book;
mod source;
mod updater;

use crate::book::Book;
use crate::updater::UpdateResult;
use clap::{CommandFactory, Parser, Subcommand};
use color_eyre::eyre::Result;
use colorful::Colorful;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

const EPUB: &str = "epub";

/// A small utility used to obtain and update web novels as e-books.
/// It currently levrage `FanFicFare` but is extensible to other updaters.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None, propagate_version = true)]
struct Args {
    #[clap(subcommand)]
    subcommand: Commands,

    /// Path to the work directory.
    #[clap(short, long, default_value = "./", value_hint = clap::ValueHint::DirPath)]
    dir: PathBuf,

    /// Number of threads to use.
    #[clap(short, long, default_value_t = 8)]
    nb_threads: usize,
}
#[derive(Subcommand, Debug)]
enum Commands {
    /// Adds books to the work directory, based on the URL(s) given.
    Add { urls: Vec<String> },

    /// Update specific books, based on path(s) given,
    /// if no path is given it will update the work directory.
    Update {
        /// List of directories containing books to update
        paths: Vec<PathBuf>,

        /// Stash books which contains more chapters than source in the folder defined in `stash_dir`
        /// and recreate them from source
        #[clap(short, long)]
        stash: bool,

        /// The directory where stashed books are stored (books in this folder are excuded from updates).
        /// It is relative to the update path.
        #[clap(short = 'd', long, default_value = "./stashed", value_hint = clap::ValueHint::DirPath)]
        stash_dir: PathBuf,
    },

    /// Recursively remove any 0 bytes epub in provided path(s)
    Clean { paths: Vec<PathBuf> },

    /// Generate a SHELL completion script and print to stdout
    Completions { shell: clap_complete::Shell },
}

struct FileToUpdate {
    file_path: walkdir::DirEntry,
    stash_path: PathBuf,
}

macro_rules! summary {
    ($s:expr, $book_name:expr, $color:ident) => {{
        let prefix = format!("[{:>+4}]", $s).bold().$color();
        format!("{} {:.50}\n", prefix, $book_name)
    }};
}

fn main() -> Result<()> {
    color_eyre::install()?;
    let args = Args::parse();
    setup_nb_threads(args.nb_threads);
    let work_dir = args.dir;

    match args.subcommand {
        Commands::Add { urls } => create_books(work_dir.as_path(), &urls),
        Commands::Update {
            mut paths,
            stash,
            stash_dir,
        } => {
            if paths.is_empty() {
                paths.push(work_dir);
            }

            let book_files: Vec<FileToUpdate> = paths
                .into_iter()
                .flat_map(|p| get_book_files(&p, &p.join(&stash_dir)))
                .collect();

            update_books(&book_files, stash);
        }
        Commands::Clean { paths } => paths.iter().for_each(|p| remove_empty_epub(p.as_path())),
        Commands::Completions { shell } => clap_complete::generate(
            shell,
            &mut Args::command(),
            "autebooks",
            &mut std::io::stdout(),
        ),
    }
    Ok(())
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

fn create_books(dir: &Path, urls: &[String]) {
    let bar = get_progress_bar(urls.len() as u64);

    urls.par_iter().for_each(|url| {
        bar.set_prefix(url.clone());
        let creation_res = Book::create(dir, url);
        bar.inc(1);

        match creation_res {
            Ok(book) => bar.println(format!("{:.50}\n", book.name)),
            Err(e) => eprintln!("{e}"),
        }
    });
}

fn update_books(book_files: &[FileToUpdate], stash: bool) {
    let bar = get_progress_bar(book_files.len() as u64);

    book_files.par_iter().for_each(|file_to_update| {
        let book = Book::new(file_to_update.file_path.path());
        bar.set_prefix(book.name.clone());

        match book.update() {
            UpdateResult::Updated(n) => bar.println(summary!(n, book.name, green)),
            UpdateResult::Skipped => bar.println(summary!("Skip", book.name, blue)),
            UpdateResult::MoreChapterThanSource(n) => {
                bar.println(summary!(-i32::from(n), book.name, red));
                if stash {
                    match book.stash_and_recreate(&file_to_update.stash_path) {
                        Ok(book) => bar.println(summary!("New", book.name, light_green)),
                        Err(e) => eprintln!("{e}"),
                    }
                }
            }
            UpdateResult::Unsupported | UpdateResult::UpToDate => (),
        }
        bar.inc(1);
    });
}

fn get_progress_bar(len: u64) -> ProgressBar {
    let bar = ProgressBar::new(len);
    let template_progress = ProgressStyle::with_template(
        "\n{prefix}\n[{elapsed}/{duration}] {wide_bar} {pos:>3}/{len:3} ({percent}%)\n{msg}",
    )
    .unwrap_or_else(|err| {
        eprintln!("{err}");
        ProgressStyle::default_bar()
    });
    bar.set_style(template_progress);
    bar
}

fn get_book_files(path: &PathBuf, stash_dir: &PathBuf) -> Vec<FileToUpdate> {
    WalkDir::new(path)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter(|e| e.path().parent().is_some_and(|p| *p != *stash_dir))
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map_or(false, |v| v == EPUB))
        .map(|e| FileToUpdate {
            file_path: e,
            stash_path: stash_dir.clone(),
        })
        .collect()
}

fn remove_empty_epub(path: &Path) {
    WalkDir::new(path)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map_or(false, |v| v == EPUB))
        .filter(|e| e.metadata().map(|m| m.len() == 0).unwrap_or(false)) // File is empty
        .for_each(|f| {
            fs::remove_file(f.path()).unwrap_or_else(|_| {
                eprintln!("{} is empty but could not be deleted", f.path().display());
            });
        });
}
