use crate::api::RoyalRoadApi;
use crate::epub::write_epub;
use clap::Parser;

mod api;
mod epub;
mod xml_ext;

#[derive(Debug, Parser)]
pub struct Args {
    /// The file path to write the EPUB to.  
    /// Must end in .epub for parsing to work correctly in most applications.
    pub output_file: String,
    #[arg(long = "book-id", short = 'b')]
    /// The ID of the book to download. Can be found in the URL of the book.  
    /// Example: https://www.royalroad.com/fiction/12345/my-book has an ID of 12345.
    pub book_id: u32,
    #[arg(long = "ignore-cache")]
    /// Ignore the cache and redownload all chapters.
    pub ignore_cache: bool,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    if let Err(err) = run().await {
        tracing::error!("{:?}", err);
    }
}

async fn run() -> eyre::Result<()> {
    let args = Args::parse();

    // If the path doesn't exist we can't canonicalize it, so we create a dummy file,
    // canonicalize it, and then delete it.
    let created = if std::fs::metadata(&args.output_file).is_err() {
        std::fs::File::create(&args.output_file)?;
        true
    } else {
        false
    };
    let out_file = std::fs::canonicalize(&args.output_file)?;
    if created {
        std::fs::remove_file(&args.output_file)?;
    }

    let api = RoyalRoadApi::new();
    let book = api.get_book(args.book_id, args.ignore_cache).await?;
    write_epub(
        &book,
        out_file.to_str().ok_or(eyre::eyre!(
            "Invalid output folder, path contains non-UTF8 characters"
        ))?,
    )?;
    tracing::info!("Wrote EPUB to {:?}", out_file);

    Ok(())
}
