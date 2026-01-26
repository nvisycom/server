//! File category evaluator for routing by file extension.

use nvisy_dal::datatypes::AnyDataValue;

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
        // Extract path from the value based on data type
        let path: Option<&str> = match data {
            AnyDataValue::Object(obj) => Some(obj.path.as_str()),
            AnyDataValue::Document(doc) => doc.metadata.get("path").and_then(|v| v.as_str()),
            AnyDataValue::Record(rec) => rec
                .columns
                .get("path")
                .or_else(|| rec.columns.get("key"))
                .and_then(|v| v.as_str()),
            _ => None,
        };

        let Some(path) = path else {
            return self.category == FileCategory::Other;
        };

        let Some(ext) = path.rsplit('.').next() else {
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
