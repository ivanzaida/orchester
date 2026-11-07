//! The hub's new team MCP server (`Agents · New MCP server` A8 in `acFkz`): its fields, checked as `add_mcp_server`
//! checks them, and its creation as v1 that waits for the team server.

pub use crate::app_mcp::setup::{
  Header, Invalid, NewTeamMcpServer, Runs, create_team_mcp_server, team_mcp_names, team_mcp_server,
};
