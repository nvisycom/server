//! Record data type for relational data.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::DataType;

/// A record representing a row in a relational table.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Record {
    /// Column values keyed by column name.
    pub columns: HashMap<String, Value>,
}

impl DataType for Record {}
