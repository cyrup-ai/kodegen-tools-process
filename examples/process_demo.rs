mod common;

use anyhow::Context;
use kodegen_mcp_client::tools;
use serde_json::json;
use tracing::{error, info};

// Response structures for process_list tool
#[derive(Debug, serde::Deserialize)]
struct ProcessInfo {
    pid: u32,
    name: String,
    cpu_percent: f32,
    memory_mb: f64,
}

#[derive(Debug, serde::Deserialize)]
struct ProcessListResult {
    processes: Vec<ProcessInfo>,
    total_count: usize,
    filter: Option<String>,
    limited: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt().with_env_filter("info").init();

    info!("Starting process tools example");

    // Connect to kodegen server with process category
    let (conn, mut server) =
        common::connect_to_local_http_server().await?;

    // Wrap client with logging
    let workspace_root = common::find_workspace_root()
        .context("Failed to find workspace root")?;
    let log_path = workspace_root.join("tmp/mcp-client/process.log");
    let client = common::LoggingClient::new(conn.client(), log_path)
        .await
        .context("Failed to create logging client")?;

    info!("Connected to server: {:?}", client.server_info());

    // 1. PROCESS_LIST - List all running processes
    info!("1. Testing process_list");
    match client.call_tool(tools::PROCESS_LIST, json!({})).await {
        Ok(result) => {
            // Extract text content from CallToolResult
            if let Some(text_content) = result.content.first().and_then(|c| c.as_text()) {
                // Deserialize the JSON result into typed structure
                match serde_json::from_str::<ProcessListResult>(&text_content.text) {
                Ok(list) => {
                    info!("✅ Found {} processes", list.total_count);
                    
                    if list.limited {
                        info!("   (Results limited - showing top processes by CPU usage)");
                    }
                    
                    if let Some(ref filter) = list.filter {
                        info!("   (Filtered by: {})", filter);
                    }
                    
                    info!("   Top {} processes by CPU usage:", list.processes.len().min(10));
                    
                    // Show top 10 processes in formatted table
                    for (i, proc) in list.processes.iter().take(10).enumerate() {
                        // Truncate process name to 20 characters (UTF-8 safe)
                        let name_display = if proc.name.chars().count() > 20 {
                            let truncated: String = proc.name.chars().take(17).collect();
                            format!("{}...", truncated)
                        } else {
                            proc.name.clone()
                        };

                        info!(
                            "   {:2}. PID {:6} | {:20} | CPU: {:5.1}% | Mem: {:7.1} MB",
                            i + 1,
                            proc.pid,
                            name_display,
                            proc.cpu_percent,
                            proc.memory_mb
                        );
                    }
                }
                Err(e) => error!("Failed to parse process list: {}", e),
            }
            } else {
                error!("No text content in response from process_list tool");
            }
        }
        Err(e) => error!("Failed to list processes: {}", e),
    }

    // 2. PROCESS_KILL - Kill a process (demonstration only - not actually killing)
    info!("2. Testing process_kill (demo with invalid PID)");
    // Note: Using an invalid PID to demonstrate without actually killing anything
    match client
        .call_tool(
            tools::PROCESS_KILL,
            json!({ "pid": 999999 }), // Invalid PID for demo
        )
        .await
    {
        Ok(result) => info!("Kill process result: {:?}", result),
        Err(e) => info!("Expected error for invalid PID: {}", e),
    }

    // Graceful shutdown
    conn.close().await?;
    server.shutdown().await?;
    info!("Process tools example completed successfully");

    Ok(())
}
