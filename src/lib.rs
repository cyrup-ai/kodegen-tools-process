/// Platform-specific process ID type
///
/// This matches `tokio::process::Child::id()` return type and ensures
/// consistent PID handling across all process-related tools.
pub type ProcessId = u32;

pub mod process_list;
pub use process_list::*;

pub mod process_kill;
pub use process_kill::*;

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
    let session_keep_alive = Duration::ZERO;

    create_http_server("process", addr, tls_config, shutdown_timeout, session_keep_alive, |_config, _tracker| {
        Box::pin(async move {
            let tool_router = ToolRouter::new();
            let prompt_router = PromptRouter::new();
            let managers = Managers::new();

            // Register all 2 process tools
            let (tool_router, prompt_router) = register_tool(
                tool_router,
                prompt_router,
                crate::ProcessListTool::new(),
            );

            let (tool_router, prompt_router) = register_tool(
                tool_router,
                prompt_router,
                crate::ProcessKillTool::new(),
            );

            Ok(RouterSet::new(tool_router, prompt_router, managers))
        })
    }).await
}

/// Start process tools HTTP server using pre-bound listener (TOCTOU-safe)
///
/// This variant is used by kodegend to eliminate TOCTOU race conditions
/// during port cleanup. The listener is already bound to a port.
///
/// # Arguments
/// * `listener` - Pre-bound TcpListener (port already reserved)
/// * `tls_config` - Optional (cert_path, key_path) for HTTPS
///
/// # Returns
/// ServerHandle for graceful shutdown, or error if startup fails
pub async fn start_server_with_listener(
    listener: tokio::net::TcpListener,
    tls_config: Option<(std::path::PathBuf, std::path::PathBuf)>,
) -> anyhow::Result<kodegen_server_http::ServerHandle> {
    use kodegen_server_http::{create_http_server_with_listener, Managers, RouterSet, register_tool};
    use rmcp::handler::server::router::{prompt::PromptRouter, tool::ToolRouter};
    use std::time::Duration;

    let shutdown_timeout = Duration::from_secs(30);
    let session_keep_alive = Duration::ZERO;

    create_http_server_with_listener("process", listener, tls_config, shutdown_timeout, session_keep_alive, |_config, _tracker| {
        Box::pin(async move {
            let tool_router = ToolRouter::new();
            let prompt_router = PromptRouter::new();
            let managers = Managers::new();

            // Register all 2 process tools
            let (tool_router, prompt_router) = register_tool(
                tool_router,
                prompt_router,
                crate::ProcessListTool::new(),
            );

            let (tool_router, prompt_router) = register_tool(
                tool_router,
                prompt_router,
                crate::ProcessKillTool::new(),
            );

            Ok(RouterSet::new(tool_router, prompt_router, managers))
        })
    }).await
}
