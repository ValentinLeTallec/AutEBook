use eyre::{bail, eyre, Result};
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::ImageReader;
use scraper::Html;
use std::io::Cursor;
use std::path::Path;
use url::Url;
use webp::Decoder;

use crate::lazy_selector;
use crate::updater::native::epub::FORBIDDEN_CHARACTERS;

lazy_selector!(IMAGE_SELECTOR, "img");

pub fn extract_file_name(url: &str) -> Result<String> {
    extract_file_name_from_url(url)
        .or_else(|| extract_file_name_from_path(url))
        .ok_or_else(|| eyre!("{url} is neither an url nor a path"))
}

fn extract_file_name_from_url(url: &str) -> Option<String> {
    let mut url = Url::parse(url).ok()?;
    url.set_query(None);
    url.set_fragment(None);

    url.path_segments()
        .and_then(std::iter::Iterator::last)
        .map(ToString::to_string)
        .map(|f| f.replace(FORBIDDEN_CHARACTERS, "_"))
}

fn extract_file_name_from_path(path: &str) -> Option<String> {
    Path::new(path)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .map(|f| f.replace(FORBIDDEN_CHARACTERS, "_"))
}

pub fn extract_urls_from_html(body: &Option<String>) -> Vec<String> {
    body.as_ref().map_or_else(Vec::new, |text| {
        Html::parse_fragment(text)
            .select(&IMAGE_SELECTOR)
            .filter_map(|element| element.value().attr("src"))
            .map(std::string::ToString::to_string)
            .collect()
    })
}

pub fn replace_url_with_path(mut body: String) -> String {
    Html::parse_fragment(&body)
        .select(&IMAGE_SELECTOR)
        .filter_map(|element| element.value().attr("src"))
        .filter_map(|src| {
            extract_file_name(src)
                .map(|f| format!("../images/{f}"))
                .map(|new_src| (src, new_src))
                .ok()
        })
        .for_each(|(src, new_src)| body = body.replace(src, &new_src));

    body
}

pub fn resize(bytes: bytes::Bytes) -> Result<Vec<u8>> {
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
    pub fn rezise(&self, bytes: &bytes::Bytes) -> Result<Vec<u8>> {
        let image = match self {
            Self::Webp => Decoder::new(bytes)
                .decode()
                .ok_or_else(|| eyre!("Image is not a valid WebP"))?
                .to_image(),
            Self::Png | Self::Jpeg => ImageReader::new(Cursor::new(&bytes))
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

#[cfg(test)]
mod test {
    use scraper::Selector;

    #[test]
    fn test_selectors() {
        assert!(Selector::parse("img").is_ok());
    }
}
