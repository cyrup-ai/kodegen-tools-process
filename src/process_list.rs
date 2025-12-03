use kodegen_mcp_tool::{Tool, ToolExecutionContext, ToolResponse};
use kodegen_mcp_tool::error::McpError;
use kodegen_mcp_schema::process::{
    ProcessListArgs, ProcessListPromptArgs, ProcessListOutput, ProcessInfo, PROCESS_LIST
};
use rmcp::model::{PromptArgument, PromptMessage, PromptMessageContent, PromptMessageRole};
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
    type PromptArgs = ProcessListPromptArgs;

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

    fn prompt_arguments() -> Vec<PromptArgument> {
        vec![
            PromptArgument {
                name: "use_case".to_string(),
                title: None,
                description: Some(
                    "Primary use case for learning (system_monitoring, debugging, process_discovery)".to_string(),
                ),
                required: Some(false),
            },
            PromptArgument {
                name: "metrics_focus".to_string(),
                title: None,
                description: Some(
                    "Which metrics to emphasize in examples (cpu, memory, all)".to_string(),
                ),
                required: Some(false),
            },
            PromptArgument {
                name: "filter_strategy".to_string(),
                title: None,
                description: Some(
                    "Filter pattern type to focus on (exact_match, substring, prefix)".to_string(),
                ),
                required: Some(false),
            },
        ]
    }

    async fn prompt(&self, _args: Self::PromptArgs) -> Result<Vec<PromptMessage>, McpError> {
        Ok(vec![
            PromptMessage {
                role: PromptMessageRole::User,
                content: PromptMessageContent::text("How do I monitor system processes and find resource hogs?"),
            },
            PromptMessage {
                role: PromptMessageRole::Assistant,
                content: PromptMessageContent::text(
                    "The process_list tool provides real-time system process monitoring:\n\n\
                     BASIC USAGE:\n\
                     1. List all processes:\n\
                        process_list({})\n\n\
                     2. Filter by process name (case-insensitive):\n\
                        process_list({\"filter\": \"python\"})\n\n\
                     3. Limit results for large process lists:\n\
                        process_list({\"filter\": \"node\", \"limit\": 10})\n\n\
                     UNDERSTANDING THE OUTPUT:\n\
                     - pid: Process ID (use with process_kill tool)\n\
                     - name: Process/command name (substring match against this)\n\
                     - cpu_percent: CPU usage percentage (can exceed 100% on multi-core)\n\
                     - memory_mb: Memory usage in megabytes (RSS-based, varies by OS)\n\
                     - Processes sorted by CPU usage (highest first) for easy identification",
                ),
            },
            PromptMessage {
                role: PromptMessageRole::User,
                content: PromptMessageContent::text("What are common patterns for debugging performance issues?"),
            },
            PromptMessage {
                role: PromptMessageRole::Assistant,
                content: PromptMessageContent::text(
                    "PERFORMANCE DEBUGGING PATTERNS:\n\n\
                     1. Find CPU hogs:\n\
                        process_list({\"limit\": 5})\n\
                        # Top 5 processes by CPU usage\n\n\
                     2. Find memory hogs:\n\
                        process_list({\"limit\": 10})\n\
                        # Check memory_mb column (already sorted by CPU, but readable)\n\n\
                     3. Monitor specific application:\n\
                        process_list({\"filter\": \"chrome\"})\n\
                        # Find all Chrome processes\n\n\
                     4. Combine filter and limit:\n\
                        process_list({\"filter\": \"java\", \"limit\": 5})\n\
                        # Top 5 Java processes by CPU\n\n\
                     IMPORTANT METRICS INTERPRETATION:\n\
                     - CPU percentages reflect snapshot at query time (not averaged)\n\
                     - On 4-core systems, 200% = full utilization of 2 cores\n\
                     - Memory includes shared libraries and cached pages\n\
                     - Negative memory values (precision loss) are rare but handled",
                ),
            },
            PromptMessage {
                role: PromptMessageRole::User,
                content: PromptMessageContent::text("How do I use this with process_kill to terminate specific processes?"),
            },
            PromptMessage {
                role: PromptMessageRole::Assistant,
                content: PromptMessageContent::text(
                    "INTEGRATION WITH PROCESS_KILL:\n\n\
                     WORKFLOW PATTERN:\n\
                     1. List and filter processes:\n\
                        process_list({\"filter\": \"stuck_service\"})\n\n\
                     2. Identify the PID from the results\n\n\
                     3. Terminate the specific process:\n\
                        process_kill({\"pid\": 12345})\n\n\
                     COMPLETE EXAMPLE:\n\
                     # Find Python processes using the most CPU\n\
                     process_list({\"filter\": \"python\", \"limit\": 1})\n\
                     # Returns: pid=8923, name=\"python\", cpu_percent=95.2\n\n\
                     # Terminate that specific process\n\
                     process_kill({\"pid\": 8923})\n\n\
                     BEST PRACTICES:\n\
                     - Always verify PID and process name before killing\n\
                     - Use limit parameter to reduce result set size\n\
                     - Filter first, then examine top result(s)\n\
                     - Re-run process_list after kill to confirm termination",
                ),
            },
            PromptMessage {
                role: PromptMessageRole::User,
                content: PromptMessageContent::text("What are the edge cases and limitations I should know?"),
            },
            PromptMessage {
                role: PromptMessageRole::Assistant,
                content: PromptMessageContent::text(
                    "EDGE CASES AND GOTCHAS:\n\n\
                     FILTERING BEHAVIOR:\n\
                     - Filter is case-insensitive substring match\n\
                     - \"python\" matches \"Python.exe\", \"my-python-app\", \"python-venv\"\n\
                     - Empty filter (omit parameter) returns ALL processes\n\
                     - Filter matching happens AFTER total count (see both in output)\n\n\
                     SYSTEM BEHAVIOR:\n\
                     - System processes appear/disappear between calls\n\
                     - CPU usage is instantaneous (not cumulative)\n\
                     - Some processes require elevated permissions to read\n\
                     - Memory reporting includes shared memory (counts multiple times)\n\n\
                     PERFORMANCE CONSIDERATIONS:\n\
                     - Uses spawn_blocking (sysinfo is CPU-intensive)\n\
                     - Limit parameter essential for systems with 1000+ processes\n\
                     - Filtering on client side (apply after total count)\n\
                     - Each call refreshes system data (not cached)\n\n\
                     OUTPUT STRUCTURE:\n\
                     - Content[0]: Human-readable summary with icons and counts\n\
                     - Content[1]: JSON with process array, metadata, and filter info\n\
                     - JSON includes: total_count (before filter), filtered_count, limited flag\n\n\
                     PLATFORM DIFFERENCES:\n\
                     - Linux: Full process list typically visible\n\
                     - macOS: Some system processes may be hidden\n\
                     - Windows: Process names differ (.exe suffixes, display names)\n\
                     - CPU percentages vary in precision by platform",
                ),
            },
        ])
    }
}
