use ferroclaw_core::{ConversationHistory, FerroError, Message, ToolResult};
use ferroclaw_tools::{ToolContext, ToolRegistry};
use tracing::info;

use crate::provider::LlmProvider;

/// Runs the agent tool-call loop until the model produces a plain-text reply
/// or `max_steps` is exceeded.
pub struct AgentLoop<'a, P: LlmProvider> {
    provider: &'a P,
    registry: &'a ToolRegistry,
    max_steps: usize,
    ctx: ToolContext,
}

impl<'a, P: LlmProvider> AgentLoop<'a, P> {
    pub fn new(provider: &'a P, registry: &'a ToolRegistry, max_steps: usize) -> Self {
        Self {
            provider,
            registry,
            max_steps,
            ctx: ToolContext::default(),
        }
    }

    pub fn with_cwd(mut self, cwd: impl Into<String>) -> Self {
        self.ctx.cwd = Some(cwd.into());
        self
    }

    /// Run the loop and return the final text response.
    pub async fn run(&self, history: &mut ConversationHistory) -> Result<String, FerroError> {
        let tools = self.registry.schemas();

        for step in 0..self.max_steps {
            let response = self.provider.complete(&history.messages, &tools).await?;

            if response.is_tool_call() {
                let calls = response.tool_calls.clone();
                info!(step, count = calls.len(), "tool calls requested");

                // Append assistant's tool-call message
                history.push(Message::assistant_tool_calls(calls.clone()));

                // Execute all tool calls and collect results
                let mut results: Vec<ToolResult> = Vec::new();
                for call in &calls {
                    println!("[tool] {}: {}", call.name, call.input);

                    let tool = self
                        .registry
                        .get(&call.name)
                        .ok_or_else(|| FerroError::ToolNotFound(call.name.clone()))?;

                    let mut result = tool.execute(call.input.clone(), &self.ctx).await?;
                    // Patch IDs so the LLM can match results to calls
                    result.tool_call_id = call.id.clone();
                    result.tool_name = call.name.clone();

                    if result.is_error {
                        println!("[tool error] {}", result.output);
                    }
                    results.push(result);
                }

                history.push(Message::tool_results(results));
            } else {
                // Plain text reply — we're done
                let text = response
                    .text
                    .unwrap_or_else(|| String::from("(empty response)"));
                info!(step, "agent loop complete");
                return Ok(text);
            }
        }

        Err(FerroError::MaxStepsExceeded(self.max_steps))
    }
}
