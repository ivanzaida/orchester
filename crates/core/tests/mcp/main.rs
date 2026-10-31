//! MCP servers: what a valid server is, how each agent CLI is given the servers of an invocation,
//! and that a secret value reaches the CLI only through its environment or its private
//! configuration file. Run with `cargo test -p orchester-core --test mcp mcp`.

mod authorization;
mod codex_names;
mod exempt;
mod invocation;
mod server;
mod stream;
