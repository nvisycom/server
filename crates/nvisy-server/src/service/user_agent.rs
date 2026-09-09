//! User agent parsing service.
//!
//! This module provides user agent string parsing to extract human-readable
//! browser/application names for use as session token names.

use std::fmt;
use std::sync::Arc;

use woothee::parser::Parser;
use woothee::woothee::VALUE_UNKNOWN;

/// Maximum length for parsed token names.
const TOKEN_NAME_MAX_LENGTH: usize = 64;

/// User agent parsing service.
///
/// Parses user agent strings to extract human-readable browser/application
/// names and versions for use as session token identifiers.
#[derive(Clone)]
pub struct UserAgentParser {
    parser: Arc<Parser>,
}

impl fmt::Debug for UserAgentParser {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UserAgentParser").finish_non_exhaustive()
    }
}

impl UserAgentParser {
    /// Creates a new instance of the [`UserAgentParser`] service.
    pub fn new() -> Self {
        Self {
            parser: Arc::new(Parser::new()),
        }
    }

    /// Parses a user agent string and returns a human-readable token name.
    ///
    /// A recognized browser becomes a label like `Chrome 120 on Mac OSX
    /// (Desktop)`. A user agent that is not a browser — a non-browser client such
    /// as the SDK, whose UA (`@nvisy/sdk/1.2.3`) already identifies it — falls
    /// back to the raw user agent rather than a useless placeholder. An empty user
    /// agent, which carries nothing to show, becomes `UNKNOWN`. The result is
    /// truncated to 64 characters.
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
    /// // A non-browser client keeps its own identifier.
    /// let name = parser.parse("@nvisy/sdk/1.2.3");
    /// assert_eq!(name, "@nvisy/sdk/1.2.3");
    /// ```
    pub fn parse(&self, user_agent: &str) -> String {
        let label = self.browser_label(user_agent).unwrap_or_else(|| {
            // Not a recognized browser. The raw user agent is more useful than a
            // placeholder (it identifies a non-browser client, e.g. the SDK);
            // only a genuinely empty one has nothing to name.
            let trimmed = user_agent.trim();
            if trimmed.is_empty() {
                "UNKNOWN".to_string()
            } else {
                trimmed.to_string()
            }
        });

        truncate(&label, TOKEN_NAME_MAX_LENGTH)
    }

    /// The composed browser label (`name [version] [on os] [(device)]`) for a
    /// recognized browser user agent, or `None` when the user agent is not a
    /// browser or carries no browser fields at all.
    fn browser_label(&self, user_agent: &str) -> Option<String> {
        let result = self.parser.parse(user_agent)?;
        let mut parts = Vec::with_capacity(4);

        // Browser name and version. A parse with no usable name is treated as
        // "not a browser" so the caller falls back to the raw user agent.
        let browser = result.name;
        if browser.is_empty() || browser == VALUE_UNKNOWN {
            return None;
        }
        let version = result.version;
        if version.is_empty() || version == VALUE_UNKNOWN {
            parts.push(browser.to_string());
        } else {
            // Use only the major version for brevity.
            let major_version = version.split('.').next().unwrap_or(version);
            parts.push(format!("{browser} {major_version}"));
        }

        // OS
        let os = result.os;
        if !os.is_empty() && os != VALUE_UNKNOWN {
            parts.push(format!("on {os}"));
        }

        // Device category
        let category = result.category;
        if !category.is_empty() && category != VALUE_UNKNOWN {
            let device = match category {
                "pc" => "Desktop",
                "smartphone" | "mobilephone" | "tablet" => "Mobile",
                "crawler" => "Bot",
                _ => "Other",
            };
            parts.push(format!("({device})"));
        }

        Some(parts.join(" "))
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

    /// Our formatting composes woothee's fields into a "<name> <ver> on <os>
    /// (<category>)" label.
    #[test]
    fn parse_formats_our_label() {
        let parser = UserAgentParser::new();
        let ua = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
        assert_eq!(parser.parse(ua), "Chrome 120 on Mac OSX (Desktop)");
    }

    /// A non-browser client (e.g. the SDK) keeps its own user agent as the label,
    /// rather than the useless "UNKNOWN" a browser parser would otherwise yield.
    #[test]
    fn parse_falls_back_to_the_raw_user_agent_for_a_non_browser() {
        let parser = UserAgentParser::new();
        assert_eq!(parser.parse("@nvisy/sdk/1.2.3"), "@nvisy/sdk/1.2.3");
    }

    /// An empty (or whitespace-only) user agent has nothing to name.
    #[test]
    fn parse_is_unknown_only_for_an_empty_user_agent() {
        let parser = UserAgentParser::new();
        assert_eq!(parser.parse(""), "UNKNOWN");
        assert_eq!(parser.parse("   "), "UNKNOWN");
    }

    /// Our length cap keeps token names within the column limit.
    #[test]
    fn truncate_caps_length() {
        let long = "A".repeat(100);
        assert_eq!(
            truncate(&long, TOKEN_NAME_MAX_LENGTH).len(),
            TOKEN_NAME_MAX_LENGTH
        );
    }
}
