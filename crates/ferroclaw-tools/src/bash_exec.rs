use async_trait::async_trait;
use ferroclaw_core::{FerroError, ToolResult};
use serde_json::{json, Value};
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;
use tracing::warn;
use uuid::Uuid;

use crate::tool::{Tool, ToolContext};

const DEFAULT_TIMEOUT_SECS: u64 = 30;
const MAX_OUTPUT_BYTES: usize = 10 * 1024; // 10 KB

/// Dangerous command fragments to warn about (not block).
const DANGEROUS_PATTERNS: &[&str] = &[
    "rm -rf /",
    "rm -rf ~",
    "sudo rm",
    "> /dev/sda",
    "mkfs",
    "dd if=",
    ":(){ :|:& };:",
];

pub struct BashExecTool {
    timeout_secs: u64,
}

impl BashExecTool {
    pub fn new() -> Self {
        Self {
            timeout_secs: DEFAULT_TIMEOUT_SECS,
        }
    }

    pub fn with_timeout(secs: u64) -> Self {
        Self { timeout_secs: secs }
    }
}

impl Default for BashExecTool {
    fn default() -> Self {
        Self::new()
    }
}

fn check_dangerous(command: &str) {
    for pattern in DANGEROUS_PATTERNS {
        if command.contains(pattern) {
            warn!("⚠️  Potentially dangerous command detected: {pattern:?}");
        }
    }
}

fn truncate(s: String, max: usize) -> String {
    if s.len() <= max {
        s
    } else {
        format!(
            "{}\n… [output truncated at {max} bytes]",
            &s[..max]
        )
    }
}

#[async_trait]
impl Tool for BashExecTool {
    fn name(&self) -> &str {
        "bash_exec"
    }

    fn description(&self) -> &str {
        "Execute a bash command and return its output (stdout + stderr combined). \
         Times out after 30 seconds by default. Output is truncated at 10 KB."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The bash command to execute."
                },
                "timeout_secs": {
                    "type": "integer",
                    "description": "Override timeout in seconds (default: 30)."
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult, FerroError> {
        let command = input["command"]
            .as_str()
            .ok_or_else(|| FerroError::ToolExecution {
                tool: self.name().into(),
                message: "missing 'command' field".into(),
            })?
            .to_owned();

        let timeout_secs = input["timeout_secs"]
            .as_u64()
            .unwrap_or(self.timeout_secs);

        check_dangerous(&command);

        let mut cmd = Command::new("bash");
        cmd.arg("-c").arg(&command);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        if let Some(cwd) = &ctx.cwd {
            cmd.current_dir(cwd);
        }

        let run = async {
            let output = cmd
                .output()
                .await
                .map_err(|e| FerroError::ToolExecution {
                    tool: "bash_exec".into(),
                    message: e.to_string(),
                })?;

            let mut combined = String::new();
            combined.push_str(&String::from_utf8_lossy(&output.stdout));
            combined.push_str(&String::from_utf8_lossy(&output.stderr));
            let combined = truncate(combined, MAX_OUTPUT_BYTES);

            let is_error = !output.status.success();
            let exit_code = output.status.code().unwrap_or(-1);

            let output_text = if is_error {
                format!("[exit {exit_code}]\n{combined}")
            } else {
                combined
            };

            Ok::<_, FerroError>(ToolResult {
                tool_call_id: Uuid::new_v4().to_string(),
                tool_name: "bash_exec".into(),
                output: output_text,
                is_error,
            })
        };

        timeout(Duration::from_secs(timeout_secs), run)
            .await
            .map_err(|_| FerroError::ToolExecution {
                tool: "bash_exec".into(),
                message: format!("timed out after {timeout_secs}s"),
            })?
    }
}
