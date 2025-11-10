/// Platform-specific process ID type
///
/// This matches `tokio::process::Child::id()` return type and ensures
/// consistent PID handling across all process-related tools.
pub type ProcessId = u32;

pub mod list_processes;
pub use list_processes::*;

pub mod kill_process;
pub use kill_process::*;

/// Start the process tools HTTP server programmatically.
///
/// This function is designed to be called from kodegend for embedded server mode.
/// It replicates the logic from main.rs but as a library function.
///
/// # Arguments
/// * `addr` - The socket address to bind to
/// * `tls_cert` - Optional path to TLS certificate file
/// * `tls_key` - Optional path to TLS private key file
///
/// # Returns
/// Returns `Ok(())` when the server shuts down gracefully, or an error if startup/shutdown fails.
pub async fn start_server(
    addr: std::net::SocketAddr,
    tls_cert: Option<std::path::PathBuf>,
    tls_key: Option<std::path::PathBuf>,
) -> anyhow::Result<()> {
    use kodegen_server_http::{Managers, RouterSet, register_tool};
    use kodegen_tools_config::ConfigManager;
    use rmcp::handler::server::router::{prompt::PromptRouter, tool::ToolRouter};
    use std::sync::Arc;

    let _ = env_logger::try_init();
    
    let config = ConfigManager::new();
    config.init().await?;
    
    let timestamp = chrono::Utc::now();
    let pid = std::process::id();
    let instance_id = format!("{}-{}", timestamp.format("%Y%m%d-%H%M%S-%9f"), pid);
    kodegen_mcp_tool::tool_history::init_global_history(instance_id.clone()).await;
    
    let tool_router = ToolRouter::new();
    let prompt_router = PromptRouter::new();
    let managers = Managers::new();
    
    // Register all 2 process tools
    let (tool_router, prompt_router) = register_tool(
        tool_router,
        prompt_router,
        crate::ListProcessesTool::new(),
    );
    
    let (tool_router, prompt_router) = register_tool(
        tool_router,
        prompt_router,
        crate::KillProcessTool::new(),
    );
    
    let router_set = RouterSet::new(tool_router, prompt_router, managers);
    
    let session_config = rmcp::transport::streamable_http_server::session::local::SessionConfig {
        channel_capacity: 16,
        keep_alive: Some(std::time::Duration::from_secs(3600)),
    };
    let session_manager = Arc::new(
        rmcp::transport::streamable_http_server::session::local::LocalSessionManager {
            sessions: Default::default(),
            session_config,
        }
    );
    
    let usage_tracker = kodegen_utils::usage_tracker::UsageTracker::new(
        format!("process-{}", instance_id)
    );
    
    let server = kodegen_server_http::HttpServer::new(
        router_set.tool_router,
        router_set.prompt_router,
        usage_tracker,
        config,
        router_set.managers,
        session_manager,
    );
    
    let shutdown_timeout = std::time::Duration::from_secs(30);
    let tls_config = tls_cert.zip(tls_key);
    let handle = server.serve_with_tls(addr, tls_config, shutdown_timeout).await?;
    
    handle.wait_for_completion(shutdown_timeout).await
        .map_err(|e| anyhow::anyhow!("Server shutdown error: {}", e))?;
    
    Ok(())
}
