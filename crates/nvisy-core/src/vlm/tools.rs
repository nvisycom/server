//! Tool integration and function calling support for VLM operations.
//!
//! This module provides comprehensive support for defining, calling, and managing
//! tools (functions) that Vision Language Models can use during generation. It includes
//! JSON Schema-based parameter validation, visual context awareness, and structured
//! result handling.
//!
//! ## Key Features
//!
//! - **Visual Tool Calling**: Tools that can access and process visual content
//! - **JSON Schema Validation**: Automatic parameter validation using JSON Schema
//! - **Type Safety**: Strongly-typed interfaces for tool definitions and calls
//! - **Rich Metadata**: Execution time, token usage, and cost tracking
//! - **Error Handling**: Comprehensive error types for tool execution failures
//! - **Structured Results**: Well-defined success and error result types
//!
//! ## Examples
//!
//! ```rust
//! use nvisy_core::vlm::tools::{VlmTool, VlmToolCall, VlmToolResult, ParameterSchema};
//! use std::collections::HashMap;
//!
//! // Define a visual analysis tool
//! let mut properties = HashMap::new();
//! properties.insert(
//!     "focus_area".to_string(),
//!     ParameterSchema::string("Area of the image to focus analysis on")
//!         .with_enum(vec!["foreground", "background", "center", "edges"])
//! );
//! properties.insert(
//!     "analysis_type".to_string(),
//!     ParameterSchema::string("Type of analysis to perform")
//!         .with_enum(vec!["objects", "text", "colors", "emotions"])
//! );
//!
//! let visual_tool = VlmTool::new(
//!     "analyze_image_region",
//!     "Analyze a specific region or aspect of the image",
//!     properties,
//!     vec!["focus_area".to_string(), "analysis_type".to_string()],
//!     true, // requires_visual_context
//! );
//!
//! // Execute tool call
//! let call = VlmToolCall::new("call-123", "analyze_image_region",
//!     serde_json::json!({
//!         "focus_area": "center",
//!         "analysis_type": "objects"
//!     }));
//!
//! let result = VlmToolResult::success("call-123",
//!     "The center of the image contains a red sports car and two people walking");
//! ```

use std::collections::HashMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json;

use crate::vlm::error::{Error, Result};

/// JSON Schema parameter type for tool function parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ParameterSchema {
    /// String parameter with optional constraints
    #[serde(rename = "string")]
    String {
        /// Description of the parameter
        description: String,
        /// Allowed enum values
        #[serde(skip_serializing_if = "Option::is_none")]
        r#enum: Option<Vec<String>>,
        /// Minimum length
        #[serde(skip_serializing_if = "Option::is_none")]
        min_length: Option<usize>,
        /// Maximum length
        #[serde(skip_serializing_if = "Option::is_none")]
        max_length: Option<usize>,
        /// Regex pattern
        #[serde(skip_serializing_if = "Option::is_none")]
        pattern: Option<String>,
    },

    /// Number parameter (integer or float)
    #[serde(rename = "number")]
    Number {
        /// Description of the parameter
        description: String,
        /// Minimum value
        #[serde(skip_serializing_if = "Option::is_none")]
        minimum: Option<f64>,
        /// Maximum value
        #[serde(skip_serializing_if = "Option::is_none")]
        maximum: Option<f64>,
        /// Multiple of (for integers)
        #[serde(skip_serializing_if = "Option::is_none")]
        multiple_of: Option<f64>,
    },

    /// Integer parameter
    #[serde(rename = "integer")]
    Integer {
        /// Description of the parameter
        description: String,
        /// Minimum value
        #[serde(skip_serializing_if = "Option::is_none")]
        minimum: Option<i64>,
        /// Maximum value
        #[serde(skip_serializing_if = "Option::is_none")]
        maximum: Option<i64>,
        /// Multiple of
        #[serde(skip_serializing_if = "Option::is_none")]
        multiple_of: Option<i64>,
    },

    /// Boolean parameter
    #[serde(rename = "boolean")]
    Boolean {
        /// Description of the parameter
        description: String,
    },

    /// Array parameter
    #[serde(rename = "array")]
    Array {
        /// Description of the parameter
        description: String,
        /// Schema for array items
        items: Box<ParameterSchema>,
        /// Minimum number of items
        #[serde(skip_serializing_if = "Option::is_none")]
        min_items: Option<usize>,
        /// Maximum number of items
        #[serde(skip_serializing_if = "Option::is_none")]
        max_items: Option<usize>,
        /// Whether items must be unique
        #[serde(skip_serializing_if = "Option::is_none")]
        unique_items: Option<bool>,
    },

    /// Object parameter with nested properties
    #[serde(rename = "object")]
    Object {
        /// Description of the parameter
        description: String,
        /// Property schemas
        properties: HashMap<String, ParameterSchema>,
        /// Required property names
        required: Vec<String>,
        /// Whether additional properties are allowed
        #[serde(skip_serializing_if = "Option::is_none")]
        additional_properties: Option<bool>,
    },
}

impl ParameterSchema {
    /// Creates a string parameter schema.
    pub fn string(description: impl Into<String>) -> Self {
        Self::String {
            description: description.into(),
            r#enum: None,
            min_length: None,
            max_length: None,
            pattern: None,
        }
    }

    /// Creates a number parameter schema.
    pub fn number(description: impl Into<String>) -> Self {
        Self::Number {
            description: description.into(),
            minimum: None,
            maximum: None,
            multiple_of: None,
        }
    }

    /// Creates an integer parameter schema.
    pub fn integer(description: impl Into<String>) -> Self {
        Self::Integer {
            description: description.into(),
            minimum: None,
            maximum: None,
            multiple_of: None,
        }
    }

    /// Creates a boolean parameter schema.
    pub fn boolean(description: impl Into<String>) -> Self {
        Self::Boolean {
            description: description.into(),
        }
    }

    /// Creates an array parameter schema.
    pub fn array(description: impl Into<String>, items: ParameterSchema) -> Self {
        Self::Array {
            description: description.into(),
            items: Box::new(items),
            min_items: None,
            max_items: None,
            unique_items: None,
        }
    }

    /// Creates an object parameter schema.
    pub fn object(
        description: impl Into<String>,
        properties: HashMap<String, ParameterSchema>,
        required: Vec<String>,
    ) -> Self {
        Self::Object {
            description: description.into(),
            properties,
            required,
            additional_properties: None,
        }
    }

    /// Adds enum constraint to string parameter.
    pub fn with_enum(mut self, values: Vec<String>) -> Self {
        if let Self::String { ref mut r#enum, .. } = self {
            *r#enum = Some(values);
        }
        self
    }

    /// Adds length constraints to string parameter.
    pub fn with_length_constraints(mut self, min: Option<usize>, max: Option<usize>) -> Self {
        if let Self::String {
            ref mut min_length,
            ref mut max_length,
            ..
        } = self
        {
            *min_length = min;
            *max_length = max;
        }
        self
    }

    /// Adds range constraints to number parameter.
    pub fn with_range(mut self, min: Option<f64>, max: Option<f64>) -> Self {
        if let Self::Number {
            ref mut minimum,
            ref mut maximum,
            ..
        } = self
        {
            *minimum = min;
            *maximum = max;
        }
        self
    }

    /// Adds range constraints to integer parameter.
    pub fn with_integer_range(mut self, min: Option<i64>, max: Option<i64>) -> Self {
        if let Self::Integer {
            ref mut minimum,
            ref mut maximum,
            ..
        } = self
        {
            *minimum = min;
            *maximum = max;
        }
        self
    }

    /// Validates a JSON value against this schema.
    pub fn validate(&self, value: &serde_json::Value) -> Result<()> {
        match (self, value) {
            (
                Self::String {
                    r#enum: Some(allowed),
                    ..
                },
                serde_json::Value::String(s),
            ) => {
                if !allowed.contains(s) {
                    return Err(Error::invalid_input());
                }
            }
            (
                Self::String {
                    min_length: Some(min),
                    ..
                },
                serde_json::Value::String(s),
            ) => {
                if s.len() < *min {
                    return Err(Error::invalid_input());
                }
            }
            (
                Self::String {
                    max_length: Some(max),
                    ..
                },
                serde_json::Value::String(s),
            ) => {
                if s.len() > *max {
                    return Err(Error::invalid_input());
                }
            }
            (Self::String { .. }, serde_json::Value::String(_)) => {
                // Valid string
            }
            (
                Self::Number {
                    minimum: Some(min), ..
                },
                serde_json::Value::Number(n),
            ) => {
                if let Some(val) = n.as_f64() {
                    if val < *min {
                        return Err(Error::invalid_input());
                    }
                }
            }
            (
                Self::Number {
                    maximum: Some(max), ..
                },
                serde_json::Value::Number(n),
            ) => {
                if let Some(val) = n.as_f64() {
                    if val > *max {
                        return Err(Error::invalid_input());
                    }
                }
            }
            (Self::Boolean { .. }, serde_json::Value::Bool(_)) => {
                // Valid boolean
            }
            (
                Self::Array {
                    items,
                    min_items,
                    max_items,
                    ..
                },
                serde_json::Value::Array(arr),
            ) => {
                if let Some(min) = min_items {
                    if arr.len() < *min {
                        return Err(Error::invalid_input());
                    }
                }
                if let Some(max) = max_items {
                    if arr.len() > *max {
                        return Err(Error::invalid_input());
                    }
                }
                for (_i, item) in arr.iter().enumerate() {
                    items.validate(item)?;
                }
            }
            _ => {
                return Err(Error::invalid_input());
            }
        }
        Ok(())
    }
}

/// A tool definition for VLM function calling.
///
/// Tools define functions that the VLM can call during generation, including
/// their parameters, validation rules, and whether they require visual context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VlmTool {
    /// Unique identifier for the tool
    pub name: String,
    /// Human-readable description of what the tool does
    pub description: String,
    /// Parameter schemas for function arguments
    pub parameters: HashMap<String, ParameterSchema>,
    /// Names of required parameters
    pub required: Vec<String>,
    /// Whether this tool requires visual context from images
    pub requires_visual_context: bool,
    /// Maximum execution time allowed for this tool
    pub max_execution_time: Option<Duration>,
    /// Cost per execution (for tracking and billing)
    pub cost_per_execution: Option<f64>,
    /// Tool-specific metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl VlmTool {
    /// Creates a new VLM tool.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: HashMap<String, ParameterSchema>,
        required: Vec<String>,
        requires_visual_context: bool,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
            required,
            requires_visual_context,
            max_execution_time: None,
            cost_per_execution: None,
            metadata: HashMap::new(),
        }
    }

    /// Sets the maximum execution time for this tool.
    pub fn with_max_execution_time(mut self, duration: Duration) -> Self {
        self.max_execution_time = Some(duration);
        self
    }

    /// Sets the cost per execution for this tool.
    pub fn with_cost_per_execution(mut self, cost: f64) -> Self {
        self.cost_per_execution = Some(cost);
        self
    }

    /// Adds metadata to the tool.
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Validates the arguments for a tool call against this tool's schema.
    pub fn validate_arguments(&self, arguments: &serde_json::Value) -> Result<()> {
        let args_obj = arguments
            .as_object()
            .ok_or_else(|| Error::invalid_input())?;

        // Check required parameters
        for required_param in &self.required {
            if !args_obj.contains_key(required_param) {
                return Err(Error::invalid_input());
            }
        }

        // Validate each provided parameter
        for (param_name, param_value) in args_obj {
            if let Some(schema) = self.parameters.get(param_name) {
                schema.validate(param_value)?;
            } else {
                return Err(Error::invalid_input());
            }
        }

        Ok(())
    }

    /// Converts this tool to the JSON schema format expected by VLM APIs.
    pub fn to_json_schema(&self) -> serde_json::Value {
        let mut properties = serde_json::Map::new();
        for (name, schema) in &self.parameters {
            properties.insert(name.clone(), serde_json::to_value(schema).unwrap());
        }

        serde_json::json!({
            "name": self.name,
            "description": self.description,
            "parameters": {
                "type": "object",
                "properties": properties,
                "required": self.required,
                "additionalProperties": false
            },
            "requires_visual_context": self.requires_visual_context
        })
    }
}

/// A tool call made by a VLM during generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VlmToolCall {
    /// Unique identifier for this tool call
    pub id: String,
    /// Name of the tool to call
    pub tool_name: String,
    /// Arguments to pass to the tool
    pub arguments: serde_json::Value,
    /// Timestamp when the call was made
    pub timestamp: std::time::SystemTime,
    /// Visual context available during this call
    pub visual_context: Option<VisualContext>,
}

impl VlmToolCall {
    /// Creates a new tool call.
    pub fn new(
        id: impl Into<String>,
        tool_name: impl Into<String>,
        arguments: serde_json::Value,
    ) -> Self {
        Self {
            id: id.into(),
            tool_name: tool_name.into(),
            arguments,
            timestamp: std::time::SystemTime::now(),
            visual_context: None,
        }
    }

    /// Creates a new tool call with visual context.
    pub fn with_visual_context(
        id: impl Into<String>,
        tool_name: impl Into<String>,
        arguments: serde_json::Value,
        visual_context: VisualContext,
    ) -> Self {
        Self {
            id: id.into(),
            tool_name: tool_name.into(),
            arguments,
            timestamp: std::time::SystemTime::now(),
            visual_context: Some(visual_context),
        }
    }

    /// Returns whether this tool call has visual context available.
    pub fn has_visual_context(&self) -> bool {
        self.visual_context.is_some()
    }

    /// Gets a typed argument from the tool call arguments.
    pub fn get_argument<T>(&self, name: &str) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let value = self
            .arguments
            .get(name)
            .ok_or_else(|| Error::invalid_input())?;

        serde_json::from_value(value.clone()).map_err(|_| Error::invalid_input())
    }
}

/// Visual context information available during tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualContext {
    /// Image data and metadata
    pub images: Vec<ImageInfo>,
    /// Text extracted from images (if available)
    pub extracted_text: Option<String>,
    /// Detected objects in images (if available)
    pub detected_objects: Option<Vec<ObjectDetection>>,
    /// Image analysis results
    pub analysis_results: HashMap<String, serde_json::Value>,
}

/// Information about an image in the visual context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageInfo {
    /// Image identifier
    pub id: String,
    /// Image format (MIME type)
    pub format: String,
    /// Image dimensions (width, height)
    pub dimensions: Option<(u32, u32)>,
    /// File size in bytes
    pub size_bytes: Option<u64>,
    /// Image description or caption
    pub description: Option<String>,
    /// Metadata extracted from image
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Object detection result from image analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectDetection {
    /// Object label or class
    pub label: String,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f64,
    /// Bounding box coordinates (x, y, width, height)
    pub bounding_box: Option<(f64, f64, f64, f64)>,
    /// Additional object metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Result of a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VlmToolResult {
    /// Tool call ID this result corresponds to
    pub call_id: String,
    /// Whether the tool execution was successful
    pub success: bool,
    /// Result content or error message
    pub content: String,
    /// Structured result data
    pub data: Option<serde_json::Value>,
    /// Execution metadata
    pub metadata: ToolExecutionMetadata,
    /// Error details if execution failed
    pub error: Option<ToolExecutionError>,
}

impl VlmToolResult {
    /// Creates a successful tool result.
    pub fn success(call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            call_id: call_id.into(),
            success: true,
            content: content.into(),
            data: None,
            metadata: ToolExecutionMetadata::default(),
            error: None,
        }
    }

    /// Creates a successful tool result with structured data.
    pub fn success_with_data(
        call_id: impl Into<String>,
        content: impl Into<String>,
        data: serde_json::Value,
    ) -> Self {
        Self {
            call_id: call_id.into(),
            success: true,
            content: content.into(),
            data: Some(data),
            metadata: ToolExecutionMetadata::default(),
            error: None,
        }
    }

    /// Creates a failed tool result.
    pub fn error(call_id: impl Into<String>, error: ToolExecutionError) -> Self {
        Self {
            call_id: call_id.into(),
            success: false,
            content: error.message.clone(),
            data: None,
            metadata: ToolExecutionMetadata::default(),
            error: Some(error),
        }
    }

    /// Sets execution metadata.
    pub fn with_metadata(mut self, metadata: ToolExecutionMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    /// Gets typed data from the result.
    pub fn get_data<T>(&self) -> Result<Option<T>>
    where
        T: serde::de::DeserializeOwned,
    {
        match &self.data {
            Some(data) => Ok(Some(
                serde_json::from_value(data.clone()).map_err(|_| Error::internal_error())?,
            )),
            None => Ok(None),
        }
    }
}

/// Metadata about tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionMetadata {
    /// Execution start time
    pub started_at: std::time::SystemTime,
    /// Execution duration
    pub duration: Option<Duration>,
    /// Tokens consumed during execution
    pub tokens_used: Option<u64>,
    /// Cost incurred for execution
    pub cost: Option<f64>,
    /// Memory usage during execution
    pub memory_used_mb: Option<u64>,
    /// Additional execution metrics
    pub metrics: HashMap<String, f64>,
}

impl Default for ToolExecutionMetadata {
    fn default() -> Self {
        Self {
            started_at: std::time::SystemTime::now(),
            duration: None,
            tokens_used: None,
            cost: None,
            memory_used_mb: None,
            metrics: HashMap::new(),
        }
    }
}

/// Error that occurred during tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionError {
    /// Error category
    pub error_type: ToolErrorType,
    /// Human-readable error message
    pub message: String,
    /// Error code for programmatic handling
    pub code: Option<String>,
    /// Additional error context
    pub context: HashMap<String, serde_json::Value>,
}

/// Categories of tool execution errors.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolErrorType {
    /// Tool validation failed
    ValidationError,
    /// Tool execution timed out
    Timeout,
    /// Tool execution was cancelled
    Cancelled,
    /// Tool encountered a runtime error
    RuntimeError,
    /// Tool requires visual context but none was provided
    MissingVisualContext,
    /// Tool has insufficient permissions
    PermissionDenied,
    /// Tool resource is unavailable
    ResourceUnavailable,
    /// Internal tool error
    InternalError,
}

impl ToolExecutionError {
    /// Creates a new tool execution error.
    pub fn new(error_type: ToolErrorType, message: impl Into<String>) -> Self {
        Self {
            error_type,
            message: message.into(),
            code: None,
            context: HashMap::new(),
        }
    }

    /// Sets the error code.
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// Adds context to the error.
    pub fn with_context(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.context.insert(key.into(), value);
        self
    }
}

/// Tool registry for managing available tools.
#[derive(Debug, Default)]
pub struct ToolRegistry {
    /// Registered tools by name
    tools: HashMap<String, VlmTool>,
}

impl ToolRegistry {
    /// Creates a new tool registry.
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Registers a new tool.
    pub fn register(&mut self, tool: VlmTool) -> Result<()> {
        if self.tools.contains_key(&tool.name) {
            return Err(Error::invalid_input());
        }

        self.tools.insert(tool.name.clone(), tool);
        Ok(())
    }

    /// Gets a tool by name.
    pub fn get(&self, name: &str) -> Option<&VlmTool> {
        self.tools.get(name)
    }

    /// Lists all registered tool names.
    pub fn list_tools(&self) -> Vec<&str> {
        self.tools.keys().map(|s| s.as_str()).collect()
    }

    /// Gets all tools as JSON schemas for API calls.
    pub fn to_json_schemas(&self) -> Vec<serde_json::Value> {
        self.tools
            .values()
            .map(|tool| tool.to_json_schema())
            .collect()
    }

    /// Validates a tool call against registered tools.
    pub fn validate_call(&self, call: &VlmToolCall) -> Result<()> {
        let tool = self
            .get(&call.tool_name)
            .ok_or_else(|| Error::invalid_input())?;

        tool.validate_arguments(&call.arguments)?;

        if tool.requires_visual_context && !call.has_visual_context() {
            return Err(Error::invalid_input());
        }

        Ok(())
    }

    /// Gets the number of registered tools.
    pub fn count(&self) -> usize {
        self.tools.len()
    }

    /// Clears all registered tools.
    pub fn clear(&mut self) {
        self.tools.clear();
    }
}
