//! What is left of Connections in the app's real window, on the QA seed's projects (`examples/seed`, the
//! `connections` scenario): the tab and the pages of agent connections are gone (`gone`); MCP servers are added,
//! edited, turned off and on and removed (`mcp`), a remote server is authorized (`mcp_authorization`), a decision
//! model's page, its editor and its test (`decision`, `decision_edit`, `decision_test`), and GitHub and MCP
//! connections are renamed (`rename`). No runner serves the project. Needs `git`, as the seed does.

#[path = "../../examples/seed/kontur/mod.rs"]
#[allow(
  dead_code,
  reason = "the seed's helpers serve every scenario; these tests seed a few"
)]
mod kontur;
#[path = "../support/mod.rs"]
mod support;

mod decision;
mod decision_edit;
mod decision_test;
mod gone;
mod harness;
mod mcp;
mod mcp_authorization;
mod rename;
