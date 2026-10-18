//! Project connections to agent provider CLIs, to GitHub through `gh` and to MCP servers: reading,
//! checking and saving them, and setting up native agent connections and agent CLIs on API providers.

pub mod api_provider;
pub mod data;
pub mod github;
pub mod mcp;
pub mod model_uses;
pub mod native;
pub mod probe;
