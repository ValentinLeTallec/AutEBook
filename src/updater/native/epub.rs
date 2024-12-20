use crate::updater::native::image;
use crate::updater::native::{cache::Cache, xml_ext::write_elements};
use crate::{ErrorPrint, MULTI_PROGRESS};
use chrono::{DateTime, Utc};
use derive_more::derive::Debug;
use epub::doc::EpubDoc;
use eyre::{bail, eyre};
use governor::{DefaultKeyedRateLimiter, Quota, RateLimiter};
use lazy_regex::regex;
use lazy_static::lazy_static;
use reqwest::blocking::{Client, Response};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::Write;
use std::num::NonZeroU32;
use std::path::Path;
use std::sync::OnceLock;
use std::thread;
use std::time::Duration;
use url::Url;
use uuid::Uuid;
use xml::writer::XmlEvent;
use xml::EmitterConfig;
use zip::write::SimpleFileOptions;

const USER_AGENT: &str = "rr-to-epub <https://github.com/isaac-mcfadyen/rr-to-epub>";
pub const FORBIDDEN_CHARACTERS: [char; 13] = [
    '/', '\\', ':', '*', '?', '"', '<', '>', '|', '%', '"', '[', ']',
];

#[allow(clippy::unwrap_used)]
pub fn compile_time_selector(selector: &str) -> scraper::Selector {
    Selector::parse(selector).unwrap()
}

pub fn send_get_request(url: &str) -> std::result::Result<Response, reqwest::Error> {
    static CLIENT_CELL: OnceLock<Client> = OnceLock::new();
    static RATE_LIMITER_CELL: OnceLock<DefaultKeyedRateLimiter<String>> = OnceLock::new();

    #[allow(clippy::unwrap_used)]
    let rate_limiter = RATE_LIMITER_CELL.get_or_init(|| {
        RateLimiter::keyed(
            Quota::per_second(NonZeroU32::new(5u32).unwrap())
                .allow_burst(NonZeroU32::new(1u32).unwrap()),
        )
    });

    let host = Url::parse(url)
        .ok()
        .and_then(|u| u.host().map(|h| h.to_string()))
        .unwrap_or_default();

    while rate_limiter.check_key(&host).is_err() {
        thread::sleep(Duration::from_millis(50));
    }

    CLIENT_CELL
        .get_or_init(Client::new)
        .get(url)
        .header("User-Agent", USER_AGENT)
        .send()
}

lazy_static! {
    static ref CONTENT_SELECTOR: Selector = compile_time_selector(".chapter-inner.chapter-content");

    // Strange selectors are because RR doesn't have a way to tell if the author's note is
    // at the start or the end in the HTML.
    static ref AUTHORS_NOTE_START_SELECTOR: Selector = compile_time_selector("hr + .portlet > .author-note");
    static ref AUTHORS_NOTE_END_SELECTOR: Selector = compile_time_selector("div + .portlet > .author-note");

    static ref TITLE_SELECTOR : Selector = compile_time_selector("h1");
    static ref AUTHOR_SELECTOR : Selector = compile_time_selector("h4 a");
    static ref DESCRIPTION_SELECTOR : Selector = compile_time_selector(".description > .hidden-content");

    static ref TITLE_ELEMENT_SELECTOR : Selector = compile_time_selector("title");
    static ref BODY_ELEMENT_SELECTOR : Selector = compile_time_selector("body");
    static ref META_CHAPTER_URL_SELECTOR : Selector = compile_time_selector("meta[name=chapterurl]");
    static ref META_CHAPTER_DATE_PUBLISHED_SELECTOR : Selector = compile_time_selector("meta[name=published]");
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Book {
    pub id: u32,
    pub url: String,
    pub title: String,
    pub author: String,
    #[debug("{description:50?}")]
    pub description: String,
    pub date_published: String,
    pub cover_url: String,
    pub chapters: Vec<Chapter>,
}
impl Book {
    pub fn new(url: &str) -> eyre::Result<Self> {
        // Cover in script tag: window.fictionCover = "...";
        let cover_regex = regex!(r#"window\.fictionCover = "(.*)";"#);
        // Chapters array in script tag: window.chapters = [...];
        let chapters_regex = regex!(r"window\.chapters = (\[.*]);");

        let request = send_get_request(url)?.error_for_status()?;
        let response = request.text()?;

        // Parse book metadata.
        let parsed = Html::parse_document(&response);
        let title = parsed
            .select(&TITLE_SELECTOR)
            .next()
            .ok_or_else(|| eyre!("No title found"))?
            .inner_html();
        let author = parsed
            .select(&AUTHOR_SELECTOR)
            .next()
            .ok_or_else(|| eyre!("No author found"))?
            .inner_html();
        let description = parsed
            .select(&DESCRIPTION_SELECTOR)
            .next()
            .ok_or_else(|| eyre!("No description found"))?
            .inner_html();

        // Parse chapter metadata.
        let cover = cover_regex
            .captures(&response)
            .ok_or_else(|| eyre!("No cover found"))?[1]
            .to_string();
        let chapters = chapters_regex
            .captures(&response)
            .ok_or_else(|| eyre!("No chapters found"))?[1]
            .to_string();
        let chapters: Vec<Chapter> = serde_json::from_str::<Vec<RoyalRoadChapter>>(&chapters)?
            .iter()
            .map(RoyalRoadChapter::to_chapter)
            .collect();

        Ok(Self {
            id: Self::get_id_from_url(url)?,
            url: url.to_string(),
            cover_url: cover,
            title,
            author,
            description,
            date_published: chapters
                .first()
                .ok_or_else(|| eyre!("No chapter"))?
                .date_published
                .to_rfc3339(),
            chapters,
        })
    }

    pub fn from_path(url: &str, path: &Path) -> eyre::Result<Self> {
        let now = chrono::Utc::now();
        let mut epub_doc = EpubDoc::new(path)?;
        let mut book = Self {
            id: Self::get_id_from_url(url)?,
            url: epub_doc.mdata("source").unwrap_or_default(),
            title: epub_doc.mdata("title").unwrap_or_default(),
            author: epub_doc.mdata("creator").unwrap_or_default(),
            description: epub_doc.mdata("description").unwrap_or_default(),
            date_published: epub_doc.mdata("date").unwrap_or_else(|| now.to_rfc3339()),
            cover_url: String::new(),
            chapters: Vec::new(),
        };

        let image_ids: Vec<_> = epub_doc
            .resources
            .iter()
            .filter(|(_id, (_path, mime))| mime.starts_with("image"))
            .map(|(id, _)| id.clone())
            .collect();

        image_ids
            .iter()
            .filter_map(|id| epub_doc.get_resource(id).map(|(i, _)| (id.clone(), i)))
            .for_each(|(id, image)| {
                if let Err(e) = Cache::write_inline_image(&book, &id, &image) {
                    MULTI_PROGRESS.eprintln(&format!("{e}"));
                };
            });

        while epub_doc.go_next() {
            if epub_doc
                .get_current_id()
                .is_some_and(|id| id == "nav.xhtml")
            {
                continue;
            }

            let xhtml = epub_doc
                .get_current_str()
                .map(|(content, _mime)| content)
                .unwrap_or_default();

            let parsed = Html::parse_document(&xhtml);

            let title = parsed
                .select(&TITLE_ELEMENT_SELECTOR)
                .next()
                .map(|e| e.inner_html())
                .unwrap_or_default();

            let content = parsed
                .select(&BODY_ELEMENT_SELECTOR)
                .next()
                .map(|e| e.inner_html())
                .map(|e| e.replace(&format!("<h3 class=\"fff_chapter_title\">{title}</h3>"), ""))
                .map(|e| e.replace(&format!("<h1 class=\"chapter-title\">{title}</h1>"), ""));

            let url = parsed
                .select(&META_CHAPTER_URL_SELECTOR)
                .next()
                .and_then(|e| e.attr("content"))
                .map(ToString::to_string)
                .unwrap_or_default();

            let date_published = parsed
                .select(&META_CHAPTER_DATE_PUBLISHED_SELECTOR)
                .next()
                .and_then(|e| e.attr("content"))
                .and_then(|d| DateTime::parse_from_rfc3339(d).ok())
                .unwrap_or_else(|| now.into())
                .into();

            let identifier: String = Url::parse(&url)
                .ok()
                .and_then(|url| {
                    url.path_segments()
                        .and_then(|mut x| x.nth(4).map(ToString::to_string))
                })
                .unwrap_or_else(|| {
                    epub_doc
                        .get_current_id()
                        .map(|s| s.replace(".xhtml", ""))
                        .unwrap_or_default()
                })
                .to_string();

            book.chapters.push(Chapter {
                identifier,
                date_published,
                title,
                url,
                content,
                authors_note_start: None,
                authors_note_end: None,
            });
        }
        Ok(book)
    }

    pub fn clone_without_chapters(&self) -> Self {
        Self {
            id: self.id,
            url: self.url.clone(),
            title: self.title.clone(),
            author: self.author.clone(),
            description: self.description.clone(),
            date_published: self.date_published.clone(),
            cover_url: self.cover_url.clone(),
            chapters: Vec::new(),
        }
    }

    fn get_id_from_url(url: &str) -> Result<u32, eyre::Error> {
        let url = Url::parse(url)?;
        let id = url
            .path_segments()
            .and_then(|mut s| s.nth(1))
            .and_then(|f| f.parse().ok())
            .ok_or_else(|| eyre!("Invalid book URL: {url}"))?;
        Ok(id)
    }
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct RoyalRoadChapter {
    pub id: u32,
    pub order: u32,
    pub date: DateTime<Utc>,
    pub title: String,
    pub url: String,
}
impl RoyalRoadChapter {
    pub fn to_chapter(&self) -> Chapter {
        Chapter {
            identifier: self.id.to_string(),
            date_published: self.date,
            title: self.title.clone(),
            url: format!("https://www.royalroad.com{}", self.url),
            content: None,
            authors_note_start: None,
            authors_note_end: None,
        }
    }
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Chapter {
    pub identifier: String,
    pub date_published: DateTime<Utc>,
    pub title: String,
    pub url: String,

    #[debug("{:?}", content.as_ref().map(|s| format!("{s:.100}")))]
    pub content: Option<String>,
    #[debug("{:?}", authors_note_start.as_ref().map(|s| format!("{s:.100}")))]
    pub authors_note_start: Option<String>,
    #[debug("{:?}", authors_note_end.as_ref().map(|s| format!("{s:.100}")))]
    pub authors_note_end: Option<String>,
}

impl PartialEq for Chapter {
    fn eq(&self, other: &Self) -> bool {
        self.identifier.eq(&other.identifier)
    }
}
impl Eq for Chapter {}
impl Chapter {
    pub fn update_chapter_content(&mut self) -> eyre::Result<()> {
        if self.content.is_some() {
            return Ok(());
        }

        let request = send_get_request(&self.url)?.error_for_status()?;
        let text = request.text()?;

        let parsed = Html::parse_document(&text);

        // Parse content.
        let content = parsed
            .select(&CONTENT_SELECTOR)
            .next()
            .ok_or_else(|| eyre!("No content found"))?
            .inner_html();
        self.content = Some(content);

        // Parse starting author note.
        if let Some(authors_note) = parsed.select(&AUTHORS_NOTE_START_SELECTOR).next() {
            let authors_note = authors_note.inner_html();
            if !authors_note.is_empty() {
                self.authors_note_start = Some(authors_note);
            }
        }
        // Parse ending author note.
        if let Some(authors_note) = parsed.select(&AUTHORS_NOTE_END_SELECTOR).next() {
            let authors_note = authors_note.inner_html();
            if !authors_note.is_empty() {
                self.authors_note_end = Some(authors_note);
            }
        }

        Ok(())
    }
}

pub fn write(book: &Book, outfile: Option<String>) -> eyre::Result<String> {
    // Create a temp dir.
    let temp_folder = tempfile::tempdir()?;

    // Choose the filename.
    let outfile = outfile
        .unwrap_or_else(|| format!("{}.epub", book.title.replace(FORBIDDEN_CHARACTERS, "_")));

    // Open the file.
    let epub_path = temp_folder
        .path()
        .join(Uuid::new_v4().to_string())
        .with_extension("epub");
    let file = std::fs::File::create(&epub_path)?;
    let mut epub_file = zip::ZipWriter::new(file);

    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // Write the mimetype.
    epub_file.start_file("mimetype", options)?;
    epub_file.write_all(b"application/epub+zip")?;

    // Write the META-INF folder.
    epub_file.add_directory("META-INF", options)?;

    // Write the container.xml file.
    epub_file.start_file("META-INF/container.xml", options)?;
    container_xml(book, &mut epub_file)?;

    // Write the table of contents for Epub v2 (toc.ncx).
    epub_file.start_file("OEBPS/toc.ncx", options)?;
    toc_ncx(book, &mut epub_file)?;

    // Write the table of contents for Epub v3 (nav.xhtml).
    epub_file.start_file("OEBPS/nav.xhtml", options)?;
    toc_nav(book, &mut epub_file)?;

    // Store image urls
    let mut images: HashSet<String> = HashSet::new();
    // Add the cover.
    images.insert(book.cover_url.clone());

    // Write each chapter.
    for chapter in &book.chapters {
        // Write the chapter file.
        epub_file.start_file(format!("OEBPS/text/{}.xhtml", chapter.identifier), options)?;
        chapter_html(chapter, &mut epub_file)?;

        // Find each inline image in the content, as well as Author's Notes.
        images.extend(image::extract_urls_from_html(&chapter.content));
        images.extend(image::extract_urls_from_html(&chapter.authors_note_start));
        images.extend(image::extract_urls_from_html(&chapter.authors_note_end));
    }

    // Store image filenames to add them to the content_opf
    let mut image_filenames: HashSet<String> = HashSet::new();
    let mut disambiguation_integer: u16 = 0;

    // Download the images and add them to the e-book
    for url in &images {
        let mut filename = match image::extract_file_name(url) {
            Ok(f) => f,
            Err(e) => {
                MULTI_PROGRESS.eprintln(&format!("{e} (URL : {url})"));
                continue;
            }
        };

        // In some case images can have the same name, we prefix it
        // with an integer to disambiguate.
        if image_filenames.contains(&filename) {
            filename = format!("{disambiguation_integer}_{filename}");
            disambiguation_integer += 1;
        }

        match download_image(book, url, &filename) {
            Ok(buffer) => {
                // Write the image to the file.
                epub_file.start_file(format!("OEBPS/images/{filename}"), options)?;
                epub_file.write_all(&buffer)?;

                image_filenames.insert(filename);
            }
            Err(err) => MULTI_PROGRESS.eprintln(&err.to_string()),
        }
    }

    // Write the title page.
    epub_file.start_file("OEBPS/text/title.xhtml", options)?;
    title_html(book, &mut epub_file)?;

    // Write the content.opf file.
    epub_file.start_file("OEBPS/content.opf", options)?;
    content_opf(book, &image_filenames, &mut epub_file)?;

    // Write the stylesheet.
    epub_file.start_file("OEBPS/styles/stylesheet.css", options)?;
    stylesheet(&mut epub_file)?;

    // Finish and copy to user destination.
    epub_file.finish()?;
    std::fs::copy(epub_path, &outfile)?;

    Ok(outfile)
}

fn stylesheet(file: &mut impl Write) -> eyre::Result<()> {
    file.write_all(include_bytes!("./assets/styles.css"))?;
    Ok(())
}

fn title_html(book: &Book, file: &mut impl Write) -> eyre::Result<()> {
    let mut xml = EmitterConfig::new().perform_indent(true);
    xml.perform_escaping = false;
    let mut xml = xml.create_writer(file);
    let cover_file_name = image::extract_file_name(&book.cover_url).unwrap_or_default();

    // Write the body
    #[rustfmt::skip]
    write_elements(
        &mut xml,
        vec![
            XmlEvent::characters("\n<!DOCTYPE html>\n"),
            XmlEvent::start_element("html")
                .ns("", "http://www.w3.org/1999/xhtml")
                .into(),

                // Write the head.
                XmlEvent::start_element("head").into(),
                    XmlEvent::start_element("title").into(),
                        XmlEvent::characters(&book.title),
                    XmlEvent::end_element().into(), // title

                    XmlEvent::start_element("link")
                        .attr("rel", "stylesheet")
                        .attr("type", "text/css")
                        .attr("href", "../styles/stylesheet.css")
                        .into(),
                    XmlEvent::end_element().into(), // link
                XmlEvent::end_element().into(), // head

                XmlEvent::start_element("body").into(),
                    // Write the cover.
                    XmlEvent::start_element("img")
                        .attr("src", &format!("../images/{cover_file_name}"))
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

    #[rustfmt::skip]
    write_elements(
        &mut xml,
        vec![
            XmlEvent::characters("\n<!DOCTYPE html>\n"),
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

                    XmlEvent::start_element("meta")
                        .attr("name", "chapterurl")
                        .attr("content", &chapter.url)
                        .into(),
                    XmlEvent::end_element().into(),

                    XmlEvent::start_element("meta")
                        .attr("name", "published")
                        .attr("content", &chapter.date_published.to_rfc3339())
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
    if let Some(mut authors_note_start) = chapter.authors_note_start.clone() {
        authors_note_start = clean_html(&authors_note_start);
        write_elements(
            &mut xml,
            vec![
                XmlEvent::start_element("div")
                    .attr("class", "authors-note-start")
                    .into(),
                XmlEvent::characters(&image::replace_url_with_path(authors_note_start)),
                XmlEvent::end_element().into(),
            ],
        )?;
    }
    // Write the content.
    if let Some(mut content) = chapter.content.clone() {
        content = clean_html(&content);

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
                XmlEvent::characters(&image::replace_url_with_path(content)),
                XmlEvent::end_element().into(),
            ],
        )?;
    }
    // Write the ending author's note, if any.
    if let Some(mut authors_note_end) = chapter.authors_note_end.clone() {
        authors_note_end = clean_html(&authors_note_end);
        write_elements(
            &mut xml,
            vec![
                XmlEvent::start_element("div")
                    .attr("class", "authors-note-end")
                    .into(),
                XmlEvent::characters(&image::replace_url_with_path(authors_note_end)),
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

fn clean_html(original_content: &str) -> String {
    // Remove the font-family: *; from styles.
    let font_family_regex = regex!(r#"\s*font-family:[^;"]*(?:;\s*|("))"#);
    let mut content = font_family_regex
        .replace_all(original_content, "$1")
        .to_string();
    let font_family_regex = regex!(r#"font-family:[^;"]*""#);
    content = font_family_regex.replace_all(&content, "\"").to_string();

    // Remove font-weight: normal and font-weight: 400 from styles.
    let font_weight_regex = regex!(r#"font-weight:\s?normal"#);
    content = font_weight_regex.replace_all(&content, "").to_string();
    let font_weight_regex = regex!(r#"font-weight:\s?400"#);
    content = font_weight_regex.replace_all(&content, "").to_string();

    // Remove &nbsp;
    let class_regex = regex!(r#" class="[^"]*""#);
    content = class_regex.replace_all(&content, "").to_string();

    // Close tags
    let img_regex = regex!(r#"(<img[^>]*[^/])>"#);
    content = img_regex.replace_all(&content, "$1/>").to_string();
    content = content.replace("<br>", "<br/>");
    content = content.replace("<hr>", "<hr/>");

    // Remove useless whitespaces
    content = content.replace("&nbsp;", " ");
    let whitespace_regex = regex!(r#"<p[^>]*>\s*</p>"#);
    content = whitespace_regex.replace_all(&content, "").to_string();

    // Remove overflow: auto.
    let overflow_regex = regex!(r#"overflow:\s?auto"#);
    content = overflow_regex.replace_all(&content, "").to_string();
    content
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

#[allow(clippy::too_many_lines)]
fn content_opf(
    book: &Book,
    image_filenames: &HashSet<String>,
    file: &mut impl Write,
) -> eyre::Result<()> {
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
            XmlEvent::start_element("dc:source").into(),
            XmlEvent::characters(&book.url),
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
            // Write the nav table.
            XmlEvent::start_element("item")
                .attr("id", "nav")
                .attr("href", "nav.xhtml")
                .attr("media-type", "application/xhtml+xml")
                .attr("properties", "nav")
                .into(),
            XmlEvent::end_element().into(),
        ],
    )?;

    for filename in image_filenames {
        write_elements(
            &mut xml,
            vec![
                // Write the cover.
                XmlEvent::start_element("item")
                    .attr("id", filename)
                    .attr("href", &format!("images/{}", &filename))
                    .attr(
                        "media-type",
                        &format!("image/{}", filename.split('.').last().unwrap_or("jpeg")),
                    )
                    .into(),
                XmlEvent::end_element().into(),
            ],
        )?;
    }

    // Write each chapter.
    for chapter in &book.chapters {
        write_elements(
            &mut xml,
            vec![
                XmlEvent::start_element("item")
                    .attr("id", &chapter.identifier)
                    .attr("href", &format!("text/{}.xhtml", &chapter.identifier))
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
    for chapter in &book.chapters {
        write_elements(
            &mut xml,
            vec![
                XmlEvent::start_element("itemref")
                    .attr("idref", &chapter.identifier)
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

fn toc_nav(book: &Book, file: &mut impl Write) -> eyre::Result<()> {
    let mut xml = EmitterConfig::new().perform_indent(true);
    xml.perform_escaping = false;
    let mut xml = xml.create_writer(file);

    #[rustfmt::skip]
    write_elements(
        &mut xml,
        vec![
            XmlEvent::characters("\n<!DOCTYPE html>\n"),
            XmlEvent::start_element("html")
                .ns("", "http://www.w3.org/1999/xhtml")
                .attr("xmlns:epub", "http://www.idpf.org/2007/ops")
                .attr("lang", "en")
                .attr("xml:lang", "en")
                .into(),

            XmlEvent::start_element("head").into(),
                XmlEvent::start_element("title").into(),
                    XmlEvent::characters("ePub NAV"),
                XmlEvent::end_element().into(),

                XmlEvent::start_element("meta")
                    .attr("charset", "utf-8")
                    .into(),
                XmlEvent::end_element().into(),
            XmlEvent::end_element().into(),

            XmlEvent::start_element("body")
                .attr("epub:type", "frontmatter")
                .into(),

                XmlEvent::start_element("nav")
                    .attr("epub:type","toc")
                    .attr( "id","toc")
                    .attr( "role","doc-toc")
                    .into(),
                    XmlEvent::start_element("h1").into(),
                        XmlEvent::characters("Table of Contents"),
                    XmlEvent::end_element().into(),

                    XmlEvent::start_element("ol").into(),
        ],
    )?;

    // Write each chapter.
    for chapter in &book.chapters {
        write_elements(
            &mut xml,
            vec![
                XmlEvent::start_element("li").into(),
                XmlEvent::start_element("a")
                    .attr("href", &format!("text/{}.xhtml", &chapter.identifier))
                    .into(),
                XmlEvent::characters(&chapter.title),
                XmlEvent::end_element().into(),
                XmlEvent::end_element().into(),
            ],
        )?;
    }
    write_elements(
        &mut xml,
        vec![
            XmlEvent::end_element().into(),
            XmlEvent::end_element().into(),
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
                    .attr("id", &chapter.identifier)
                    .attr("playOrder", &format!("{}", index + 1))
                    .into(),
                XmlEvent::start_element("navLabel").into(),
                XmlEvent::start_element("text").into(),
                XmlEvent::characters(&chapter.title),
                XmlEvent::end_element().into(),
                XmlEvent::end_element().into(),
                XmlEvent::start_element("content")
                    .attr("src", &format!("text/{}.xhtml", &chapter.identifier))
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

pub fn download_image(book: &Book, url: &str, filename: &str) -> eyre::Result<Vec<u8>> {
    // If the image is in the cache, directly use it.
    if let Some(image) = Cache::read_inline_image(book, filename)? {
        return Ok(image.into());
    }

    let image = send_get_request(url)?;

    if !image.status().is_success() {
        // Ignore failed images.
        bail!(
            "Failed to download image from URL. This is likely NOT a bug with rr-to-epub. URL: {}",
            url
        );
    }

    let buffer = image::resize(image.bytes()?).map_err(|err| eyre!("{err} URL: {url}"))?;

    // Save the image in the cache.
    Cache::write_inline_image(book, filename, &buffer)?;

    Ok(buffer)
}

#[cfg(test)]
mod test {
    use crate::updater::native::epub::clean_html;

    #[test]
    fn clean_font_familly_1() {
        // Prepare
        let content = "<span style=\"color: rgba(0, 235, 255, 1); font-family: consolas, terminal, monaco\">txt</span>";

        // Act
        let actual = clean_html(content);

        // Assert
        let expected = String::from("<span style=\"color: rgba(0, 235, 255, 1);\">txt</span>");
        assert_eq!(actual, expected);
    }

    #[test]
    fn clean_font_familly_2() {
        // Prepare
        let content = "<span style=\"font-family: consolas, terminal, monaco; color: rgba(0, 235, 255, 1)\">txt</span>";

        // Act
        let actual = clean_html(content);

        // Assert
        let expected = String::from("<span style=\"color: rgba(0, 235, 255, 1)\">txt</span>");
        assert_eq!(actual, expected);
    }

    #[test]
    fn clean_nbsp() {
        // Prepare
        let content = "<p class=\"cnM5NDA4MTVmMmRlNzQ1ZjI5YmRmZDcxYjgxYTc5NGYx\" style=\"text-align: center\">&nbsp;</p>";

        // Act
        let actual = clean_html(content);

        // Assert
        let expected = String::new();
        assert_eq!(actual, expected);
    }

    #[test]
    fn close_img_tag() {
        // Prepare
        let content = "<img src=\"https://site.com/img.gif\" alt=\"image\">";

        // Act
        let actual = clean_html(content);

        // Assert
        let expected = String::from("<img src=\"https://site.com/img.gif\" alt=\"image\"/>");
        assert_eq!(actual, expected);
    }

    #[test]
    fn dont_break_closed_img_tag() {
        // Prepare
        let content = "<img src=\"https://site.com/img.gif\" alt=\"image\"/>";

        // Act
        let actual = clean_html(content);

        // Assert
        let expected = String::from("<img src=\"https://site.com/img.gif\" alt=\"image\"/>");
        assert_eq!(actual, expected);
    }
}
