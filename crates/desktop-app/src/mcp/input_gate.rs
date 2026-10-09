//! Keeps MCP clients' window input away from a person-only control (*Let automation change agent
//! permissions*). lurq's synthetic input reaches the same handlers as a person's, so the control cannot
//! tell them apart; the app can, because every synthetic input arrives as a call of one of lurq's input
//! tools. Three locks:
//!
//! - While a protected control is mounted, lurq itself denies its input tools ([`INPUT`]), checked on the event loop
//!   when a call runs: a call queued before the control mounted, or one whose answer timed out and runs later, is
//!   refused there too. The control denies them as it is created and allows them again when it is dropped.
//! - The app's server refuses every `lurq_*` tool but those that only look ([`LOOKING`]) with `person_only` while a
//!   protected control is mounted, whatever its target (an element, coordinates, keys to the focused control), so a
//!   client reads why; a lurq tool this list does not know counts as input.
//! - While a forwarded input call is in flight, a protected control takes no change that widens (turning its switch
//!   off, which narrows, is always taken), and it takes none before it lived through one pass.
//!
//! Looking stays allowed, and so do the app's own tools: `navigate` can show Settings, but nothing can
//! press its switch.

use std::sync::{
  Arc,
  atomic::{AtomicUsize, Ordering},
};

use lurq::mcp::{
  McpHandle, McpService,
  rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler,
    model::{
      CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, ListToolsResult, PaginatedRequestParams,
      ServerConfig,
    },
    service::RequestContext,
  },
};
use parking_lot::Mutex;

/// lurq's tools that only look at the window; every other `lurq_*` tool sends it input or changes it.
pub const LOOKING: &[&str] = &[
  "lurq_file_dialogs",
  "lurq_find",
  "lurq_find_by_class",
  "lurq_find_by_id",
  "lurq_inspect",
  "lurq_logs",
  "lurq_menu",
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

/// lurq's tools that send the window input or change it, which lurq denies while a protected control is
/// mounted. `tests/mcp_tools.rs` checks that every lurq tool is in this list or in [`LOOKING`].
pub const INPUT: &[&str] = &[
  "lurq_act",
  "lurq_file_dialog_respond",
  "lurq_interact",
  "lurq_navigate",
  "lurq_resize",
  "lurq_set_value",
];

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
  /// lurq's control of its tools, once the server started.
  tools: Mutex<Option<McpHandle>>,
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
  /// Lets the gate deny lurq's input tools through `tools` while a protected control is mounted.
  pub fn attach(&self, tools: McpHandle) {
    *self.0.tools.lock() = Some(tools);
  }

  /// Marks a protected control as mounted until the returned guard is dropped; the first one makes lurq
  /// deny its input tools. Called on the event loop, which runs lurq's tool calls.
  pub fn protect(&self) -> Protected {
    if self.0.protected.fetch_add(1, Ordering::SeqCst) == 0
      && let Some(tools) = self.0.tools.lock().as_ref()
    {
      for tool in INPUT {
        tools.deny_tool(*tool);
      }
    }
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
    if self.0.0.protected.fetch_sub(1, Ordering::SeqCst) == 1
      && let Some(tools) = self.0.0.tools.lock().as_ref()
    {
      for tool in INPUT {
        tools.allow_tool(tool);
      }
    }
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
    if self.gate.protecting() {
      tracing::info!(tool = %request.name, "window input refused while a person-only control shows");
      let refusal = serde_json::json!({ "code": "person_only", "message": REFUSAL });
      return Ok(CallToolResult::error(vec![ContentBlock::text(refusal.to_string())]).into());
    }
    // Only calls that go on to the window count, so refused calls cannot keep a person's change out.
    self.gate.0.in_flight.fetch_add(1, Ordering::SeqCst);
    let _in_flight = InFlight(self.gate.clone());
    self.tools.call_tool(request, context).await
  }
}
