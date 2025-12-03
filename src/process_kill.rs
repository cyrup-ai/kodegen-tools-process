use kodegen_mcp_tool::{Tool, ToolExecutionContext, ToolResponse};
use kodegen_mcp_tool::error::McpError;
use kodegen_mcp_schema::process::{ProcessKillArgs, ProcessKillPromptArgs, ProcessKillOutput, PROCESS_KILL};
use rmcp::model::{PromptArgument, PromptMessage, PromptMessageContent, PromptMessageRole};
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
    type PromptArgs = ProcessKillPromptArgs;

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

    fn prompt_arguments() -> Vec<PromptArgument> {
        vec![
            PromptArgument {
                name: "scenario".to_string(),
                title: None,
                description: Some(
                    "Scenario to focus teaching on: 'hung_process' (unresponsive app), 'daemon' (background service), \
                     'cleanup' (resource cleanup), 'error_handling' (error cases), or 'workflow' (integration patterns)"
                        .to_string(),
                ),
                required: Some(false),
            },
            PromptArgument {
                name: "show_safety_warnings".to_string(),
                title: None,
                description: Some(
                    "Include extensive safety warnings, gotchas, and platform-specific issues (true/false)"
                        .to_string(),
                ),
                required: Some(false),
            },
        ]
    }

    async fn prompt(&self, args: Self::PromptArgs) -> Result<Vec<PromptMessage>, McpError> {
        let mut messages = vec![
            // Basic introduction
            PromptMessage {
                role: PromptMessageRole::User,
                content: PromptMessageContent::text("What does the process_kill tool do?"),
            },
            PromptMessage {
                role: PromptMessageRole::Assistant,
                content: PromptMessageContent::text(
                    "The process_kill tool terminates a running process by sending a SIGKILL signal. \
                     This is a forceful, immediate termination:\n\n\
                     Basic Usage:\n\
                     process_kill({\"pid\": 12345})\n\n\
                     Key Characteristics:\n\
                     - Sends SIGKILL (signal 9 on Unix/Linux/macOS)\n\
                     - Immediate termination - process cannot catch or ignore this signal\n\
                     - No cleanup allowed - process cannot save state or close files gracefully\n\
                     - Destructive and non-idempotent\n\n\
                     SIGKILL vs SIGTERM:\n\
                     - SIGTERM (15): Asks process to terminate gracefully - can be caught and handled\n\
                     - SIGKILL (9): Forces immediate termination - cannot be caught\n\n\
                     This tool uses SIGKILL exclusively. Use it when:\n\
                     - Process is completely hung and unresponsive\n\
                     - Graceful shutdown has failed or is not possible\n\
                     - You need immediate termination\n\n\
                     Returns:\n\
                     - success: true if process was terminated\n\
                     - pid: The terminated process ID\n\
                     - process_name: Name of the killed process\n\
                     - signal: \"SIGKILL\"",
                ),
            },
            // Finding the PID first
            PromptMessage {
                role: PromptMessageRole::User,
                content: PromptMessageContent::text(
                    "How do I find the PID of a process before killing it?",
                ),
            },
            PromptMessage {
                role: PromptMessageRole::Assistant,
                content: PromptMessageContent::text(
                    "Always use the process_list tool BEFORE process_kill to find the correct PID:\n\n\
                     Step 1: List processes with a filter\n\
                     process_list({\"filter\": \"myapp\", \"limit\": 10})\n\n\
                     This returns processes matching \"myapp\" with their PIDs, CPU usage, memory, etc.\n\n\
                     Step 2: Verify it's the correct process\n\
                     - Check the process name matches what you expect\n\
                     - Verify it's not a critical system process\n\
                     - Note the PID (e.g., 45823)\n\n\
                     Step 3: Kill the process\n\
                     process_kill({\"pid\": 45823})\n\n\
                     Example Workflow:\n\
                     # Find hung Node.js process\n\
                     process_list({\"filter\": \"node\"})\n\
                     # Verify PID 12345 is the hung app, not another Node process\n\
                     # Kill it\n\
                     process_kill({\"pid\": 12345})\n\n\
                     WARNING: Never kill a process without verifying the PID first! \
                     Killing the wrong PID can terminate critical system services or other applications.",
                ),
            },
            // When to use vs when NOT to use
            PromptMessage {
                role: PromptMessageRole::User,
                content: PromptMessageContent::text(
                    "When should I use process_kill, and when should I avoid it?",
                ),
            },
            PromptMessage {
                role: PromptMessageRole::Assistant,
                content: PromptMessageContent::text(
                    "Use process_kill when:\n\
                     - Process is completely frozen/hung and unresponsive\n\
                     - Process is consuming excessive resources and won't stop\n\
                     - Graceful shutdown attempts have failed\n\
                     - Process is in an infinite loop or deadlock\n\
                     - You need immediate termination for system stability\n\n\
                     AVOID process_kill when:\n\
                     - Process is actively saving data (wait for completion)\n\
                     - Process can be shut down gracefully (use proper shutdown)\n\
                     - Process is a critical system service (use service manager)\n\
                     - You're not certain what the process does (investigate first)\n\
                     - Process has important child processes (they may be orphaned)\n\
                     - Process holds locks that won't auto-release\n\n\
                     Better Alternatives:\n\
                     - Application shutdown commands (e.g., '/quit', shutdown API)\n\
                     - SIGTERM signal (allows graceful cleanup)\n\
                     - Service managers (systemctl stop, pm2 stop, etc.)\n\
                     - Application-specific stop commands\n\n\
                     Remember: SIGKILL is the 'nuclear option' - use it as a last resort!",
                ),
            },
            // What happens to child processes
            PromptMessage {
                role: PromptMessageRole::User,
                content: PromptMessageContent::text(
                    "What happens to child processes when I kill a parent process?",
                ),
            },
            PromptMessage {
                role: PromptMessageRole::Assistant,
                content: PromptMessageContent::text(
                    "When you kill a parent process, child processes are NOT automatically killed:\n\n\
                     What Happens:\n\
                     1. Parent process is immediately terminated (SIGKILL)\n\
                     2. Child processes become orphaned\n\
                     3. Orphaned children are re-parented to init (PID 1) or systemd\n\
                     4. Children continue running unless they detect parent exit\n\n\
                     Example:\n\
                     Parent PID 1000 has children PID 1001, 1002, 1003\n\
                     process_kill({\"pid\": 1000})\n\
                     Result: PID 1000 dies, but 1001, 1002, 1003 keep running\n\n\
                     How to Handle Child Processes:\n\n\
                     Option 1: Kill children first, then parent\n\
                     process_list({\"filter\": \"child-process\"})\n\
                     process_kill({\"pid\": child_pid_1})\n\
                     process_kill({\"pid\": child_pid_2})\n\
                     process_kill({\"pid\": parent_pid})\n\n\
                     Option 2: Let children exit naturally\n\
                     Some well-designed children exit when parent dies\n\
                     process_kill({\"pid\": parent_pid})\n\
                     # Wait a moment, then check if children exited\n\
                     process_list({\"filter\": \"child-process\"})\n\n\
                     Option 3: Use process groups (platform-specific)\n\
                     Some systems support killing entire process groups\n\n\
                     Best Practice:\n\
                     Before killing a parent, use process_list to identify children and plan accordingly.",
                ),
            },
            // Return values and status
            PromptMessage {
                role: PromptMessageRole::User,
                content: PromptMessageContent::text(
                    "What information does process_kill return after execution?",
                ),
            },
            PromptMessage {
                role: PromptMessageRole::Assistant,
                content: PromptMessageContent::text(
                    "process_kill returns detailed information about the termination:\n\n\
                     Success Response:\n\
                     {\n\
                       \"success\": true,\n\
                       \"pid\": 12345,\n\
                       \"process_name\": \"myapp\",\n\
                       \"signal\": \"SIGKILL\",\n\
                       \"message\": \"Successfully terminated process 12345 (myapp)\"\n\
                     }\n\n\
                     Fields:\n\
                     - success: true if process was killed\n\
                     - pid: The process ID that was terminated\n\
                     - process_name: Name of the killed process\n\
                     - signal: Always \"SIGKILL\" for this tool\n\
                     - message: Human-readable confirmation\n\n\
                     Error Response:\n\
                     - Process not found: PID doesn't exist (already exited)\n\
                     - Permission denied: Can't kill process (wrong user, protected process)\n\
                     - Failed to send kill signal: OS-level failure\n\n\
                     Use the return value to:\n\
                     - Confirm the kill succeeded\n\
                     - Log what was killed (process name + PID)\n\
                     - Handle errors appropriately (retry, escalate, etc.)\n\
                     - Verify you killed the intended process\n\n\
                     Example error handling:\n\
                     try:\n\
                       result = process_kill({\"pid\": 12345})\n\
                       log(\"Killed: \" + result.process_name)\n\
                     catch error:\n\
                       if \"not found\" in error:\n\
                         # Process already gone - that's OK\n\
                       elif \"permission denied\" in error:\n\
                         # Need elevated privileges",
                ),
            },
        ];

        // Add scenario-specific content
        match args.scenario.as_deref() {
            Some("hung_process") => {
                messages.extend(vec![
                    PromptMessage {
                        role: PromptMessageRole::User,
                        content: PromptMessageContent::text(
                            "How do I handle a hung, unresponsive application?",
                        ),
                    },
                    PromptMessage {
                        role: PromptMessageRole::Assistant,
                        content: PromptMessageContent::text(
                            "For hung/frozen applications, use this pattern:\n\n\
                             Scenario: Application frozen, UI unresponsive, consuming CPU\n\n\
                             Step 1: Identify the hung process\n\
                             process_list({\"filter\": \"AppName\", \"limit\": 5})\n\
                             Look for high CPU usage or the process you know is frozen.\n\n\
                             Step 2: Confirm it's truly hung\n\
                             - Check if CPU usage is stuck at 100% or 0%\n\
                             - Verify the application is actually unresponsive (not just slow)\n\
                             - Note the PID\n\n\
                             Step 3: Force kill\n\
                             process_kill({\"pid\": 67890})\n\n\
                             Real Example - Hung Python Script:\n\
                             # Find it\n\
                             process_list({\"filter\": \"python\"})\n\
                             # Results show PID 23456 with 99.8% CPU, stuck for hours\n\
                             # Kill it\n\
                             process_kill({\"pid\": 23456})\n\n\
                             Note: SIGKILL doesn't allow cleanup, so:\n\
                             - Unsaved data will be lost\n\
                             - Temporary files may remain\n\
                             - Database transactions may be incomplete\n\
                             - Locks may remain held (usually auto-released by OS)",
                        ),
                    },
                ]);
            }
            Some("daemon") => {
                messages.extend(vec![
                    PromptMessage {
                        role: PromptMessageRole::User,
                        content: PromptMessageContent::text(
                            "How do I kill a background daemon or service?",
                        ),
                    },
                    PromptMessage {
                        role: PromptMessageRole::Assistant,
                        content: PromptMessageContent::text(
                            "For daemon/service processes, prefer graceful shutdown but force kill if needed:\n\n\
                             Background Services Workflow:\n\n\
                             1. Try graceful shutdown first (if available)\n\
                             - For systemd: systemctl stop servicename\n\
                             - For process managers: pm2 stop, supervisorctl stop, etc.\n\
                             - These send SIGTERM, allowing graceful cleanup\n\n\
                             2. If graceful shutdown fails or hangs\n\
                             process_list({\"filter\": \"daemon-name\"})\n\
                             process_kill({\"pid\": found_pid})\n\n\
                             Example - Stuck Web Server:\n\
                             # Graceful shutdown hung, need to force kill\n\
                             process_list({\"filter\": \"nginx\"})\n\
                             # Find master process PID 8888\n\
                             process_kill({\"pid\": 8888})\n\n\
                             Important Considerations:\n\
                             - Killing parent daemon may leave child processes orphaned\n\
                             - Some daemons auto-restart via systemd/supervisor\n\
                             - Check if service manager will resurrect the process\n\
                             - Child processes may become adopted by init (PID 1)\n\
                             - Daemon lock files may remain in /var/run or /tmp",
                        ),
                    },
                ]);
            }
            Some("cleanup") => {
                messages.extend(vec![
                    PromptMessage {
                        role: PromptMessageRole::User,
                        content: PromptMessageContent::text(
                            "How do I clean up leftover processes after a crash?",
                        ),
                    },
                    PromptMessage {
                        role: PromptMessageRole::Assistant,
                        content: PromptMessageContent::text(
                            "For cleanup of leftover/orphaned processes:\n\n\
                             Common Scenario: Parent process crashed, leaving child processes running\n\n\
                             Step 1: Find orphaned processes\n\
                             process_list({\"filter\": \"app-worker\"})\n\
                             # Look for processes that should have exited with parent\n\n\
                             Step 2: Identify orphans\n\
                             - Processes with parent PID 1 (adopted by init)\n\
                             - Processes consuming resources unnecessarily\n\
                             - Multiple instances of single-instance apps\n\n\
                             Step 3: Clean up\n\
                             process_kill({\"pid\": orphan_pid_1})\n\
                             process_kill({\"pid\": orphan_pid_2})\n\n\
                             Example - Cleanup After Test Suite Crash:\n\
                             # Test runner crashed, leaving test servers running\n\
                             process_list({\"filter\": \"test-server\", \"limit\": 20})\n\
                             # Kill all found instances\n\
                             process_kill({\"pid\": 11111})\n\
                             process_kill({\"pid\": 11112})\n\
                             process_kill({\"pid\": 11113})\n\n\
                             Batch Cleanup Pattern:\n\
                             1. List all matching processes\n\
                             2. Filter to only the ones you want to kill\n\
                             3. Kill each one individually (verify PID each time)\n\
                             4. Handle errors for already-exited processes gracefully",
                        ),
                    },
                ]);
            }
            Some("error_handling") => {
                messages.extend(vec![
                    PromptMessage {
                        role: PromptMessageRole::User,
                        content: PromptMessageContent::text(
                            "What errors can occur and how should I handle them?",
                        ),
                    },
                    PromptMessage {
                        role: PromptMessageRole::Assistant,
                        content: PromptMessageContent::text(
                            "Common errors and how to handle them:\n\n\
                             1. Process Not Found\n\
                             Error: \"Failed to kill process 12345: Process not found\"\n\
                             Cause: Process already exited or PID doesn't exist\n\
                             Solution: This is often OK - process may have exited naturally\n\
                             Handle gracefully: Check if process needed killing or was already gone\n\n\
                             2. Permission Denied\n\
                             Error: \"Failed to kill process 1234: Permission denied or process protected\"\n\
                             Cause: Trying to kill process owned by another user or system process\n\
                             Solutions:\n\
                             - Run with elevated privileges if appropriate (sudo/root)\n\
                             - Verify you own the process\n\
                             - Don't try to kill system-critical processes\n\n\
                             3. Protected Process\n\
                             Error: \"Failed to kill process 1: Permission denied or process protected\"\n\
                             Cause: OS prevents killing critical system processes\n\
                             Solution: Don't kill init (PID 1), kernel threads, or system services\n\n\
                             Example Error Handling Pattern:\n\
                             try:\n\
                                 process_kill({\"pid\": target_pid})\n\
                             catch \"Process not found\":\n\
                                 # Process already gone - that's fine\n\
                                 log(\"Process already exited\")\n\
                             catch \"Permission denied\":\n\
                                 # Can't kill - check permissions\n\
                                 log(\"Insufficient permissions to kill process\")\n\
                                 # Possibly retry with elevated privileges\n\n\
                             Idempotency Note:\n\
                             process_kill is NOT idempotent:\n\
                             - First call: Kills process, returns success\n\
                             - Second call: Fails with \"Process not found\"\n\
                             - Always wrap in try/catch if calling multiple times",
                        ),
                    },
                ]);
            }
            Some("workflow") => {
                messages.extend(vec![
                    PromptMessage {
                        role: PromptMessageRole::User,
                        content: PromptMessageContent::text(
                            "What's the recommended workflow for safely killing processes?",
                        ),
                    },
                    PromptMessage {
                        role: PromptMessageRole::Assistant,
                        content: PromptMessageContent::text(
                            "Safe process termination workflow:\n\n\
                             RECOMMENDED WORKFLOW:\n\n\
                             1. Identify Target\n\
                             process_list({\"filter\": \"target_name\", \"limit\": 10})\n\
                             Review: name, PID, CPU, memory, parent PID\n\n\
                             2. Verify Process\n\
                             - Is this the correct process?\n\
                             - Will killing it affect other processes?\n\
                             - Are there child processes to consider?\n\
                             - Is this a system-critical process?\n\n\
                             3. Consider Alternatives\n\
                             - Can the process exit gracefully?\n\
                             - Should you signal the process differently?\n\
                             - Is there a proper shutdown command?\n\n\
                             4. Execute Kill\n\
                             process_kill({\"pid\": verified_pid})\n\n\
                             5. Verify Termination\n\
                             process_list({\"filter\": \"target_name\"})\n\
                             # Should return empty or not include the killed PID\n\n\
                             6. Handle Cleanup (if needed)\n\
                             - Remove lock files\n\
                             - Clean up temporary files\n\
                             - Restart dependent services\n\n\
                             Full Example - Restart Stuck Service:\n\
                             # Step 1: Find it\n\
                             process_list({\"filter\": \"my-service\"})\n\
                             # Found: PID 5555, using 800MB RAM, 50% CPU\n\n\
                             # Step 2: Verify (yes, it's the right one)\n\n\
                             # Step 3: Kill it\n\
                             process_kill({\"pid\": 5555})\n\n\
                             # Step 4: Verify it's gone\n\
                             process_list({\"filter\": \"my-service\"})\n\
                             # Empty result - success\n\n\
                             # Step 5: Restart service\n\
                             # (start service command here)\n\n\
                             Integration Pattern:\n\
                             process_list → inspect → verify → process_kill → confirm → cleanup",
                        ),
                    },
                ]);
            }
            _ => {
                // Default: Add general usage examples
                messages.extend(vec![
                    PromptMessage {
                        role: PromptMessageRole::User,
                        content: PromptMessageContent::text(
                            "Can you show me some practical examples?",
                        ),
                    },
                    PromptMessage {
                        role: PromptMessageRole::Assistant,
                        content: PromptMessageContent::text(
                            "Practical examples of using process_kill:\n\n\
                             Example 1: Kill hung browser tab (Chrome helper process)\n\
                             process_list({\"filter\": \"chrome helper\"})\n\
                             process_kill({\"pid\": 34567})\n\n\
                             Example 2: Kill runaway build process\n\
                             process_list({\"filter\": \"webpack\"})\n\
                             process_kill({\"pid\": 45678})\n\n\
                             Example 3: Kill stuck development server\n\
                             process_list({\"filter\": \"npm\"})\n\
                             # Identify the dev server PID\n\
                             process_kill({\"pid\": 56789})\n\n\
                             Example 4: Clean up test processes\n\
                             process_list({\"filter\": \"jest\", \"limit\": 20})\n\
                             # Kill each leftover test runner\n\
                             process_kill({\"pid\": 11111})\n\
                             process_kill({\"pid\": 22222})",
                        ),
                    },
                ]);
            }
        }

        // Add platform considerations
        messages.extend(vec![
            PromptMessage {
                role: PromptMessageRole::User,
                content: PromptMessageContent::text(
                    "Are there platform-specific differences I should know about?",
                ),
            },
            PromptMessage {
                role: PromptMessageRole::Assistant,
                content: PromptMessageContent::text(
                    "Platform-specific considerations:\n\n\
                     Unix/Linux:\n\
                     - SIGKILL (signal 9) is always honored by the kernel\n\
                     - Cannot be caught, blocked, or ignored by the process\n\
                     - Root can kill any process; users can only kill their own\n\
                     - Kernel threads cannot be killed\n\
                     - Init (PID 1) is specially protected\n\n\
                     macOS:\n\
                     - Same Unix signal behavior as Linux\n\
                     - System Integrity Protection (SIP) may prevent killing some Apple processes\n\
                     - Sandboxed apps may have additional restrictions\n\
                     - Root privileges work same as Linux\n\n\
                     Windows:\n\
                     - This tool uses sysinfo library which maps to Windows TerminateProcess\n\
                     - Behavior similar to SIGKILL: immediate, forceful termination\n\
                     - Administrator privileges needed for system processes\n\
                     - Protected processes (SYSTEM, services) may not be killable\n\n\
                     Cross-Platform Notes:\n\
                     - Tool works consistently across all platforms\n\
                     - Permission model varies but concept is same\n\
                     - Always check return value for success/failure\n\
                     - Error messages indicate reason for failure",
                ),
            },
        ]);

        // Add safety warnings if requested
        if args.show_safety_warnings.unwrap_or(false) {
            messages.extend(vec![
                PromptMessage {
                    role: PromptMessageRole::User,
                    content: PromptMessageContent::text(
                        "What are the safety warnings and gotchas I should know?",
                    ),
                },
                PromptMessage {
                    role: PromptMessageRole::Assistant,
                    content: PromptMessageContent::text(
                        "CRITICAL SAFETY WARNINGS AND GOTCHAS:\n\n\
                         ⚠️  DESTRUCTIVE OPERATION:\n\
                         - Process CANNOT clean up resources\n\
                         - Unsaved data WILL BE LOST\n\
                         - Open file handles closed abruptly\n\
                         - Database transactions may be incomplete\n\
                         - Network connections dropped immediately\n\n\
                         ⚠️  NON-IDEMPOTENT:\n\
                         - First kill succeeds, second fails\n\
                         - Always wrap in error handling\n\
                         - Don't assume kill can be retried safely\n\n\
                         ⚠️  CHILD PROCESSES:\n\
                         - Killing parent doesn't automatically kill children\n\
                         - Children become orphaned (re-parented to PID 1)\n\
                         - May need to kill child processes separately\n\
                         - Use process_list to find children before killing parent\n\n\
                         ⚠️  SYSTEM PROCESSES:\n\
                         - NEVER kill PID 1 (init/systemd) - will crash system\n\
                         - NEVER kill kernel threads\n\
                         - NEVER kill critical system daemons (sshd, systemd, etc.)\n\
                         - Permission denied errors protect you from most mistakes\n\n\
                         ⚠️  CASCADING EFFECTS:\n\
                         - Killing database may corrupt data\n\
                         - Killing web server drops active connections\n\
                         - Killing parent service may break dependents\n\
                         - Lock files may remain and block restart\n\n\
                         ⚠️  TIMING ISSUES:\n\
                         - PID can be reused immediately after process exits\n\
                         - Race condition: process exits between list and kill\n\
                         - Always verify PID is correct process before killing\n\
                         - Process may exit naturally while you're preparing to kill\n\n\
                         ⚠️  PERMISSION GOTCHAS:\n\
                         - Can only kill processes you own (unless root)\n\
                         - Sudo/root required for other users' processes\n\
                         - System processes require elevated privileges\n\
                         - Some processes protected even from root (platform-specific)\n\n\
                         BEST PRACTICES TO AVOID ISSUES:\n\
                         1. Always use process_list first - NEVER guess PIDs\n\
                         2. Double-check process name matches expected\n\
                         3. Handle \"process not found\" gracefully (may already be gone)\n\
                         4. Log kills for debugging and audit trails\n\
                         5. Consider graceful shutdown alternatives first\n\
                         6. Check for child processes before killing parent\n\
                         7. Clean up lock files and temp files after kill if needed\n\
                         8. Wrap in try/catch - don't assume success\n\n\
                         WHEN NOT TO USE process_kill:\n\
                         - Process can exit gracefully (use proper shutdown)\n\
                         - Process is actively saving data (wait for completion)\n\
                         - Process is a critical system service (use service manager)\n\
                         - You're not sure what the process does (investigate first)\n\
                         - Process PID is from old/stale data (refresh with process_list)",
                    ),
                },
            ]);
        } else {
            // Add brief best practices
            messages.extend(vec![
                PromptMessage {
                    role: PromptMessageRole::User,
                    content: PromptMessageContent::text("What are the key best practices?"),
                },
                PromptMessage {
                    role: PromptMessageRole::Assistant,
                    content: PromptMessageContent::text(
                        "Key best practices for process_kill:\n\n\
                         1. ALWAYS verify PID first\n\
                         Use process_list to find and confirm the process before killing.\n\n\
                         2. Handle errors gracefully\n\
                         Process may already be gone - this is often fine.\n\
                         Wrap kills in try/catch for robust code.\n\n\
                         3. Consider child processes\n\
                         Killing parent leaves children orphaned.\n\
                         Check for children with process_list if needed.\n\n\
                         4. Prefer graceful shutdown when possible\n\
                         SIGKILL is a last resort for truly hung processes.\n\
                         Try proper shutdown mechanisms first.\n\n\
                         5. Remember it's NOT idempotent\n\
                         Second kill will fail with \"process not found\".\n\
                         Don't retry kills without checking if process exists.\n\n\
                         6. Never kill system-critical processes\n\
                         Stick to application processes you control.\n\
                         System processes (PID 1, kernel threads) are protected.\n\n\
                         7. Log your actions\n\
                         Record what you killed and why for debugging.\n\n\
                         Common Pattern:\n\
                         process_list → verify → process_kill → handle errors → cleanup",
                    ),
                },
            ]);
        }

        Ok(messages)
    }
}
