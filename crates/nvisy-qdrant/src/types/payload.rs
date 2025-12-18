//! Payload-related types for Qdrant operations.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
#[cfg(feature = "utoipa")]
use utoipa::ToSchema;

use crate::error::{QdrantError, QdrantResult};

/// Represents payload data associated with a point in Qdrant.
///
/// Payload is a key-value map that stores metadata and attributes for vector points.
/// Values can be strings, numbers, booleans, arrays, or nested objects.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct Payload {
    /// The internal payload data
    #[serde(flatten)]
    data: HashMap<String, serde_json::Value>,
}

impl Payload {
    /// Create a new empty payload
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    /// Create a payload from a HashMap
    pub fn from_map(data: HashMap<String, serde_json::Value>) -> Self {
        Self { data }
    }

    /// Insert a key-value pair into the payload
    pub fn insert(
        &mut self,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Option<serde_json::Value> {
        self.data.insert(key.into(), value.into())
    }

    /// Insert a key-value pair and return self for chaining
    pub fn with(mut self, key: impl Into<String>, value: impl Into<serde_json::Value>) -> Self {
        self.insert(key, value);
        self
    }

    /// Get a value from the payload
    pub fn get(&self, key: &str) -> Option<&serde_json::Value> {
        self.data.get(key)
    }

    /// Get a mutable reference to a value in the payload
    pub fn get_mut(&mut self, key: &str) -> Option<&mut serde_json::Value> {
        self.data.get_mut(key)
    }

    /// Remove a key from the payload
    pub fn remove(&mut self, key: &str) -> Option<serde_json::Value> {
        self.data.remove(key)
    }

    /// Check if the payload contains a key
    pub fn contains_key(&self, key: &str) -> bool {
        self.data.contains_key(key)
    }

    /// Check if the payload is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Get the number of key-value pairs in the payload
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Get all keys in the payload
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.data.keys()
    }

    /// Get all values in the payload
    pub fn values(&self) -> impl Iterator<Item = &serde_json::Value> {
        self.data.values()
    }

    /// Get an iterator over key-value pairs
    pub fn iter(&self) -> impl Iterator<Item = (&String, &serde_json::Value)> {
        self.data.iter()
    }

    /// Get a mutable iterator over key-value pairs
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&String, &mut serde_json::Value)> {
        self.data.iter_mut()
    }

    /// Clear all data from the payload
    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// Extend the payload with another payload's data
    pub fn extend(&mut self, other: Payload) {
        self.data.extend(other.data);
    }

    /// Merge another payload into this one, overwriting existing keys
    pub fn merge(&mut self, other: &Payload) {
        for (key, value) in &other.data {
            self.data.insert(key.clone(), value.clone());
        }
    }

    /// Get a typed value from the payload
    pub fn get_typed<T>(&self, key: &str) -> QdrantResult<Option<T>>
    where
        T: for<'de> serde::Deserialize<'de>,
    {
        match self.get(key) {
            Some(value) => {
                let typed_value = serde_json::from_value(value.clone()).map_err(|e| {
                    QdrantError::payload_error(format!(
                        "Failed to deserialize payload value: {}",
                        e
                    ))
                })?;
                Ok(Some(typed_value))
            }
            None => Ok(None),
        }
    }

    /// Get a string value from the payload
    pub fn get_string(&self, key: &str) -> Option<&str> {
        self.get(key)?.as_str()
    }

    /// Get an integer value from the payload
    pub fn get_i64(&self, key: &str) -> Option<i64> {
        self.get(key)?.as_i64()
    }

    /// Get a float value from the payload
    pub fn get_f64(&self, key: &str) -> Option<f64> {
        self.get(key)?.as_f64()
    }

    /// Get a boolean value from the payload
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get(key)?.as_bool()
    }

    /// Get an array value from the payload
    pub fn get_array(&self, key: &str) -> Option<&Vec<serde_json::Value>> {
        self.get(key)?.as_array()
    }

    /// Get an object value from the payload
    pub fn get_object(&self, key: &str) -> Option<&serde_json::Map<String, serde_json::Value>> {
        self.get(key)?.as_object()
    }

    /// Insert a string value
    pub fn insert_string(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.insert(key, serde_json::Value::String(value.into()));
    }

    /// Insert an integer value
    pub fn insert_i64(&mut self, key: impl Into<String>, value: i64) {
        self.insert(key, serde_json::Value::Number(value.into()));
    }

    /// Insert a float value
    pub fn insert_f64(&mut self, key: impl Into<String>, value: f64) -> QdrantResult<()> {
        let number = serde_json::Number::from_f64(value)
            .ok_or_else(|| QdrantError::payload_error(format!("Invalid f64 value: {}", value)))?;
        self.insert(key, serde_json::Value::Number(number));
        Ok(())
    }

    /// Insert a boolean value
    pub fn insert_bool(&mut self, key: impl Into<String>, value: bool) {
        self.insert(key, serde_json::Value::Bool(value));
    }

    /// Insert an array value
    pub fn insert_array(&mut self, key: impl Into<String>, value: Vec<serde_json::Value>) {
        self.insert(key, serde_json::Value::Array(value));
    }

    /// Insert an object value
    pub fn insert_object(
        &mut self,
        key: impl Into<String>,
        value: serde_json::Map<String, serde_json::Value>,
    ) {
        self.insert(key, serde_json::Value::Object(value));
    }

    /// Convert to Qdrant's internal payload representation
    pub fn into_qdrant_payload(self) -> HashMap<String, qdrant_client::qdrant::Value> {
        self.data
            .into_iter()
            .map(|(k, v)| (k, json_value_to_qdrant_value(v)))
            .collect()
    }

    /// Create from Qdrant's internal payload representation
    pub fn from_qdrant_payload(payload: HashMap<String, qdrant_client::qdrant::Value>) -> Self {
        let data = payload
            .into_iter()
            .map(|(k, v)| (k, qdrant_value_to_json_value(v)))
            .collect();
        Self { data }
    }

    /// Convert to a JSON object
    pub fn to_json_object(&self) -> serde_json::Map<String, serde_json::Value> {
        self.data
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// Create from a JSON object
    pub fn from_json_object(object: serde_json::Map<String, serde_json::Value>) -> Self {
        Self {
            data: object.into_iter().collect(),
        }
    }
}

impl From<HashMap<String, serde_json::Value>> for Payload {
    fn from(data: HashMap<String, serde_json::Value>) -> Self {
        Self::from_map(data)
    }
}

impl From<serde_json::Map<String, serde_json::Value>> for Payload {
    fn from(object: serde_json::Map<String, serde_json::Value>) -> Self {
        Self::from_json_object(object)
    }
}

impl From<Payload> for HashMap<String, serde_json::Value> {
    fn from(payload: Payload) -> Self {
        payload.data
    }
}

impl From<Payload> for serde_json::Value {
    fn from(payload: Payload) -> Self {
        serde_json::Value::Object(payload.to_json_object())
    }
}

impl std::ops::Index<&str> for Payload {
    type Output = serde_json::Value;

    fn index(&self, key: &str) -> &Self::Output {
        &self.data[key]
    }
}

impl std::ops::IndexMut<&str> for Payload {
    fn index_mut(&mut self, key: &str) -> &mut Self::Output {
        self.data.get_mut(key).expect("key not found")
    }
}

impl IntoIterator for Payload {
    type IntoIter = std::collections::hash_map::IntoIter<String, serde_json::Value>;
    type Item = (String, serde_json::Value);

    fn into_iter(self) -> Self::IntoIter {
        self.data.into_iter()
    }
}

impl<'a> IntoIterator for &'a Payload {
    type IntoIter = std::collections::hash_map::Iter<'a, String, serde_json::Value>;
    type Item = (&'a String, &'a serde_json::Value);

    fn into_iter(self) -> Self::IntoIter {
        self.data.iter()
    }
}

/// Convert a serde_json::Value to a Qdrant Value
fn json_value_to_qdrant_value(value: serde_json::Value) -> qdrant_client::qdrant::Value {
    use qdrant_client::qdrant::value::Kind;
    use qdrant_client::qdrant::{ListValue, Struct, Value};

    let kind = match value {
        serde_json::Value::Null => Kind::NullValue(0), // protobuf null value
        serde_json::Value::Bool(b) => Kind::BoolValue(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Kind::IntegerValue(i)
            } else if let Some(f) = n.as_f64() {
                Kind::DoubleValue(f)
            } else {
                Kind::StringValue(n.to_string())
            }
        }
        serde_json::Value::String(s) => Kind::StringValue(s),
        serde_json::Value::Array(arr) => {
            let values = arr.into_iter().map(json_value_to_qdrant_value).collect();
            Kind::ListValue(ListValue { values })
        }
        serde_json::Value::Object(obj) => {
            let fields = obj
                .into_iter()
                .map(|(k, v)| (k, json_value_to_qdrant_value(v)))
                .collect();
            Kind::StructValue(Struct { fields })
        }
    };

    Value { kind: Some(kind) }
}

/// Convert a Qdrant Value to a serde_json::Value
fn qdrant_value_to_json_value(value: qdrant_client::qdrant::Value) -> serde_json::Value {
    use qdrant_client::qdrant::value::Kind;

    match value.kind {
        Some(kind) => match kind {
            Kind::NullValue(_) => serde_json::Value::Null,
            Kind::BoolValue(b) => serde_json::Value::Bool(b),
            Kind::IntegerValue(i) => serde_json::Value::Number(i.into()),
            Kind::DoubleValue(f) => serde_json::Number::from_f64(f)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null),
            Kind::StringValue(s) => serde_json::Value::String(s),
            Kind::ListValue(list) => {
                let values = list
                    .values
                    .into_iter()
                    .map(qdrant_value_to_json_value)
                    .collect();
                serde_json::Value::Array(values)
            }
            Kind::StructValue(struct_val) => {
                let obj = struct_val
                    .fields
                    .into_iter()
                    .map(|(k, v)| (k, qdrant_value_to_json_value(v)))
                    .collect();
                serde_json::Value::Object(obj)
            }
        },
        None => serde_json::Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_payload_creation() {
        let payload = Payload::new();
        assert!(payload.is_empty());
        assert_eq!(payload.len(), 0);
    }

    #[test]
    fn test_payload_insert_and_get() {
        let mut payload = Payload::new();
        payload.insert("key1", "value1");
        payload.insert("key2", 42i64);
        payload.insert("key3", true);

        assert_eq!(payload.get_string("key1"), Some("value1"));
        assert_eq!(payload.get_i64("key2"), Some(42));
        assert_eq!(payload.get_bool("key3"), Some(true));
        assert!(!payload.is_empty());
        assert_eq!(payload.len(), 3);
    }

    #[test]
    fn test_payload_chaining() {
        let payload = Payload::new()
            .with("name", "test")
            .with("age", 25)
            .with("active", true);

        assert_eq!(payload.get_string("name"), Some("test"));
        assert_eq!(payload.get_i64("age"), Some(25));
        assert_eq!(payload.get_bool("active"), Some(true));
    }

    #[test]
    fn test_payload_from_hashmap() {
        let mut map = HashMap::new();
        map.insert("key".to_string(), json!("value"));

        let payload = Payload::from_map(map);
        assert_eq!(payload.get_string("key"), Some("value"));
    }

    #[test]
    fn test_payload_typed_get() {
        let mut payload = Payload::new();
        payload.insert("number", json!(42));
        payload.insert("text", json!("hello"));
        payload.insert("flag", json!(true));

        let number: i32 = payload.get_typed("number").unwrap().unwrap();
        assert_eq!(number, 42);

        let text: String = payload.get_typed("text").unwrap().unwrap();
        assert_eq!(text, "hello");

        let flag: bool = payload.get_typed("flag").unwrap().unwrap();
        assert!(flag);
    }

    #[test]
    fn test_payload_merge() {
        let mut payload1 = Payload::new().with("key1", "value1").with("key2", "value2");
        let payload2 = Payload::new()
            .with("key2", "new_value2")
            .with("key3", "value3");

        payload1.merge(&payload2);

        assert_eq!(payload1.get_string("key1"), Some("value1"));
        assert_eq!(payload1.get_string("key2"), Some("new_value2")); // overwritten
        assert_eq!(payload1.get_string("key3"), Some("value3"));
    }

    #[test]
    fn test_payload_iteration() {
        let payload = Payload::new().with("a", 1).with("b", 2).with("c", 3);

        let keys: Vec<&String> = payload.keys().collect();
        assert_eq!(keys.len(), 3);

        let mut count = 0;
        for (key, _value) in &payload {
            assert!(payload.contains_key(key));
            count += 1;
        }
        assert_eq!(count, 3);
    }

    #[test]
    fn test_json_value_conversion() {
        // Test various JSON types
        let original_json = json!({
            "string": "hello",
            "number": 42,
            "float": 3.14,
            "bool": true,
            "null": null,
            "array": [1, 2, 3],
            "object": {"nested": "value"}
        });

        // Convert JSON -> Qdrant Value -> JSON
        let qdrant_value = json_value_to_qdrant_value(original_json.clone());
        let converted_json = qdrant_value_to_json_value(qdrant_value);

        assert_eq!(original_json, converted_json);
    }
}
