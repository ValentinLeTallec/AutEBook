use crate::api::RoyalRoadApi;
use crate::epub::write_epub;
use clap::Parser;
use epub::Book;
use url::Url;

mod api;
mod cache;
mod epub;
mod xml_ext;

#[derive(Debug, Parser)]
pub struct DownloadArgs {
    /// The file path to write the EPUB to. Defaults to <book-title>.epub.
    /// Must end in .epub for parsing to work correctly in most applications.
    pub output_file: Option<String>,

    #[arg(long = "book-id", short = 'b')]
    /// The ID of the book to download.  One of --book-id or --book-url is required.
    pub book_id: Option<u32>,

    #[arg(long = "book-url", short = 'u')]
    /// The URL of the book to download. One of --book-id or --book-url is required.
    pub book_url: Option<String>,
}

#[derive(Debug, Parser)]
pub struct UpdateArgs {
    /// The folder of books to update, or the path to a single book file.
    pub folder_or_file: String,
}

#[derive(Debug, Parser)]
pub struct GlobalArgs {
    #[arg(long = "ignore-cache", global = true)]
    /// Ignore the cache and redownload all chapters even if the book wasn't modified.
    pub ignore_cache: bool,
}

#[derive(Debug, Parser)]
pub enum Command {
    #[clap(name = "download", about = "Download a book from Royal Road.")]
    Download(DownloadArgs),
    #[clap(name = "update", about = "Update all books in a folder.")]
    Update(UpdateArgs),
}

#[derive(Debug, Parser)]
pub struct App {
    #[clap(subcommand)]
    command: Command,

    #[clap(flatten)]
    global: GlobalArgs,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    if let Err(err) = run().await {
        tracing::error!("{:?}", err);
    }
}

async fn run() -> eyre::Result<()> {
    let app = App::parse();
    let command = app.command;
    match command {
        Command::Download(args) => download(app.global, args).await?,
        Command::Update(args) => update(app.global, args).await?,
    }

    Ok(())
}

async fn download(global_args: GlobalArgs, args: DownloadArgs) -> eyre::Result<()> {
    let id = match (args.book_id, args.book_url) {
        (Some(id), _) => id,
        (None, Some(url)) => {
            let Ok(url) = Url::parse(&url) else {
                tracing::error!("Invalid book URL: {}", url);
                std::process::exit(1);
            };
            let Ok(id) = url
                .path_segments()
                .and_then(|mut s| s.nth(1))
                .unwrap()
                .parse()
            else {
                tracing::error!("Invalid book URL: {}", url);
                std::process::exit(1);
            };
            id
        }
        _ => {
            tracing::error!("One of --book-id or --book-url is required.");
            std::process::exit(1);
        }
    };

    let api = RoyalRoadApi::new();
    let book = api.get_book(id, global_args.ignore_cache).await?;
    write_epub(&book, args.output_file).await?;
    Ok(())
}
async fn update(global_args: GlobalArgs, args: UpdateArgs) -> eyre::Result<()> {
    let api = RoyalRoadApi::new();

    let Ok(stat) = std::fs::metadata(&args.folder_or_file) else {
        tracing::error!(
            "Folder or file \"{}\" does not exist, aborting.",
            args.folder_or_file
        );
        return Ok(());
    };
    if stat.is_file() {
        let file_name = args.folder_or_file;
        if !file_name.ends_with(".epub") {
            tracing::error!(
                "File \"{}\" is not an EPUB file or does not have the .epub extension, aborting.",
                file_name
            );
            return Ok(());
        }

        let path = std::path::Path::new(&file_name);
        let path = path.canonicalize()?;
        let id = Book::id_from_file(path)?;
        if let Some(id) = id {
            tracing::info!("Found book file \"{}\", updating.", file_name);
            let book = api.get_book(id, global_args.ignore_cache).await?;
            write_epub(&book, Some(file_name)).await?;
        } else {
            tracing::error!("Book file at \"{}\" is unmanaged, aborting.", file_name);
        }
    } else {
        let list = std::fs::read_dir(&args.folder_or_file)?;
        for file in list {
            let file = file?;
            let file_name = file
                .file_name()
                .into_string()
                .map_err(|_| eyre::eyre!("Invalid file name: {:?}", file.file_name()))?;
            if !file_name.ends_with(".epub") {
                continue;
            }
            let id = Book::id_from_file(file.path())?;
            if let Some(id) = id {
                tracing::info!("Found book file \"{}\", updating.", file_name);
                let book = api.get_book(id, global_args.ignore_cache).await?;
                write_epub(&book, Some(file_name)).await?;
            } else {
                tracing::warn!("Found unmanaged book file \"{}\", skipping.", file_name,);
            }
        }
    }
    Ok(())
}
