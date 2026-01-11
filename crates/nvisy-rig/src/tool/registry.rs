//! Tool registry for managing available tools.

use std::collections::HashMap;
use std::sync::Arc;

use super::{ToolCall, ToolDefinition, ToolInput, ToolOutput, ToolResult};
use crate::Result;

/// Handler function for tool execution.
pub type ToolHandler =
    Arc<dyn Fn(ToolInput) -> futures::future::BoxFuture<'static, Result<ToolOutput>> + Send + Sync>;

/// Registry of available tools.
#[derive(Default)]
pub struct ToolRegistry {
    definitions: HashMap<String, ToolDefinition>,
    handlers: HashMap<String, ToolHandler>,
}

impl ToolRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a registry with default tools.
    pub fn with_defaults() -> Self {
        use super::definition::ToolBuilder;

        let mut registry = Self::new();

        // Register default tool definitions
        registry.register_definition(ToolBuilder::search());
        registry.register_definition(ToolBuilder::read());
        registry.register_definition(ToolBuilder::extract());
        registry.register_definition(ToolBuilder::edit());
        registry.register_definition(ToolBuilder::insert());
        registry.register_definition(ToolBuilder::redact());

        registry
    }

    /// Registers a tool definition.
    pub fn register_definition(&mut self, definition: ToolDefinition) {
        self.definitions
            .insert(definition.name().to_string(), definition);
    }

    /// Registers a tool handler.
    pub fn register_handler(&mut self, name: impl Into<String>, handler: ToolHandler) {
        self.handlers.insert(name.into(), handler);
    }

    /// Registers both definition and handler.
    pub fn register(&mut self, definition: ToolDefinition, handler: ToolHandler) {
        let name = definition.name().to_string();
        self.definitions.insert(name.clone(), definition);
        self.handlers.insert(name, handler);
    }

    /// Returns a tool definition by name.
    pub fn get_definition(&self, name: &str) -> Option<&ToolDefinition> {
        self.definitions.get(name)
    }

    /// Returns all tool definitions.
    pub fn definitions(&self) -> impl Iterator<Item = &ToolDefinition> {
        self.definitions.values()
    }

    /// Returns all tool definitions as a vector.
    pub fn definitions_vec(&self) -> Vec<ToolDefinition> {
        self.definitions.values().cloned().collect()
    }

    /// Returns whether a tool exists.
    pub fn has_tool(&self, name: &str) -> bool {
        self.definitions.contains_key(name)
    }

    /// Returns whether a tool has a handler.
    pub fn has_handler(&self, name: &str) -> bool {
        self.handlers.contains_key(name)
    }

    /// Executes a tool call.
    pub async fn execute(&self, call: &ToolCall) -> ToolResult {
        let Some(handler) = self.handlers.get(&call.name) else {
            return ToolResult::failure(call.id, format!("tool '{}' not found", call.name));
        };

        let input = ToolInput {
            call_id: call.id,
            arguments: call.arguments.clone(),
        };

        match handler(input).await {
            Ok(output) => ToolResult::success(call.id, output),
            Err(e) => ToolResult::failure(call.id, e.to_string()),
        }
    }

    /// Returns the number of registered tools.
    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    /// Returns whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }
}

impl std::fmt::Debug for ToolRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolRegistry")
            .field("definitions", &self.definitions.keys().collect::<Vec<_>>())
            .field("handlers", &self.handlers.keys().collect::<Vec<_>>())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_with_defaults() {
        let registry = ToolRegistry::with_defaults();

        assert!(registry.has_tool("search"));
        assert!(registry.has_tool("read"));
        assert!(registry.has_tool("extract"));
        assert!(registry.has_tool("edit"));
        assert!(registry.has_tool("insert"));
        assert!(registry.has_tool("redact"));
    }

    #[test]
    fn registry_register_definition() {
        let mut registry = ToolRegistry::new();

        registry.register_definition(ToolDefinition::new(
            "custom",
            "A custom tool",
            serde_json::json!({}),
        ));

        assert!(registry.has_tool("custom"));
        assert!(!registry.has_handler("custom"));
    }
}
