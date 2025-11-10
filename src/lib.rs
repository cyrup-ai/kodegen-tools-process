/// Platform-specific process ID type
///
/// This matches `tokio::process::Child::id()` return type and ensures
/// consistent PID handling across all process-related tools.
pub type ProcessId = u32;

pub mod list_processes;
pub use list_processes::*;

pub mod kill_process;
pub use kill_process::*;

/// Start the process tools HTTP server programmatically
///
/// Returns a ServerHandle for graceful shutdown control.
/// This function is non-blocking - the server runs in background tasks.
///
/// # Arguments
/// * `addr` - Socket address to bind to (e.g., "127.0.0.1:30439")
/// * `tls_cert` - Optional path to TLS certificate file
/// * `tls_key` - Optional path to TLS private key file
///
/// # Returns
/// ServerHandle for graceful shutdown, or error if startup fails
pub async fn start_server(
    addr: std::net::SocketAddr,
    tls_cert: Option<std::path::PathBuf>,
    tls_key: Option<std::path::PathBuf>,
) -> anyhow::Result<kodegen_server_http::ServerHandle> {
    use kodegen_server_http::{create_http_server, Managers, RouterSet, register_tool};
    use rmcp::handler::server::router::{prompt::PromptRouter, tool::ToolRouter};
    use std::time::Duration;

    let tls_config = match (tls_cert, tls_key) {
        (Some(cert), Some(key)) => Some((cert, key)),
        _ => None,
    };

    let shutdown_timeout = Duration::from_secs(30);

    create_http_server("process", addr, tls_config, shutdown_timeout, |_config, _tracker| {
        Box::pin(async move {
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

            Ok(RouterSet::new(tool_router, prompt_router, managers))
        })
    }).await
}
