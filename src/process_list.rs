use kodegen_mcp_schema::{Tool, ToolExecutionContext, ToolResponse};
use kodegen_mcp_schema::McpError;
use kodegen_mcp_schema::process::{
    ProcessListArgs, ProcessListOutput, ProcessListPrompts, ProcessInfo, PROCESS_LIST
};
use sysinfo::System;

// ============================================================================
// TOOL STRUCT
// ============================================================================

#[derive(Clone, Default)]
pub struct ProcessListTool;

impl ProcessListTool {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

// ============================================================================
// TOOL IMPLEMENTATION
// ============================================================================

impl Tool for ProcessListTool {
    type Args = ProcessListArgs;
    type Prompts = ProcessListPrompts;

    fn name() -> &'static str {
        PROCESS_LIST
    }

    fn description() -> &'static str {
        "List all running processes with PID, command name, CPU usage, and memory usage. \
         Supports filtering by process name and limiting results. Returns comprehensive \
         process information for system monitoring and debugging."
    }

    fn read_only() -> bool {
        true
    }

    async fn execute(&self, args: Self::Args, _ctx: ToolExecutionContext) -> Result<ToolResponse<ProcessListOutput>, McpError> {
        // Clone filter before moving args into closure
        let filter_clone = args.filter.clone();

        // Use spawn_blocking because sysinfo operations are CPU-intensive
        let processes = tokio::task::spawn_blocking(move || {
            let mut system = System::new_all();
            system.refresh_all();

            let mut process_list: Vec<ProcessInfo> = system
                .processes()
                .iter()
                .map(|(pid, process)| {
                    ProcessInfo {
                        pid: pid.as_u32(),
                        name: process.name().to_string_lossy().to_string(),
                        cpu_percent: process.cpu_usage(),
                        // Note: Precision loss is acceptable for display purposes
                        memory_mb: f64::from(u32::try_from(process.memory()).unwrap_or(u32::MAX))
                            / 1024.0
                            / 1024.0,
                    }
                })
                .collect();

            // Apply filter if provided
            if let Some(filter) = &args.filter {
                let filter_lower = filter.to_lowercase();
                process_list.retain(|p| p.name.to_lowercase().contains(&filter_lower));
            }

            // Sort by CPU usage (descending) for useful output
            process_list.sort_by(|a, b| {
                b.cpu_percent
                    .partial_cmp(&a.cpu_percent)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            // Apply limit if specified
            if args.limit > 0 {
                process_list.truncate(args.limit);
            }

            process_list
        })
        .await
        .map_err(|e| McpError::Other(anyhow::anyhow!("Failed to list processes: {e}")))?;

        // Human-readable summary
        let filter_text = filter_clone.as_deref().unwrap_or("none");
        let summary = format!(
            "\x1b[36m󰒓 Processes\x1b[0m\n 󰋽 Count: {} · Filter: {}",
            processes.len(),
            filter_text
        );

        Ok(ToolResponse::new(
            summary,
            ProcessListOutput {
                success: true,
                count: processes.len(),
                processes,
            },
        ))
    }
}
