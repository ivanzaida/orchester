//! Connections in the app's real window on the QA seed's projects (`examples/seed`, the `connections`
//! scenario): what the list and the inspectors show and their states (`list`), MCP servers added, edited,
//! turned off and on and removed (`mcp`), a CLI added from the search path (`add_cli`), connections renamed
//! (`rename`) and what Test connection says (`test_result`). No runner serves the project. Needs `git`, as the
//! seed does.

#[path = "../../examples/seed/kontur/mod.rs"]
#[allow(
  dead_code,
  reason = "the seed's helpers serve every scenario; these tests seed a few"
)]
mod kontur;
#[path = "../support/mod.rs"]
mod support;

mod accounts;
mod add_cli;
mod api_provider;
mod harness;
mod layout;
mod list;
mod mcp;
mod qa_scenarios;
mod reads;
mod rename;
mod test_result;
