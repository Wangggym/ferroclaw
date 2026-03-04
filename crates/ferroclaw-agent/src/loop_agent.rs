use ferroclaw_core::{ConversationHistory, FerroError, Message, ToolResult};
use ferroclaw_tools::{ToolContext, ToolRegistry};
use futures::future::join_all;
use tracing::info;

use crate::provider::LlmProvider;

/// Runs the agent tool-call loop until the model produces a plain-text reply
/// or `max_steps` is exceeded.
///
/// Tool calls within a single step are executed concurrently (matching
/// OpenClaw's parallel tool execution behaviour).
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
            let response = self.provider.complete(history.as_slice(), &tools).await?;

            if response.is_tool_call() {
                let calls = response.tool_calls.clone();
                info!(step, count = calls.len(), "tool calls requested");

                // Append assistant's tool-call message
                history.push(Message::assistant_tool_calls(calls.clone()));

                // Execute all tool calls concurrently (parallel, like OpenClaw)
                let futures: Vec<_> = calls
                    .iter()
                    .map(|call| {
                        let registry = self.registry;
                        let ctx = &self.ctx;
                        async move {
                            tracing::info!(tool = %call.name, "executing tool");

                            let tool = registry
                                .get(&call.name)
                                .ok_or_else(|| FerroError::ToolNotFound(call.name.clone()))?;

                            let mut result = tool.execute(call.input.clone(), ctx).await?;
                            result.tool_call_id = call.id.clone();
                            result.tool_name = call.name.clone();

                            if result.is_error {
                                tracing::warn!(tool = %call.name, output = %result.output, "tool returned error");
                            }

                            Ok::<ToolResult, FerroError>(result)
                        }
                    })
                    .collect();

                // Collect results preserving original call order
                let outcomes: Vec<Result<ToolResult, FerroError>> = join_all(futures).await;
                let mut results: Vec<ToolResult> = Vec::with_capacity(outcomes.len());
                for outcome in outcomes {
                    results.push(outcome?);
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
