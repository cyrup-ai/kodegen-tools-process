# kodegen-tools-process

[![License](https://img.shields.io/badge/license-Apache%202.0%20OR%20MIT-blue.svg)](LICENSE.md)
[![Rust](https://img.shields.io/badge/rust-nightly-orange.svg)](https://www.rust-lang.org/)

Memory-efficient, blazing-fast MCP (Model Context Protocol) tools for process management in code generation agents.

Part of the [KODEGEN.ᴀɪ](https://kodegen.ai) ecosystem.

## Features

- **List Processes**: Query all running processes with CPU usage, memory consumption, and filtering capabilities
- **Kill Processes**: Safely terminate processes by PID with proper error handling
- **MCP Compatible**: Full Model Context Protocol support for AI agent integration
- **HTTP/HTTPS Server**: Ready-to-deploy server with SSE streaming support
- **Type-Safe**: Leverages Rust's type system for robust process management
- **Cross-Platform**: Works on macOS, Linux, and Windows

## Tools

### `process_list`

List all running processes with detailed metrics:

```json
{
  "filter": "python",
  "limit": 10
}
```

Returns:
- Process ID (PID)
- Process name/command
- CPU usage percentage
- Memory usage in MB

Results are sorted by CPU usage (highest first).

### `process_kill`

Terminate a process by PID:

```json
{
  "pid": 12345
}
```

Sends SIGKILL signal for immediate termination. Use with caution as this prevents graceful shutdown.

## Installation

### Prerequisites

- Rust nightly toolchain (required for edition 2024)
- Cargo package manager

```bash
# Install Rust nightly
rustup toolchain install nightly
rustup override set nightly
```

### Building from Source

```bash
# Clone the repository
git clone https://github.com/cyrup-ai/kodegen-tools-process
cd kodegen-tools-process

# Build the library
cargo build --release

# Build the server binary
cargo build --release --bin kodegen-process
```

## Usage

### Running the HTTP Server

```bash
# Start server on default port (30447)
cargo run --bin kodegen-process -- --http 127.0.0.1:30447
```

The server exposes MCP tools via HTTP at `http://127.0.0.1:30447/mcp`.

### Example Client Usage

```rust
use kodegen_mcp_client::{create_streamable_client, tools};
use serde_json::json;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Connect to server
    let (client, conn) = create_streamable_client("http://127.0.0.1:30447/mcp").await?;

    // List all processes
    let result = client.call_tool(tools::PROCESS_LIST, json!({})).await?;
    println!("Processes: {:?}", result);

    // List filtered processes
    let result = client.call_tool(
        tools::PROCESS_LIST,
        json!({"filter": "rust", "limit": 5})
    ).await?;

    // Kill a process (use with caution!)
    let result = client.call_tool(
        tools::PROCESS_KILL,
        json!({"pid": 12345})
    ).await?;

    conn.close().await?;
    Ok(())
}
```

### Running Examples

```bash
# Run the process demo example
cargo run --example process_demo
```

The example demonstrates:
- Connecting to the local HTTP server
- Listing processes with filtering
- Safe kill process testing (with invalid PID)
- JSONL logging to `tmp/mcp-client/process.log`

## Development

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test process_list
```

### Code Quality

```bash
# Format code
cargo fmt

# Run linter
cargo clippy

# Check code without building
cargo check
```

## Architecture

The codebase follows the KODEGEN MCP Tool pattern:

1. **Tool Implementation** - Each tool implements the `Tool` trait with:
   - Type-safe arguments via `Args` and `PromptArgs`
   - Async `execute()` method for core logic
   - Built-in prompt support for AI agents

2. **Process Management** - Uses `sysinfo` crate wrapped in `tokio::task::spawn_blocking` to avoid blocking the async runtime

3. **HTTP Server** - Built on `kodegen_server_http` with automatic tool registration and routing

See [CLAUDE.md](CLAUDE.md) for detailed architectural documentation.

## Dependencies

- **kodegen_mcp_tool** - MCP tool framework
- **rmcp** - MCP SDK for protocol implementation
- **sysinfo** - Cross-platform process information
- **tokio** - Async runtime
- **serde** - Serialization framework

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE.md) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE.md) or http://opensource.org/licenses/MIT)

at your option.

## Links

- **Homepage**: https://kodegen.ai
- **Repository**: https://github.com/cyrup-ai/kodegen-tools-process
- **Documentation**: See [CLAUDE.md](CLAUDE.md) for development guide
