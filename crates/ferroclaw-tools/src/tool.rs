use async_trait::async_trait;
use ferroclaw_core::{FerroError, ToolResult};
use serde_json::Value;

/// Shared context passed to every tool execution.
#[derive(Clone, Default)]
pub struct ToolContext {
    /// Current working directory for the execution environment.
    pub cwd: Option<String>,
}

/// Trait all tools must implement.
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;

    /// JSON Schema object describing the input parameters.
    fn input_schema(&self) -> Value;

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult, FerroError>;
}
