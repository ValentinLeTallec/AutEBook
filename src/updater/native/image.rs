use color_eyre::eyre::{self, bail, eyre};
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::io::Reader;
use scraper::{Html, Selector};
use std::io::Cursor;
use url::Url;
use webp::Decoder;

use crate::updater::native::epub::FORBIDDEN_CHARACTERS;

pub fn extract_file_name(url: &str) -> eyre::Result<String> {
    let mut url = Url::parse(url)?;
    url.set_query(None);
    url.set_fragment(None);

    Ok(url
        .path_segments()
        .ok_or_else(|| eyre!("Invalid image URL"))?
        .last()
        .ok_or_else(|| eyre!("Invalid image URL"))?
        .to_string()
        .replace(FORBIDDEN_CHARACTERS, "_"))
}

pub fn extract_urls_from_html(body: &Option<String>) -> Vec<String> {
    if let (Ok(selector), Some(text)) = (Selector::parse("img"), body) {
        Html::parse_fragment(text)
            .select(&selector)
            .filter_map(|element| element.value().attr("src"))
            .map(std::string::ToString::to_string)
            .collect()
    } else {
        Vec::new()
    }
}

pub fn replace_url_with_path(mut body: String) -> eyre::Result<String> {
    let parsed = Html::parse_fragment(&body);
    let selector = Selector::parse("img").map_err(|err| eyre!("{err}"))?;

    for element in parsed.select(&selector) {
        if let Some(src) = element.value().attr("src") {
            let new_src = format!("../images/{}", extract_file_name(src)?);
            body = body.replace(src, &new_src);
        }
    }
    Ok(body)
}

pub fn resize(bytes: bytes::Bytes) -> eyre::Result<Vec<u8>> {
    let managed_image_format = ManagedImageFormat::new(&bytes).ok_or_else(|| {
        eyre!("Unsupported inline image format. Please report this as a bug and include the link.")
    })?;

    let buffer: Vec<u8> = match managed_image_format {
        ManagedImageFormat::Html => bail!("Skipping html."),
        ManagedImageFormat::Gif | ManagedImageFormat::Svg => bytes.into(),
        ManagedImageFormat::Png | ManagedImageFormat::Jpeg | ManagedImageFormat::Webp => {
            managed_image_format
                .as_resizable_image()
                .ok_or_else(|| eyre!("Image is not rezisable."))?
                .rezise(&bytes)?
        }
    };

    Ok(buffer)
}

enum ManagedImageFormat {
    Png,
    Jpeg,
    Webp,
    Gif,
    Svg,
    Html,
}
enum ResizableImageFormat {
    Png,
    Jpeg,
    Webp,
}

impl ManagedImageFormat {
    pub fn new(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > 8
            && bytes[0] == 0x89
            && bytes[1] == 0x50
            && bytes[2] == 0x4E
            && bytes[3] == 0x47
            && bytes[4] == 0x0D
            && bytes[5] == 0x0A
            && bytes[6] == 0x1A
            && bytes[7] == 0x0A
        {
            return Some(Self::Png);
        }

        if bytes.len() > 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF {
            return Some(Self::Jpeg);
        }

        if bytes.len() > 11
            && bytes[0] == 0x52
            && bytes[1] == 0x49
            && bytes[2] == 0x46
            && bytes[3] == 0x46
            && bytes[8] == 0x57
            && bytes[9] == 0x45
            && bytes[10] == 0x42
            && bytes[11] == 0x50
        {
            return Some(Self::Webp);
        }

        if bytes.len() > 3
            && bytes[0] == 0x47
            && bytes[1] == 0x49
            && bytes[2] == 0x46
            && bytes[3] == 0x38
        {
            return Some(Self::Gif);
        }

        let text = std::str::from_utf8(bytes).ok()?;

        if text.to_lowercase().trim().starts_with("<?xml")
            || text.to_lowercase().trim().starts_with("<svg")
        {
            return Some(Self::Svg);
        }

        if text.to_lowercase().trim().starts_with("<!doctype html>")
            || text.to_lowercase().trim().starts_with("<html")
        {
            return Some(Self::Html);
        }
        None
    }

    pub const fn as_resizable_image(&self) -> Option<ResizableImageFormat> {
        match self {
            Self::Png => Some(ResizableImageFormat::Png),
            Self::Jpeg => Some(ResizableImageFormat::Jpeg),
            Self::Webp => Some(ResizableImageFormat::Webp),
            Self::Gif | Self::Svg | Self::Html => None,
        }
    }
}

impl ResizableImageFormat {
    /// Resize the image to max width of 600px and re-encode WebP to PNG.
    pub fn rezise(&self, bytes: &bytes::Bytes) -> eyre::Result<Vec<u8>> {
        let image = match self {
            Self::Webp => Decoder::new(bytes)
                .decode()
                .ok_or_else(|| eyre!("Image is not a valid WebP"))?
                .to_image(),
            Self::Png | Self::Jpeg => Reader::new(Cursor::new(&bytes))
                .with_guessed_format()?
                .decode()?,
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

        match self {
            // We write both PNG and WebP as PNG because WebP is not supported by some e-readers.
            Self::Png | Self::Webp => image.write_with_encoder(PngEncoder::new_with_quality(
                Cursor::new(&mut buffer),
                CompressionType::Fast,
                FilterType::Adaptive,
            ))?,
            Self::Jpeg => image
                .write_with_encoder(JpegEncoder::new_with_quality(Cursor::new(&mut buffer), 80))?,
        };
        Ok(buffer)
    }
}
