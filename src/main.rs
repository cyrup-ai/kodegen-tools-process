//! Process Category HTTP Server
//!
//! Serves process management tools via HTTP/HTTPS transport using kodegen_server_http.

use anyhow::Result;
use kodegen_config::CATEGORY_PROCESS;
use kodegen_server_http::{ServerBuilder, Managers, RouterSet, register_tool};
use rmcp::handler::server::router::{prompt::PromptRouter, tool::ToolRouter};

#[tokio::main]
async fn main() -> Result<()> {
    ServerBuilder::new()
        .category(CATEGORY_PROCESS)
        .register_tools(|| async {
            let tool_router = ToolRouter::new();
            let prompt_router = PromptRouter::new();
            let managers = Managers::new();

            // Register all 2 process tools
            let (tool_router, prompt_router) = register_tool(
                tool_router,
                prompt_router,
                kodegen_tools_process::ProcessListTool::new(),
            );

            let (tool_router, prompt_router) = register_tool(
                tool_router,
                prompt_router,
                kodegen_tools_process::ProcessKillTool::new(),
            );

            Ok(RouterSet::new(tool_router, prompt_router, managers))
        })
        .run()
        .await
}
