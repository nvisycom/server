//! The billing unit a model call metered.

use super::db_enum;

db_enum! {
    /// The billing unit a single model call metered.
    ///
    /// Corresponds to the `USAGE_METER` `PostgreSQL` enum. A call reports an open
    /// set of these — tokens for token-billed models, seconds for speech-to-text,
    /// pages/images for document OCR, characters for per-character services — and
    /// each metered unit is one row paired with its amount. The amount is only
    /// meaningful read together with the meter: `1500` is tokens, seconds, or
    /// pages depending on which this is.
    pub enum UsageMeter = "crate::schema::sql_types::UsageMeter" {
        /// Input/prompt tokens (token-billed models).
        TokensInput = "tokens_input",
        /// Output/completion tokens.
        TokensOutput = "tokens_output",
        /// Total tokens as reported (not necessarily input + output).
        TokensTotal = "tokens_total",
        /// Seconds of media processed (speech-to-text).
        Seconds = "seconds",
        /// Pages processed (document OCR, per page).
        Pages = "pages",
        /// Images processed (vision/OCR, per image).
        Images = "images",
        /// Characters processed (per-character services).
        Characters = "characters",
    }
}
