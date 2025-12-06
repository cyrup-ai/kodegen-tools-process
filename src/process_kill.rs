use kodegen_mcp_schema::{Tool, ToolExecutionContext, ToolResponse};
use kodegen_mcp_schema::McpError;
use kodegen_mcp_schema::process::{ProcessKillArgs, ProcessKillOutput, ProcessKillPrompts, PROCESS_KILL};
use sysinfo::{Pid, ProcessesToUpdate, Signal, System};

// Compile-time platform validation for PID conversion safety
// This ensures u32 → usize conversion cannot truncate
#[cfg(not(any(target_pointer_width = "32", target_pointer_width = "64")))]
compile_error!("ProcessKillTool only supports 32-bit and 64-bit platforms");

// ============================================================================
// TOOL STRUCT
// ============================================================================

#[derive(Clone, Default)]
pub struct ProcessKillTool;

impl ProcessKillTool {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

// ============================================================================
// TOOL IMPLEMENTATION
// ============================================================================

impl Tool for ProcessKillTool {
    type Args = ProcessKillArgs;
    type Prompts = ProcessKillPrompts;

    fn name() -> &'static str {
        PROCESS_KILL
    }

    fn description() -> &'static str {
        "Terminate a running process by its PID. Sends SIGKILL signal to forcefully stop the \
         process. Use with caution as this does not allow graceful shutdown. Returns success \
         if process was terminated, error if process not found or permission denied."
    }

    fn read_only() -> bool {
        false // Modifies system state
    }

    fn destructive() -> bool {
        true // Terminates processes
    }

    fn idempotent() -> bool {
        false // Killing twice will fail (process no longer exists)
    }

    async fn execute(&self, args: Self::Args, _ctx: ToolExecutionContext) -> Result<ToolResponse<ProcessKillOutput>, McpError> {
        let pid = args.pid;

        // Validate PID
        if pid == 0 {
            return Err(McpError::InvalidArguments(
                "Invalid PID 0: cannot kill process with ID 0".to_string(),
            ));
        }

        // Use spawn_blocking for sysinfo operations
        let result = tokio::task::spawn_blocking(move || {
            let mut system = System::new();
            system.refresh_processes(ProcessesToUpdate::All, true);

            // Platform-validated PID conversion
            // Safe: u32 fits in usize on 32-bit and 64-bit platforms
            #[cfg(any(target_pointer_width = "32", target_pointer_width = "64"))]
            let sysinfo_pid = {
                // Document why conversion is safe on supported platforms:
                // - On 64-bit: usize = u64, conversion u32 → u64 is lossless
                // - On 32-bit: usize = u32, conversion u32 → u32 is identity
                Pid::from(pid as usize)
            };

            #[cfg(not(any(target_pointer_width = "32", target_pointer_width = "64")))]
            {
                return Err("Process termination not supported on this platform");
            }

            if let Some(process) = system.process(sysinfo_pid) {
                let process_name = process.name().to_string_lossy().to_string();
                let killed = process.kill_with(Signal::Kill);

                match killed {
                    Some(true) => Ok(process_name),
                    Some(false) => Err("Permission denied or process protected"),
                    None => Err("Failed to send kill signal"),
                }
            } else {
                Err("Process not found")
            }
        })
        .await
        .map_err(|e| McpError::Other(anyhow::anyhow!("Failed to kill process: {e}")))?;

        match result {
            Ok(_process_name) => {
                // Human-readable summary with ANSI red color and Nerd Font icons
                let summary = format!(
                    "\x1b[31m Process Killed: PID {}\x1b[0m\n\
                      Signal: SIGKILL · Status: terminated",
                    pid
                );

                Ok(ToolResponse::new(
                    summary,
                    ProcessKillOutput {
                        success: true,
                        pid,
                        message: format!("Successfully terminated process {}", pid),
                    },
                ))
            }
            Err(reason) => Err(McpError::PermissionDenied(format!(
                "Failed to kill process {pid}: {reason}"
            ))),
        }
    }
}
