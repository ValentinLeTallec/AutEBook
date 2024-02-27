use crate::cache::Cache;
use crate::xml_ext::write_elements;
use eyre::bail;
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::io::Reader;
use regex::Regex;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::io::{Cursor, Write};
use std::path::PathBuf;
use url::Url;
use uuid::Uuid;
use webp::Decoder;
use xml::writer::XmlEvent;
use xml::EmitterConfig;

const USER_AGENT: &str = "rr-to-epub <https://github.com/isaac-mcfadyen/rr-to-epub>";

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Book {
    pub id: u32,
    pub title: String,
    pub author: String,
    pub description: String,
    pub date_published: String,
    pub cover_url: String,
    pub cover: Option<Vec<u8>>,
    pub chapters: Vec<Chapter>,

    #[serde(skip)]
    client: reqwest::Client,
}
impl Book {
    pub fn id_from_file(path: PathBuf) -> eyre::Result<Option<u32>> {
        // Open the file as a ZIP.
        let mut reader = zip::ZipArchive::new(std::fs::File::open(path)?)?;

        // Open the file at OEBPS/content.opf.
        let mut content_opf = reader.by_name("OEBPS/content.opf")?;

        // Read the file.
        let mut contents = String::new();
        content_opf.read_to_string(&mut contents)?;

        // Parse as XML.
        let mut parsed = xml::EventReader::new(contents.as_bytes());
        loop {
            let Ok(event) = parsed.next() else {
                return Ok(None);
            };
            match event {
                xml::reader::XmlEvent::StartElement { attributes, .. } => {
                    let is_rr_tag = attributes.iter().any(|v| {
                        v.name.local_name == "name" && v.value == "rr-to-epub:royal-road-id"
                    });
                    if is_rr_tag {
                        // Find the content attribute.
                        let Some(content) =
                            attributes.iter().find(|v| v.name.local_name == "content")
                        else {
                            return Ok(None);
                        };

                        let id = content.value.parse::<u32>()?;
                        return Ok(Some(id));
                    }
                }
                xml::reader::XmlEvent::EndDocument => {
                    // End of document without finding the ID.
                    return Ok(None);
                }
                _ => {}
            }
        }
    }
    pub async fn new(id: u32) -> eyre::Result<Self> {
        // Cover in script tag: window.fictionCover = "...";
        let cover_regex = Regex::new(r#"window\.fictionCover = "(.*)";"#).unwrap();
        // Chapters array in script tag: window.chapters = [...];
        let chapters_regex = Regex::new(r#"window\.chapters = (\[.*]);"#).unwrap();
        let client = reqwest::Client::new();

        let request = client
            .get(format!("https://www.royalroad.com/fiction/{}", id))
            .header("User-Agent", USER_AGENT)
            .send()
            .await?
            .error_for_status()?;
        let response = request.text().await?;

        // Parse book metadata.
        let parsed = Html::parse_document(&response);
        let title_selector = Selector::parse("h1").unwrap();
        let author_selector = Selector::parse("h4 a").unwrap();
        let description_selector = Selector::parse(".description > .hidden-content").unwrap();
        let title = parsed
            .select(&title_selector)
            .next()
            .ok_or(eyre::eyre!("No title found"))?
            .inner_html();
        let author = parsed
            .select(&author_selector)
            .next()
            .ok_or(eyre::eyre!("No author found"))?
            .inner_html();
        let description = parsed
            .select(&description_selector)
            .next()
            .ok_or(eyre::eyre!("No description found"))?
            .inner_html();

        // Parse chapter metadata.
        let cover = cover_regex
            .captures(&response)
            .ok_or(eyre::eyre!("No cover found"))?[1]
            .to_string();
        let chapters = chapters_regex
            .captures(&response)
            .ok_or(eyre::eyre!("No chapters found"))?[1]
            .to_string();
        let chapters: Vec<Chapter> = serde_json::from_str(&chapters)?;

        Ok(Self {
            id,
            cover_url: cover,
            cover: None,
            title,
            author,
            description,
            date_published: chapters.first().unwrap().date.clone(),
            chapters,
            client,
        })
    }
    pub async fn update_cover(&mut self) -> eyre::Result<()> {
        let cover = self
            .client
            .get(&self.cover_url)
            .header("User-Agent", USER_AGENT)
            .send()
            .await?
            .error_for_status()?;
        let bytes = cover.bytes().await?;
        self.cover = Some(bytes.to_vec());
        Ok(())
    }
    pub async fn update_chapter_content(&mut self) -> eyre::Result<()> {
        let num_chapters = self.chapters.len();
        let content_selector = Selector::parse(".chapter-inner.chapter-content").unwrap();

        // Strange selectors are because RR doesn't have a way to tell if the author's note is
        // at the start or the end in the HTML.
        let authors_note_start_selector = Selector::parse("hr + .portlet > .author-note").unwrap();
        let authors_note_end_selector = Selector::parse("div + .portlet > .author-note").unwrap();
        for (index, chapter) in self.chapters.iter_mut().enumerate() {
            tracing::info!(
                "Downloading chapter '{}' ({} of {})",
                chapter.title,
                index + 1,
                num_chapters
            );
            let url = format!("https://www.royalroad.com{}", chapter.url);
            let request = self
                .client
                .get(url)
                .header("User-Agent", USER_AGENT)
                .send()
                .await?
                .error_for_status()?;
            let text = request.text().await?;

            let parsed = Html::parse_document(&text);

            // Parse content.
            let content = parsed
                .select(&content_selector)
                .next()
                .ok_or(eyre::eyre!("No content found"))?
                .inner_html();
            chapter.content = Some(content);

            // Parse starting AN.
            if let Some(authors_note) = parsed.select(&authors_note_start_selector).next() {
                let authors_note = authors_note.inner_html();
                if !authors_note.is_empty() {
                    chapter.authors_note_start = Some(authors_note);
                }
            }
            // Parse ending AN.
            if let Some(authors_note) = parsed.select(&authors_note_end_selector).next() {
                let authors_note = authors_note.inner_html();
                if !authors_note.is_empty() {
                    chapter.authors_note_end = Some(authors_note);
                }
            }

            // Sleep for 0.5 seconds to avoid rate limiting.
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
        Ok(())
    }
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Chapter {
    pub id: u32,
    pub date: String,
    pub slug: String,
    pub title: String,
    pub url: String,
    pub content: Option<String>,

    pub authors_note_start: Option<String>,
    pub authors_note_end: Option<String>,
}

pub async fn write_epub(book: &Book, outfile: Option<String>) -> eyre::Result<()> {
    // Create a temp dir.
    let temp_folder = tempfile::tempdir()?;

    // Choose the filename.
    let outfile = match outfile {
        Some(outfile) => outfile,
        None => format!(
            "{}.epub",
            book.title
                .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "-")
        ),
    };

    // Open the file.
    let epub_path = temp_folder
        .path()
        .join(Uuid::new_v4().to_string())
        .with_extension("epub");
    tracing::debug!("Writing epub to {:?}", epub_path);
    let file = std::fs::File::create(&epub_path)?;
    let mut epub_file = zip::ZipWriter::new(file);

    // Write the mimetype.
    epub_file.start_file("mimetype", zip::write::FileOptions::default())?;
    epub_file.write_all(b"application/epub+zip")?;

    // Write the META-INF folder.
    epub_file.add_directory("META-INF", zip::write::FileOptions::default())?;

    // Write the container.xml file.
    epub_file.start_file("META-INF/container.xml", zip::write::FileOptions::default())?;
    container_xml(book, &mut epub_file)?;

    // Write the content.opf file.
    epub_file.start_file("OEBPS/content.opf", zip::write::FileOptions::default())?;
    content_opf(book, &mut epub_file)?;

    // Write the table of contents (toc.ncx) file.
    epub_file.start_file("OEBPS/toc.ncx", zip::write::FileOptions::default())?;
    toc_ncx(book, &mut epub_file)?;

    // Write each chapter.
    for chapter in book.chapters.iter() {
        // Write the chapter file.
        epub_file.start_file(
            format!("OEBPS/text/{}.xhtml", chapter.id),
            zip::write::FileOptions::default(),
        )?;
        chapter_html(chapter, &mut epub_file)?;

        // Parse and download each inline image in the content, as well as Author's Notes.
        let mut images = parse_images(&chapter.content.clone().unwrap_or("".to_string()))?;
        images.extend(parse_images(
            &chapter.authors_note_start.clone().unwrap_or("".to_string()),
        )?);
        images.extend(parse_images(
            &chapter.authors_note_end.clone().unwrap_or("".to_string()),
        )?);
        if !images.is_empty() {
            tracing::info!("Downloading inline images for chapter '{}'.", chapter.title);
            for url in images.iter() {
                epub_file.start_file(
                    format!("OEBPS/images/{}", extract_image_name(url)?),
                    zip::write::FileOptions::default(),
                )?;
                download_image(book, url.to_string(), &mut epub_file).await?;
            }
        }
    }

    // Write the cover.
    epub_file.start_file(
        "OEBPS/images/cover.jpeg",
        zip::write::FileOptions::default(),
    )?;
    download_image(book, book.cover_url.clone(), &mut epub_file).await?;

    // Write the title page.
    epub_file.start_file("OEBPS/text/title.xhtml", zip::write::FileOptions::default())?;
    title_html(book, &mut epub_file)?;

    // Write the stylesheet.
    epub_file.start_file(
        "OEBPS/styles/stylesheet.css",
        zip::write::FileOptions::default(),
    )?;
    stylesheet(&mut epub_file)?;

    // Finish and copy to user destination.
    epub_file.finish()?;
    tracing::debug!("Copying epub from {:?} to {:?}", epub_path, outfile);
    std::fs::copy(epub_path, &outfile)?;

    tracing::info!("Wrote EPUB to {:?}", outfile);
    Ok(())
}

fn stylesheet(file: &mut impl Write) -> eyre::Result<()> {
    file.write_all(include_bytes!("./assets/styles.css"))?;
    Ok(())
}

fn title_html(book: &Book, file: &mut impl Write) -> eyre::Result<()> {
    let mut xml = EmitterConfig::new()
        .perform_indent(true)
        .create_writer(file);

    // Write the body
    write_elements(
        &mut xml,
        vec![
            XmlEvent::start_element("html")
                .ns("", "http://www.w3.org/1999/xhtml")
                .into(),
            // Write the head.
            XmlEvent::start_element("head").into(),
            XmlEvent::start_element("title").into(),
            XmlEvent::characters(&book.title),
            XmlEvent::end_element().into(),
            XmlEvent::start_element("link")
                .attr("rel", "stylesheet")
                .attr("type", "text/css")
                .attr("href", "../styles/stylesheet.css")
                .into(),
            XmlEvent::end_element().into(),
            XmlEvent::start_element("body").into(),
            // Write the cover.
            XmlEvent::start_element("img")
                .attr("src", "../images/cover.jpeg")
                .attr("alt", "Cover")
                .attr("class", "cover")
                .into(),
            XmlEvent::end_element().into(),
            XmlEvent::start_element("h1").attr("class", "title").into(),
            XmlEvent::characters(&book.title),
            XmlEvent::end_element().into(),
            XmlEvent::start_element("h2").attr("class", "author").into(),
            XmlEvent::characters(&book.author),
            XmlEvent::end_element().into(),
            XmlEvent::end_element().into(),
            XmlEvent::end_element().into(),
        ],
    )?;
    Ok(())
}

fn chapter_html(chapter: &Chapter, file: &mut impl Write) -> eyre::Result<()> {
    let mut xml = EmitterConfig::new().perform_indent(true);
    xml.perform_escaping = false;
    let mut xml = xml.create_writer(file);

    write_elements(
        &mut xml,
        vec![
            XmlEvent::start_element("html")
                .ns("", "http://www.w3.org/1999/xhtml")
                .attr("xml:lang", "en")
                .into(),
            // Write the head.
            XmlEvent::start_element("head").into(),
            XmlEvent::start_element("title").into(),
            XmlEvent::characters(&chapter.title),
            XmlEvent::end_element().into(),
            XmlEvent::start_element("meta")
                .attr("name", "generator")
                .attr("content", "text/html; charset=UTF-8")
                .into(),
            XmlEvent::end_element().into(),
            XmlEvent::start_element("link")
                .attr("href", "../styles/stylesheet.css")
                .attr("rel", "stylesheet")
                .attr("type", "text/css")
                .into(),
            XmlEvent::end_element().into(),
            XmlEvent::end_element().into(),
            // Write the body.
            XmlEvent::start_element("body").into(),
            XmlEvent::start_element("h1")
                .attr("class", "chapter-title")
                .into(),
            XmlEvent::characters(&chapter.title),
            XmlEvent::end_element().into(),
        ],
    )?;

    // Write the starting author's note, if any.
    if let Some(authors_note_start) = chapter.authors_note_start.clone() {
        write_elements(
            &mut xml,
            vec![
                XmlEvent::start_element("div")
                    .attr("class", "authors-note-start")
                    .into(),
                XmlEvent::characters(&rewrite_images(authors_note_start)?),
                XmlEvent::end_element().into(),
            ],
        )?;
    }
    // Write the content.
    if let Some(mut content) = chapter.content.clone() {
        // Remove the font-family: *; from styles.
        let font_family_regex = Regex::new(r#"font-family:.*?;"#).unwrap();
        content = font_family_regex.replace_all(&content, "").to_string();

        // Remove font-weight: normal and font-weight: 400 from styles.
        let font_weight_regex = Regex::new(r#"font-weight:\s?normal"#).unwrap();
        content = font_weight_regex.replace_all(&content, "").to_string();
        let font_weight_regex = Regex::new(r#"font-weight:\s?400"#).unwrap();
        content = font_weight_regex.replace_all(&content, "").to_string();

        // Remove overflow: auto.
        let overflow_regex = Regex::new(r#"overflow:\s?auto"#).unwrap();
        content = overflow_regex.replace_all(&content, "").to_string();

        // Remove any "stolen from Amazon" messages.
        // Please don't use this tool to re-publish authors' works without their permission.
        let messages = include_str!("./assets/messages.txt");
        for message in messages.split('\n') {
            content = content.replace(message, "");
        }

        write_elements(
            &mut xml,
            vec![
                XmlEvent::start_element("div")
                    .attr("class", "chapter-content")
                    .into(),
                // Rewrite the images to be pointing to our downloaded ones.
                XmlEvent::characters(&rewrite_images(content)?),
                XmlEvent::end_element().into(),
            ],
        )?;
    }
    // Write the ending author's note, if any.
    if let Some(authors_note_end) = chapter.authors_note_end.clone() {
        write_elements(
            &mut xml,
            vec![
                XmlEvent::start_element("div")
                    .attr("class", "authors-note-end")
                    .into(),
                XmlEvent::characters(&rewrite_images(authors_note_end)?),
                XmlEvent::end_element().into(),
            ],
        )?;
    }

    // Close elements.
    write_elements(
        &mut xml,
        vec![
            XmlEvent::end_element().into(),
            XmlEvent::end_element().into(),
        ],
    )?;
    Ok(())
}

fn container_xml(_: &Book, file: &mut impl Write) -> eyre::Result<()> {
    let mut xml = EmitterConfig::new()
        .perform_indent(true)
        .create_writer(file);

    write_elements(
        &mut xml,
        vec![
            XmlEvent::start_element("container")
                .attr("version", "1.0")
                .ns("a", "urn:oasis:names:tc:opendocument:xmlns:container")
                .into(),
            XmlEvent::start_element("rootfiles").into(),
            XmlEvent::start_element("rootfile")
                .attr("full-path", "OEBPS/content.opf")
                .attr("media-type", "application/oebps-package+xml")
                .into(),
            XmlEvent::end_element().into(),
            XmlEvent::end_element().into(),
            XmlEvent::end_element().into(),
        ],
    )?;
    Ok(())
}

fn content_opf(book: &Book, file: &mut impl Write) -> eyre::Result<()> {
    let mut xml = EmitterConfig::new()
        .perform_indent(true)
        .create_writer(file);
    write_elements(
        &mut xml,
        vec![
            XmlEvent::start_element("package")
                .ns("", "http://www.idpf.org/2007/opf")
                .attr("version", "3.0")
                .attr("unique-identifier", "bookid")
                .into(),
            XmlEvent::start_element("metadata")
                .ns("dc", "http://purl.org/dc/elements/1.1/")
                .into(),
            XmlEvent::start_element("dc:title").into(),
            XmlEvent::characters(&book.title),
            XmlEvent::end_element().into(),
            XmlEvent::start_element("dc:creator").into(),
            XmlEvent::characters(&book.author),
            XmlEvent::end_element().into(),
            XmlEvent::start_element("dc:description").into(),
            XmlEvent::characters(&book.description),
            XmlEvent::end_element().into(),
            XmlEvent::start_element("dc:date").into(),
            XmlEvent::characters(&book.date_published),
            XmlEvent::end_element().into(),
            XmlEvent::start_element("dc:identifier")
                .attr("id", "bookid")
                .into(),
            XmlEvent::characters(&book.id.to_string()),
            XmlEvent::end_element().into(),
            XmlEvent::start_element("dc:language").into(),
            XmlEvent::characters("en"),
            XmlEvent::end_element().into(),
            XmlEvent::start_element("meta")
                .attr("name", "cover")
                .attr("content", "cover")
                .into(),
            XmlEvent::end_element().into(),
            XmlEvent::start_element("meta")
                .attr("name", "primary-writing-mode")
                .attr("content", "horizontal-lr")
                .into(),
            XmlEvent::end_element().into(),
            XmlEvent::start_element("meta")
                .attr("name", "rr-to-epub:royal-road-id")
                .attr("content", &book.id.to_string())
                .into(),
            XmlEvent::end_element().into(),
            XmlEvent::end_element().into(),
            // Write the manifest.
            XmlEvent::start_element("manifest").into(),
            // Write the title page.
            XmlEvent::start_element("item")
                .attr("id", "title")
                .attr("href", "text/title.xhtml")
                .attr("media-type", "application/xhtml+xml")
                .into(),
            XmlEvent::end_element().into(),
            // Write the cover.
            XmlEvent::start_element("item")
                .attr("id", "cover")
                .attr("href", "images/cover.jpeg")
                .attr("media-type", "image/jpeg")
                .into(),
            XmlEvent::end_element().into(),
            // Write the stylesheet.
            XmlEvent::start_element("item")
                .attr("id", "stylesheet")
                .attr("href", "styles/stylesheet.css")
                .attr("media-type", "text/css")
                .into(),
            XmlEvent::end_element().into(),
            // Write the table of contents.
            XmlEvent::start_element("item")
                .attr("id", "toc")
                .attr("href", "toc.ncx")
                .attr("media-type", "application/xhtml+xml")
                .into(),
            XmlEvent::end_element().into(),
        ],
    )?;

    // Write each chapter.
    for chapter in book.chapters.iter() {
        write_elements(
            &mut xml,
            vec![
                XmlEvent::start_element("item")
                    .attr("id", &format!("{}", &chapter.id))
                    .attr("href", &format!("text/{}.xhtml", &chapter.id))
                    .attr("media-type", "application/xhtml+xml")
                    .into(),
                XmlEvent::end_element().into(),
            ],
        )?;
    }
    write_elements(
        &mut xml,
        vec![
            XmlEvent::end_element().into(),
            // Start the spine.
            XmlEvent::start_element("spine").attr("toc", "ncx").into(),
            // Write the title page entry.
            XmlEvent::start_element("itemref")
                .attr("idref", "title")
                .into(),
            XmlEvent::end_element().into(),
        ],
    )?;
    // For each chapter, write a link.
    for chapter in book.chapters.iter() {
        write_elements(
            &mut xml,
            vec![
                XmlEvent::start_element("itemref")
                    .attr("idref", &format!("{}", &chapter.id))
                    .into(),
                XmlEvent::end_element().into(),
            ],
        )?;
    }
    write_elements(
        &mut xml,
        vec![
            XmlEvent::end_element().into(),
            XmlEvent::end_element().into(),
        ],
    )?;

    Ok(())
}

fn toc_ncx(book: &Book, file: &mut impl Write) -> eyre::Result<()> {
    let mut xml = EmitterConfig::new()
        .perform_indent(true)
        .create_writer(file);

    write_elements(
        &mut xml,
        vec![
            XmlEvent::start_element("ncx")
                .ns("", "http://www.daisy.org/z3986/2005/ncx/")
                .attr("version", "2005-1")
                .into(),
            XmlEvent::start_element("head").into(),
            XmlEvent::start_element("meta")
                .attr("name", "dtb:uid")
                .attr("content", &format!("{}", book.id))
                .into(),
            XmlEvent::end_element().into(),
            XmlEvent::start_element("meta")
                .attr("name", "dtb:depth")
                .attr("content", "2")
                .into(),
            XmlEvent::end_element().into(),
            XmlEvent::start_element("meta")
                .attr("name", "dtb:totalPageCount")
                .attr("content", "0")
                .into(),
            XmlEvent::end_element().into(),
            XmlEvent::start_element("meta")
                .attr("name", "dtb:maxPageNumber")
                .attr("content", "0")
                .into(),
            XmlEvent::end_element().into(),
            XmlEvent::end_element().into(),
            XmlEvent::start_element("docTitle").into(),
            XmlEvent::start_element("text").into(),
            XmlEvent::characters(&book.title),
            XmlEvent::end_element().into(),
            XmlEvent::end_element().into(),
            XmlEvent::start_element("navMap").into(),
            XmlEvent::start_element("navPoint")
                .attr("id", "cover")
                .attr("playOrder", "0")
                .into(),
            XmlEvent::start_element("navLabel").into(),
            XmlEvent::start_element("text").into(),
            XmlEvent::characters("Cover"),
            XmlEvent::end_element().into(),
            XmlEvent::end_element().into(),
            XmlEvent::start_element("content")
                .attr("src", "text/title.xhtml")
                .into(),
            XmlEvent::end_element().into(),
            XmlEvent::end_element().into(),
        ],
    )?;

    // For each chapter, write a link.
    for (index, chapter) in book.chapters.iter().enumerate() {
        write_elements(
            &mut xml,
            vec![
                XmlEvent::start_element("navPoint")
                    .attr("id", &format!("{}", &chapter.id))
                    .attr("playOrder", &format!("{}", index + 1))
                    .into(),
                XmlEvent::start_element("navLabel").into(),
                XmlEvent::start_element("text").into(),
                XmlEvent::characters(&chapter.title),
                XmlEvent::end_element().into(),
                XmlEvent::end_element().into(),
                XmlEvent::start_element("content")
                    .attr("src", &format!("text/{}.xhtml", &chapter.id))
                    .into(),
                XmlEvent::end_element().into(),
                XmlEvent::end_element().into(),
            ],
        )?;
    }

    // Write the end of the document.
    write_elements(
        &mut xml,
        vec![
            XmlEvent::end_element().into(),
            XmlEvent::end_element().into(),
        ],
    )?;

    Ok(())
}

fn extract_image_name(url: &str) -> eyre::Result<String> {
    let mut url = Url::parse(url)?;
    url.set_query(None);
    url.set_fragment(None);

    Ok(url
        .path_segments()
        .ok_or(eyre::eyre!("Invalid image URL"))?
        .last()
        .ok_or(eyre::eyre!("Invalid image URL"))?
        .to_string())
}
fn parse_images(body: &str) -> eyre::Result<Vec<String>> {
    let parsed = Html::parse_fragment(body);
    let selector = Selector::parse("img").unwrap();

    let mut sources = Vec::new();
    for element in parsed.select(&selector) {
        if let Some(src) = element.value().attr("src") {
            sources.push(src.to_string());
        }
    }
    Ok(sources)
}
fn rewrite_images(mut body: String) -> eyre::Result<String> {
    let parsed = Html::parse_fragment(&body);
    let selector = Selector::parse("img").unwrap();

    for element in parsed.select(&selector) {
        if let Some(src) = element.value().attr("src") {
            let new_src = format!("../images/{}", extract_image_name(src)?);
            body = body.replace(src, &new_src);
        }
    }
    Ok(body)
}

fn is_png(bytes: &[u8]) -> bool {
    bytes.len() > 8
        && bytes[0] == 0x89
        && bytes[1] == 0x50
        && bytes[2] == 0x4E
        && bytes[3] == 0x47
        && bytes[4] == 0x0D
        && bytes[5] == 0x0A
        && bytes[6] == 0x1A
        && bytes[7] == 0x0A
}
fn is_jpeg(bytes: &[u8]) -> bool {
    bytes.len() > 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF
}
fn is_webp(bytes: &[u8]) -> bool {
    bytes.len() > 11
        && bytes[0] == 0x52
        && bytes[1] == 0x49
        && bytes[2] == 0x46
        && bytes[3] == 0x46
        && bytes[8] == 0x57
        && bytes[9] == 0x45
        && bytes[10] == 0x42
        && bytes[11] == 0x50
}
fn is_gif(bytes: &[u8]) -> bool {
    bytes.len() > 3 && bytes[0] == 0x47 && bytes[1] == 0x49 && bytes[2] == 0x46 && bytes[3] == 0x38
}
fn is_svg(bytes: &[u8]) -> bool {
    if let Ok(text) = std::str::from_utf8(bytes) {
        text.to_lowercase().trim().starts_with("<?xml")
            || text.to_lowercase().trim().starts_with("<svg")
    } else {
        false
    }
}
fn is_html(bytes: &[u8]) -> bool {
    if let Ok(text) = std::str::from_utf8(bytes) {
        text.to_lowercase().trim().starts_with("<!doctype html>")
            || text.to_lowercase().trim().starts_with("<html")
    } else {
        false
    }
}

async fn download_image(book: &Book, url: String, file: &mut impl Write) -> eyre::Result<()> {
    let filename = extract_image_name(&url)?;
    match Cache::read_inline_image(book, &filename)? {
        Some(image) => {
            file.write_all(&image)?;
        }
        None => {
            let client = reqwest::Client::new();
            let image = client
                .get(&url)
                .header("User-Agent", USER_AGENT)
                .send()
                .await?;
            if !image.status().is_success() {
                // Ignore failed images.
                tracing::warn!(
                    "Failed to download image from URL. This is likely NOT a bug with rr-to-epub. URL: {}",
                    url
                );
                return Ok(());
            }
            let bytes = image.bytes().await?;

            let image = if is_html(&bytes) {
                // Skip HTML pages.
                return Ok(());
            } else if is_gif(&bytes) || is_svg(&bytes) {
                // Directly pass through the image.
                file.write_all(&bytes)?;
                return Ok(());
            } else if is_webp(&bytes) {
                // Decode the image.
                let Some(webp) = Decoder::new(&bytes).decode() else {
                    tracing::warn!(
                        "Failed to decode webp image. Please report this as a bug and include the following URL: {}",
                        url
                    );
                    return Ok(()); // Skip the image.
                };
                webp.to_image()
            } else if is_jpeg(&bytes) || is_png(&bytes) {
                // Decode the image.
                Reader::new(Cursor::new(&bytes))
                    .with_guessed_format()?
                    .decode()?
            } else {
                bail!("Unsupported inline image format. Please report this as a bug and include the following URL: {}", url);
            };

            // Resize to max width of 600px.
            let width = image.width();
            let height = image.height();
            let image = image.resize(
                600,
                600 * height / width,
                image::imageops::FilterType::Lanczos3,
            );

            // Encode the image.
            let mut buffer = Vec::new();
            if is_png(&bytes) || is_webp(&bytes) {
                // We write both PNG and WebP as PNG because WebP is not supported by some e-readers.
                image.write_with_encoder(PngEncoder::new_with_quality(
                    Cursor::new(&mut buffer),
                    CompressionType::Fast,
                    FilterType::Adaptive,
                ))?;
            } else if is_jpeg(&bytes) {
                image.write_with_encoder(JpegEncoder::new_with_quality(
                    Cursor::new(&mut buffer),
                    80,
                ))?;
            } else {
                bail!("Unsupported inline image format. Please report this as a bug and include the following URL: {}", url);
            };

            // Save the image in the cache.
            Cache::write_inline_image(book, &filename, &buffer)?;

            // Write the image to the file.
            file.write_all(&buffer)?;
        }
    }

    Ok(())
}
