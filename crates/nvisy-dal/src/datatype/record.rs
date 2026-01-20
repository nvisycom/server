//! Record data type for relational data.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::DataType;

/// A record representing a row in a relational table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    /// Column values keyed by column name.
    pub columns: HashMap<String, serde_json::Value>,
}

impl Record {
    /// Creates a new empty record.
    pub fn new() -> Self {
        Self {
            columns: HashMap::new(),
        }
    }

    /// Creates a record from columns.
    pub fn from_columns(columns: HashMap<String, serde_json::Value>) -> Self {
        Self { columns }
    }

    /// Sets a column value.
    pub fn set(mut self, column: impl Into<String>, value: impl Into<serde_json::Value>) -> Self {
        self.columns.insert(column.into(), value.into());
        self
    }

    /// Gets a column value.
    pub fn get(&self, column: &str) -> Option<&serde_json::Value> {
        self.columns.get(column)
    }
}

impl Default for Record {
    fn default() -> Self {
        Self::new()
    }
}

impl DataType for Record {
    const TYPE_ID: &'static str = "record";

    fn data_type_id() -> super::DataTypeId {
        super::DataTypeId::Record
    }
}
