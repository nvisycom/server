//! User agent parsing service.
//!
//! This module provides user agent string parsing to extract human-readable
//! browser/application names for use as session token names.

use std::sync::Arc;

/// Maximum length for parsed token names.
const TOKEN_NAME_MAX_LENGTH: usize = 64;

/// User agent parsing service.
///
/// Parses user agent strings to extract human-readable browser/application
/// names and versions for use as session token identifiers.
#[derive(Clone)]
pub struct UserAgentParser {
    parser: Arc<woothee::parser::Parser>,
}

impl std::fmt::Debug for UserAgentParser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UserAgentParser").finish_non_exhaustive()
    }
}

impl UserAgentParser {
    /// Creates a new instance of the [`UserAgentParser`] service.
    pub fn new() -> Self {
        Self {
            parser: Arc::new(woothee::parser::Parser::new()),
        }
    }

    /// Parses a user agent string and returns a human-readable token name.
    ///
    /// Extracts the browser/application name, version, OS, and device category
    /// from the user agent, falling back to "Unknown" if parsing fails. The
    /// result is truncated to 64 characters.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let parser = UserAgentParser::new();
    ///
    /// // Chrome on macOS
    /// let name = parser.parse("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) ... Chrome/120.0.0.0 ...");
    /// assert_eq!(name, "Chrome 120 on macOS (Desktop)");
    ///
    /// // Safari on iOS
    /// let name = parser.parse("Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) ... Safari/604.1");
    /// assert_eq!(name, "Safari 17 on iOS (Mobile)");
    /// ```
    pub fn parse(&self, user_agent: &str) -> String {
        let name = match self.parser.parse(user_agent) {
            Some(result) => {
                let mut parts = Vec::with_capacity(4);

                // Browser name and version
                let browser = result.name;
                let version = result.version;
                if version.is_empty() || version == woothee::woothee::VALUE_UNKNOWN {
                    parts.push(browser.to_string());
                } else {
                    // Use only major version for brevity
                    let major_version = version.split('.').next().unwrap_or(version);
                    parts.push(format!("{} {}", browser, major_version));
                }

                // OS
                let os = result.os;
                if !os.is_empty() && os != woothee::woothee::VALUE_UNKNOWN {
                    parts.push(format!("on {}", os));
                }

                // Device category
                let category = result.category;
                if !category.is_empty() && category != woothee::woothee::VALUE_UNKNOWN {
                    let device = match category {
                        "pc" => "Desktop",
                        "smartphone" => "Mobile",
                        "mobilephone" => "Mobile",
                        "tablet" => "Mobile",
                        "crawler" => "Bot",
                        _ => "Other",
                    };
                    parts.push(format!("({})", device));
                }

                parts.join(" ")
            }
            None => "UNKNOWN".to_string(),
        };

        truncate(&name, TOKEN_NAME_MAX_LENGTH)
    }
}

impl Default for UserAgentParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Truncates a string to a maximum length, ensuring valid UTF-8 boundaries.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        s.chars().take(max_len).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_chrome_macos_user_agent() {
        let parser = UserAgentParser::new();
        let ua = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
        let name = parser.parse(ua);
        assert_eq!(name, "Chrome 120 on Mac OSX (Desktop)");
    }

    #[test]
    fn parse_firefox_windows_user_agent() {
        let parser = UserAgentParser::new();
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:121.0) Gecko/20100101 Firefox/121.0";
        let name = parser.parse(ua);
        assert_eq!(name, "Firefox 121 on Windows 10 (Desktop)");
    }

    #[test]
    fn parse_safari_ios_user_agent() {
        let parser = UserAgentParser::new();
        let ua = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1";
        let name = parser.parse(ua);
        assert!(name.contains("Safari"));
        assert!(name.contains("Mobile"));
    }

    #[test]
    fn truncates_long_names() {
        let long_string = "A".repeat(100);
        let truncated = truncate(&long_string, TOKEN_NAME_MAX_LENGTH);
        assert_eq!(truncated.len(), TOKEN_NAME_MAX_LENGTH);
    }

    #[test]
    fn respects_max_length() {
        let parser = UserAgentParser::new();
        let ua = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
        let name = parser.parse(ua);
        assert!(name.len() <= TOKEN_NAME_MAX_LENGTH);
    }
}
