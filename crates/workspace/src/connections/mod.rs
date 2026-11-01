//! Project connections to agent provider CLIs, to GitHub through `gh` and to MCP servers: reading,
//! checking and saving them, and setting up native agent connections and agent CLIs on API providers. Which agent CLI
//! the start page warns of ([`missing_cli`]), and the file a person chooses for it ([`choose_program`]).

pub mod api_provider;
pub mod choose_program;
pub mod data;
pub mod github;
pub mod mcp;
pub mod missing_cli;
pub mod model_uses;
pub mod native;
pub mod probe;
