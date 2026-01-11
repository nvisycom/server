//! Document format conversion service.
//!
//! Provides functionality to convert documents between different formats
//! such as PDF, DOCX, PNG, and others.

use serde::{Deserialize, Serialize};

/// Supported document formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocumentFormat {
    /// PDF format.
    Pdf,
    /// Microsoft Word (DOCX).
    Docx,
    /// Microsoft Word (legacy DOC).
    Doc,
    /// Plain text.
    Txt,
    /// Markdown.
    Markdown,
    /// HTML.
    Html,
    /// PNG image.
    Png,
    /// JPEG image.
    Jpeg,
    /// WebP image.
    WebP,
    /// TIFF image.
    Tiff,
    /// SVG vector image.
    Svg,
}

impl DocumentFormat {
    /// Returns the MIME type for this format.
    pub fn mime_type(&self) -> &'static str {
        match self {
            Self::Pdf => "application/pdf",
            Self::Docx => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            Self::Doc => "application/msword",
            Self::Txt => "text/plain",
            Self::Markdown => "text/markdown",
            Self::Html => "text/html",
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::WebP => "image/webp",
            Self::Tiff => "image/tiff",
            Self::Svg => "image/svg+xml",
        }
    }

    /// Returns the file extension for this format.
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Docx => "docx",
            Self::Doc => "doc",
            Self::Txt => "txt",
            Self::Markdown => "md",
            Self::Html => "html",
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::WebP => "webp",
            Self::Tiff => "tiff",
            Self::Svg => "svg",
        }
    }

    /// Returns whether this format is an image format.
    pub fn is_image(&self) -> bool {
        matches!(
            self,
            Self::Png | Self::Jpeg | Self::WebP | Self::Tiff | Self::Svg
        )
    }

    /// Returns whether this format is a document format.
    pub fn is_document(&self) -> bool {
        matches!(
            self,
            Self::Pdf | Self::Docx | Self::Doc | Self::Txt | Self::Markdown | Self::Html
        )
    }

    /// Attempts to detect the format from a MIME type.
    pub fn from_mime_type(mime: &str) -> Option<Self> {
        match mime {
            "application/pdf" => Some(Self::Pdf),
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => {
                Some(Self::Docx)
            }
            "application/msword" => Some(Self::Doc),
            "text/plain" => Some(Self::Txt),
            "text/markdown" => Some(Self::Markdown),
            "text/html" => Some(Self::Html),
            "image/png" => Some(Self::Png),
            "image/jpeg" => Some(Self::Jpeg),
            "image/webp" => Some(Self::WebP),
            "image/tiff" => Some(Self::Tiff),
            "image/svg+xml" => Some(Self::Svg),
            _ => None,
        }
    }
}

/// Options for format conversion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionOptions {
    /// Target format.
    pub target_format: DocumentFormat,
    /// Quality for lossy formats (1-100).
    pub quality: u8,
    /// DPI for rasterization.
    pub dpi: u32,
    /// Whether to embed fonts in output.
    pub embed_fonts: bool,
    /// Whether to flatten layers.
    pub flatten_layers: bool,
    /// Page range to convert (None = all pages).
    pub page_range: Option<PageRange>,
}

impl Default for ConversionOptions {
    fn default() -> Self {
        Self {
            target_format: DocumentFormat::Pdf,
            quality: 90,
            dpi: 150,
            embed_fonts: true,
            flatten_layers: false,
            page_range: None,
        }
    }
}

impl ConversionOptions {
    /// Creates options for converting to a specific format.
    pub fn to_format(format: DocumentFormat) -> Self {
        Self {
            target_format: format,
            ..Default::default()
        }
    }

    /// Sets the quality for lossy formats.
    pub fn with_quality(mut self, quality: u8) -> Self {
        self.quality = quality.min(100);
        self
    }

    /// Sets the DPI for rasterization.
    pub fn with_dpi(mut self, dpi: u32) -> Self {
        self.dpi = dpi;
        self
    }

    /// Sets the page range to convert.
    pub fn with_page_range(mut self, range: PageRange) -> Self {
        self.page_range = Some(range);
        self
    }
}

/// A range of pages to convert.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageRange {
    /// First page (1-indexed).
    pub start: u32,
    /// Last page (inclusive, 1-indexed).
    pub end: u32,
}

impl PageRange {
    /// Creates a new page range.
    pub fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    /// Creates a range for a single page.
    pub fn single(page: u32) -> Self {
        Self {
            start: page,
            end: page,
        }
    }

    /// Creates a range from a page to the end.
    pub fn from_page(start: u32) -> Self {
        Self {
            start,
            end: u32::MAX,
        }
    }
}

/// Result of a format conversion.
#[derive(Debug, Clone)]
pub struct ConversionResult {
    /// The converted document data.
    pub data: Vec<u8>,
    /// The output format.
    pub format: DocumentFormat,
    /// Number of pages in the output (if applicable).
    pub page_count: Option<u32>,
}

impl ConversionResult {
    /// Returns the MIME type of the result.
    pub fn mime_type(&self) -> &'static str {
        self.format.mime_type()
    }

    /// Returns the size in bytes.
    pub fn size_bytes(&self) -> usize {
        self.data.len()
    }
}

/// Service for converting documents between formats.
#[derive(Clone, Default)]
pub struct FormatConversionService {
    _private: (),
}

impl FormatConversionService {
    /// Creates a new format conversion service.
    pub fn new() -> Self {
        Self { _private: () }
    }

    /// Converts a document to the specified format.
    pub async fn convert(
        &self,
        data: &[u8],
        _source_format: DocumentFormat,
        options: ConversionOptions,
    ) -> crate::Result<ConversionResult> {
        // TODO: Implement actual format conversion
        // This will require external libraries:
        // - PDF: lopdf, pdf-rs, or pdfium bindings
        // - Images: image crate
        // - Office: unoconv, pandoc, or similar

        // For now, return placeholder
        Ok(ConversionResult {
            data: data.to_vec(),
            format: options.target_format,
            page_count: None,
        })
    }

    /// Checks if a conversion between two formats is supported.
    pub fn is_conversion_supported(&self, source: DocumentFormat, target: DocumentFormat) -> bool {
        // Define supported conversion paths
        match (source, target) {
            // Same format is always "supported" (no-op)
            (s, t) if s == t => true,

            // PDF conversions
            (DocumentFormat::Pdf, DocumentFormat::Png) => true,
            (DocumentFormat::Pdf, DocumentFormat::Jpeg) => true,
            (DocumentFormat::Pdf, DocumentFormat::Txt) => true,

            // Image conversions
            (s, t) if s.is_image() && t.is_image() => true,

            // Document to PDF
            (DocumentFormat::Docx, DocumentFormat::Pdf) => true,
            (DocumentFormat::Html, DocumentFormat::Pdf) => true,
            (DocumentFormat::Markdown, DocumentFormat::Pdf) => true,
            (DocumentFormat::Markdown, DocumentFormat::Html) => true,

            _ => false,
        }
    }

    /// Returns all formats that the source format can be converted to.
    pub fn supported_target_formats(&self, source: DocumentFormat) -> Vec<DocumentFormat> {
        let all_formats = [
            DocumentFormat::Pdf,
            DocumentFormat::Docx,
            DocumentFormat::Txt,
            DocumentFormat::Markdown,
            DocumentFormat::Html,
            DocumentFormat::Png,
            DocumentFormat::Jpeg,
            DocumentFormat::WebP,
            DocumentFormat::Tiff,
        ];

        all_formats
            .into_iter()
            .filter(|&target| self.is_conversion_supported(source, target))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_mime_types() {
        assert_eq!(DocumentFormat::Pdf.mime_type(), "application/pdf");
        assert_eq!(DocumentFormat::Png.mime_type(), "image/png");
    }

    #[test]
    fn test_format_extensions() {
        assert_eq!(DocumentFormat::Pdf.extension(), "pdf");
        assert_eq!(DocumentFormat::Docx.extension(), "docx");
    }

    #[test]
    fn test_is_image() {
        assert!(DocumentFormat::Png.is_image());
        assert!(DocumentFormat::Jpeg.is_image());
        assert!(!DocumentFormat::Pdf.is_image());
    }

    #[test]
    fn test_from_mime_type() {
        assert_eq!(
            DocumentFormat::from_mime_type("application/pdf"),
            Some(DocumentFormat::Pdf)
        );
        assert_eq!(
            DocumentFormat::from_mime_type("image/png"),
            Some(DocumentFormat::Png)
        );
        assert_eq!(DocumentFormat::from_mime_type("unknown/type"), None);
    }

    #[test]
    fn test_conversion_supported() {
        let service = FormatConversionService::new();
        assert!(service.is_conversion_supported(DocumentFormat::Pdf, DocumentFormat::Png));
        assert!(service.is_conversion_supported(DocumentFormat::Png, DocumentFormat::Jpeg));
        assert!(!service.is_conversion_supported(DocumentFormat::Txt, DocumentFormat::Png));
    }

    #[test]
    fn test_page_range() {
        let range = PageRange::new(1, 5);
        assert_eq!(range.start, 1);
        assert_eq!(range.end, 5);

        let single = PageRange::single(3);
        assert_eq!(single.start, 3);
        assert_eq!(single.end, 3);
    }
}
