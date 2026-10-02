//! Project connections to agent provider CLIs, to GitHub through `gh` and to MCP servers: reading,
//! checking and saving them, and setting up native agent connections.

pub mod data;
pub mod github;
pub mod mcp;
pub mod native;
pub mod probe;
