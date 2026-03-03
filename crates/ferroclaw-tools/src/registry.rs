use std::collections::HashMap;
use std::sync::Arc;

use ferroclaw_core::FerroError;
use serde_json::Value;

use crate::tool::Tool;

/// Registry of available tools, indexed by name.
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: impl Tool + 'static) {
        self.tools.insert(tool.name().to_owned(), Arc::new(tool));
    }

    pub fn get(&self, name: &str) -> Option<&Arc<dyn Tool>> {
        self.tools.get(name)
    }

    /// Returns all tool schemas in OpenAI function-calling format.
    pub fn schemas(&self) -> Vec<Value> {
        self.tools
            .values()
            .map(|t| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.name(),
                        "description": t.description(),
                        "parameters": t.input_schema(),
                    }
                })
            })
            .collect()
    }

    pub fn tool_names(&self) -> Vec<&str> {
        self.tools.keys().map(|s| s.as_str()).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    pub fn len(&self) -> usize {
        self.tools.len()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience: look up a tool and return a typed error if missing.
pub fn require_tool<'a>(
    registry: &'a ToolRegistry,
    name: &str,
) -> Result<&'a Arc<dyn Tool>, FerroError> {
    registry
        .get(name)
        .ok_or_else(|| FerroError::ToolNotFound(name.to_owned()))
}
