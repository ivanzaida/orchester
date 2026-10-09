//! Keeps MCP clients' window input away from a person-only control (*Let automation change agent
//! permissions*). lurq's synthetic input reaches the same handlers as a person's, so the control cannot
//! tell them apart; the app's server can, because every synthetic input arrives as a call of one of lurq's
//! input tools. Two locks, together complete:
//!
//! - While a protected control is mounted, the server refuses every window-input tool (any `lurq_*` tool but those that
//!   only look) with `person_only`, whatever its target: an element, coordinates, or keys sent to the focused control.
//! - While such a call is in flight (lurq applies its input on the event loop before it answers), a protected control
//!   takes no change: an input call checked just before the control mounted cannot flip it either.
//!
//! Looking stays allowed, and so do the app's own tools: `navigate` can show Settings, but nothing can
//! press its switch.

use std::sync::{
  Arc,
  atomic::{AtomicUsize, Ordering},
};

use lurq::mcp::{
  McpService,
  rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler,
    model::{
      CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, ListToolsResult, PaginatedRequestParams,
      ServerConfig,
    },
    service::RequestContext,
  },
};

/// lurq's tools that only look at the window; every other `lurq_*` tool sends it input or changes it.
const LOOKING: &[&str] = &[
  "lurq_file_dialogs",
  "lurq_find",
  "lurq_find_by_class",
  "lurq_find_by_id",
  "lurq_inspect",
  "lurq_logs",
  "lurq_profile_end",
  "lurq_profile_read",
  "lurq_profile_start",
  "lurq_read_tree",
  "lurq_screenshot",
  "lurq_wait",
  "lurq_windows",
];

const REFUSAL: &str = "A person changes this setting in the window: while Settings shows Let automation change agent \
  permissions, window input from MCP clients (clicks, keys, values, resizing, menus) is refused. Looking at the window \
  and the app's own tools stay available.";

/// Whether `tool` sends the window input or changes it, so a protected control could be reached through it.
pub fn sends_input(tool: &str) -> bool {
  tool.starts_with("lurq_") && !LOOKING.contains(&tool)
}

#[derive(Default)]
struct Counts {
  /// Protected controls mounted.
  protected: AtomicUsize,
  /// Window-input calls the server forwarded that have not answered.
  in_flight: AtomicUsize,
}

/// Shared between the app's MCP server and the window. Clones share the same counts.
#[derive(Clone, Default)]
pub struct InputGate(Arc<Counts>);

impl PartialEq for InputGate {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl lurq::app::component::DevtoolsInspectable for InputGate {}

impl InputGate {
  /// Marks a protected control as mounted until the returned guard is dropped.
  pub fn protect(&self) -> Protected {
    self.0.protected.fetch_add(1, Ordering::SeqCst);
    Protected(self.clone())
  }

  /// Whether a protected control is mounted.
  pub fn protecting(&self) -> bool {
    self.0.protected.load(Ordering::SeqCst) > 0
  }

  /// Whether an MCP client's window input is being applied now: a change a protected control hears now
  /// came from it, not from a person.
  pub fn input_in_flight(&self) -> bool {
    self.0.in_flight.load(Ordering::SeqCst) > 0
  }

  /// lurq's tools for one client session, behind this gate.
  pub fn guard(&self, tools: McpService) -> GuardedTools {
    GuardedTools {
      tools,
      gate: self.clone(),
    }
  }
}

/// A protected control's mark; dropping it, when the control unmounts, lifts the lock.
pub struct Protected(InputGate);

impl Drop for Protected {
  fn drop(&mut self) {
    self.0.0.protected.fetch_sub(1, Ordering::SeqCst);
  }
}

/// Counts a forwarded input call while it is in flight.
struct InFlight(InputGate);

impl Drop for InFlight {
  fn drop(&mut self) {
    self.0.0.in_flight.fetch_sub(1, Ordering::SeqCst);
  }
}

/// lurq's tools of one client session, refusing window input while a protected control is mounted.
#[derive(Clone)]
pub struct GuardedTools {
  tools: McpService,
  gate: InputGate,
}

impl ServerHandler for GuardedTools {
  fn get_info(&self) -> ServerConfig {
    self.tools.get_info()
  }

  async fn list_tools(
    &self,
    request: Option<PaginatedRequestParams>,
    context: RequestContext<RoleServer>,
  ) -> Result<ListToolsResult, McpError> {
    self.tools.list_tools(request, context).await
  }

  async fn call_tool(
    &self,
    request: CallToolRequestParams,
    context: RequestContext<RoleServer>,
  ) -> Result<CallToolResponse, McpError> {
    if !sends_input(&request.name) {
      return self.tools.call_tool(request, context).await;
    }
    self.gate.0.in_flight.fetch_add(1, Ordering::SeqCst);
    let _in_flight = InFlight(self.gate.clone());
    if self.gate.protecting() {
      tracing::info!(tool = %request.name, "window input refused while a person-only control shows");
      let refusal = serde_json::json!({ "code": "person_only", "message": REFUSAL });
      return Ok(CallToolResult::error(vec![ContentBlock::text(refusal.to_string())]).into());
    }
    self.tools.call_tool(request, context).await
  }
}
