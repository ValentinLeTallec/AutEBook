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
mod source;
mod updater;

use crate::updater::UpdateResult;
use clap::{CommandFactory, Parser, Subcommand};
use colorful::Colorful;
use eyre::{bail, eyre, Error, Result};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use walkdir::WalkDir;

const EPUB: &str = "epub";

pub static MULTI_PROGRESS: LazyLock<MultiProgress> = LazyLock::new(MultiProgress::new);

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
    },

    /// Copy books in `<STASH_DIR>` while adding a timestamp, then recreates books inplace
    Stash {
        /// The directory where stashed books are stored.
        #[clap(short = 'd', long, default_value = "./stashed", value_hint = clap::ValueHint::DirPath)]
        stash_dir: PathBuf,

        /// List of path to books to be stashed
        #[clap(num_args = 1..)]
        paths: Vec<PathBuf>,
    },

    /// Generate a SHELL completion script and print to stdout
    Completions { shell: clap_complete::Shell },
}

macro_rules! summary {
    ($s:expr, $book_name:expr, $color:ident) => {{
        let prefix = format!("[{:>+4}]", $s).bold().$color();
        format!("{} {:.50}\n", prefix, $book_name)
    }};
}

fn main() {
    let args = Args::parse();
    setup_nb_threads(args.nb_threads);
    let work_dir = args.dir;

    match args.subcommand {
        Commands::Add { urls } => create_books(work_dir.as_path(), &urls),
        Commands::Update { mut paths } => {
            if paths.is_empty() {
                paths.push(work_dir);
            }

            let book_files = get_book_files(&paths);

            update_books(&book_files);
        }
        Commands::Completions { shell } => clap_complete::generate(
            shell,
            &mut Args::command(),
            "autebooks",
            &mut std::io::stdout(),
        ),
        Commands::Stash { stash_dir, paths } => stash_and_recreate(&stash_dir, &paths),
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

fn create_books(dir: &Path, urls: &[String]) {
    let bar = MULTI_PROGRESS.add(get_progress_bar(urls.len() as u64, 1));

    urls.par_iter().for_each(|url| {
        bar.set_prefix(url.clone());

        match source::from_url(url).create(dir, None, url) {
            Ok(title) => bar.println(format!("{title:.50}\n")),
            Err(e) => bar.println(summary!(e, url, red)),
        }
        bar.inc(1);
    });
    bar.finish_and_clear();
}

fn update_books(book_files: &Vec<PathBuf>) {
    let bar = MULTI_PROGRESS.add(get_progress_bar(book_files.len() as u64, 1));

    book_files.par_iter().for_each(|path| {
        let source = source::from_path(path);
        let title = source.get_title(path);

        bar.set_prefix(title.clone());
        match source.update(path) {
            UpdateResult::Updated(n) => bar.println(summary!(n, title, green)),
            #[cfg(feature = "fanficfare")]
            UpdateResult::Skipped => bar.println(summary!("Skip", title, blue)),
            #[cfg(feature = "fanficfare")]
            UpdateResult::MoreChapterThanSource(n) => {
                bar.println(summary!(-i32::from(n), title, red));
            }
            UpdateResult::Unsupported | UpdateResult::UpToDate => (),
            UpdateResult::Error(e) => bar.eprintln(&e),
        }
        bar.inc(1);
    });
    bar.finish_and_clear();
}

#[must_use]
pub fn get_progress_bar(len: u64, show_if_more_than: u64) -> ProgressBar {
    let show = show_if_more_than < len;

    let bar = if show {
        ProgressBar::new(len)
    } else {
        ProgressBar::hidden()
    };
    let template_progress = ProgressStyle::with_template(if show {
        "\n{prefix}\n[{elapsed}/{duration}] {wide_bar} {pos:>3}/{len:3} ({percent}%)\n{msg}"
    } else {
        ""
    })
    .unwrap_or_else(|err| {
        eprintln!("{err}");
        ProgressStyle::default_bar()
    });
    bar.set_style(template_progress);
    bar
}

pub trait ErrorPrint {
    fn eprintln(&self, msg: &Error);
}
impl ErrorPrint for ProgressBar {
    fn eprintln(&self, msg: &Error) {
        let msg = format!("{msg:}").red();
        self.suspend(|| eprintln!("{msg}"));
    }
}
impl ErrorPrint for MultiProgress {
    fn eprintln(&self, msg: &Error) {
        let msg = format!("{msg:}").red();
        self.suspend(|| eprintln!("{msg}"));
    }
}

fn get_book_files(paths: &[PathBuf]) -> Vec<PathBuf> {
    paths
        .iter()
        .flat_map(|path| WalkDir::new(path).into_iter())
        .filter_map(std::result::Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_some_and(|v| v == EPUB))
        .map(|e| e.path().to_owned())
        .collect()
}

fn stash_and_recreate(stash_dir: &Path, paths: &[PathBuf]) {
    let bar = MULTI_PROGRESS.add(get_progress_bar(paths.len() as u64, 1));

    // Create stashing directory
    if let Err(err) = std::fs::create_dir_all(stash_dir) {
        bar.eprintln(&err.into());
        return;
    }

    get_book_files(paths)
        .par_iter()
        .map(|book| -> Result<String> {
            let path_str = book.to_string_lossy();
            let parent_dir = book.parent().unwrap_or_else(|| Path::new("./"));

            let original_filestem = book
                .file_stem()
                .ok_or_else(|| eyre!("No filename for path {path_str}"))?
                .to_string_lossy();

            let stashed_filename = format!(
                "{}_{}.{EPUB}",
                original_filestem,
                chrono::Utc::now().format("%Y-%m-%d_%Hh%M")
            );

            if let Some(url) = source::get_url(book) {
                std::fs::rename(book, stash_dir.join(stashed_filename))?;
                bar.set_prefix(format!("{path_str}"));

                // Creation of the new instance of the book
                source::from_url(&url).create(
                    parent_dir,
                    book.file_name().map(|e| e.to_string_lossy()).as_deref(),
                    &url,
                )
            } else {
                bail!("No url could be found for {path_str}")
            }
        })
        .inspect(|_| bar.inc(1))
        .for_each(|e| match e {
            Ok(title) => bar.println(format!("{title:.50}\n")),
            Err(e) => bar.eprintln(&e),
        });

    bar.finish_and_clear();
}
