//! File category evaluator for routing by file extension.

use nvisy_dal::AnyDataValue;

use crate::definition::FileCategory;

/// Evaluates file category based on extension.
#[derive(Debug, Clone)]
pub struct FileCategoryEvaluator {
    /// File category to match.
    category: FileCategory,
}

impl FileCategoryEvaluator {
    /// Creates a new file category evaluator.
    pub fn new(category: FileCategory) -> Self {
        Self { category }
    }

    /// Evaluates whether the data matches the file category.
    pub fn evaluate(&self, data: &AnyDataValue) -> bool {
        let ext = match data {
            AnyDataValue::Blob(blob) => blob.path.rsplit('.').next(),
            _ => return false,
        };

        let Some(ext) = ext else {
            return self.category == FileCategory::Other;
        };

        let ext = ext.to_lowercase();
        match self.category {
            FileCategory::Text => {
                matches!(ext.as_str(), "txt" | "md" | "markdown" | "rst" | "text")
            }
            FileCategory::Image => {
                matches!(
                    ext.as_str(),
                    "jpg"
                        | "jpeg"
                        | "png"
                        | "gif"
                        | "bmp"
                        | "webp"
                        | "svg"
                        | "ico"
                        | "tiff"
                        | "tif"
                )
            }
            FileCategory::Audio => {
                matches!(
                    ext.as_str(),
                    "mp3" | "wav" | "flac" | "aac" | "ogg" | "wma" | "m4a"
                )
            }
            FileCategory::Video => {
                matches!(
                    ext.as_str(),
                    "mp4" | "webm" | "avi" | "mov" | "mkv" | "wmv" | "flv" | "m4v"
                )
            }
            FileCategory::Document => {
                matches!(
                    ext.as_str(),
                    "pdf" | "doc" | "docx" | "odt" | "rtf" | "epub"
                )
            }
            FileCategory::Archive => {
                matches!(
                    ext.as_str(),
                    "zip" | "tar" | "gz" | "rar" | "7z" | "bz2" | "xz"
                )
            }
            FileCategory::Spreadsheet => {
                matches!(ext.as_str(), "xls" | "xlsx" | "csv" | "ods" | "tsv")
            }
            FileCategory::Presentation => {
                matches!(ext.as_str(), "ppt" | "pptx" | "odp" | "key")
            }
            FileCategory::Code => {
                matches!(
                    ext.as_str(),
                    "rs" | "py"
                        | "js"
                        | "ts"
                        | "java"
                        | "c"
                        | "cpp"
                        | "h"
                        | "hpp"
                        | "go"
                        | "rb"
                        | "php"
                        | "swift"
                        | "kt"
                        | "scala"
                        | "sh"
                        | "bash"
                        | "zsh"
                        | "sql"
                        | "html"
                        | "css"
                        | "json"
                        | "yaml"
                        | "yml"
                        | "toml"
                        | "xml"
                )
            }
            FileCategory::Other => true,
        }
    }
}

impl From<FileCategory> for FileCategoryEvaluator {
    fn from(category: FileCategory) -> Self {
        Self::new(category)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evaluate_image() {
        let evaluator = FileCategoryEvaluator::new(FileCategory::Image);

        let jpg = AnyDataValue::Blob(nvisy_dal::datatype::Blob::new("photo.jpg", vec![]));
        let png = AnyDataValue::Blob(nvisy_dal::datatype::Blob::new("image.PNG", vec![]));
        let pdf = AnyDataValue::Blob(nvisy_dal::datatype::Blob::new("doc.pdf", vec![]));

        assert!(evaluator.evaluate(&jpg));
        assert!(evaluator.evaluate(&png));
        assert!(!evaluator.evaluate(&pdf));
    }

    #[test]
    fn test_evaluate_document() {
        let evaluator = FileCategoryEvaluator::new(FileCategory::Document);

        let pdf = AnyDataValue::Blob(nvisy_dal::datatype::Blob::new("report.pdf", vec![]));
        let docx = AnyDataValue::Blob(nvisy_dal::datatype::Blob::new("letter.docx", vec![]));
        let txt = AnyDataValue::Blob(nvisy_dal::datatype::Blob::new("notes.txt", vec![]));

        assert!(evaluator.evaluate(&pdf));
        assert!(evaluator.evaluate(&docx));
        assert!(!evaluator.evaluate(&txt));
    }
}
