//! Window input and application-owned permission consent are separate authorizations. Settings and unrelated
//! sensitive confirmations always remain person-only; a classified setup widening may be answered under the
//! stored permission opt-in (automatic approvals also need their separate opt-in). The server re-reads consent
//! off the UI thread before forwarding input; lurq's event-loop denial and confirmation handlers recheck it.
//! Looking stays allowed. Neither consent nor this gate grants window-control scopes or provider permissions.

mod local;
use std::sync::{
  Arc,
  atomic::{AtomicBool, AtomicUsize, Ordering},
};

pub use local::LocalInput;
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

/// What a refused client reads while Settings shows *Let automation change agent permissions*.
const SETTING_REFUSAL: &str = "A person changes this setting in the window: while Settings shows Let automation \
  change agent permissions, window input from MCP clients (clicks, keys, values, resizing, menus) is refused.";
/// What a refused client reads while a Connections page asks a person to confirm what a change widens.
const QUESTION_REFUSAL: &str = "A person answers this in the window: while Local integration, a Connections page, Agents page, Tools page, Secrets page, Project settings or Pipelines page asks to \
  confirm what a change lets agents do (in any tab, until it is answered or the page is left), \
  window input from MCP clients (clicks, keys, values, resizing, menus) is refused.";
const STILL_AVAILABLE: &str = "Looking at the window and the app's own tools stay available.";

/// Which person-only control is mounted, for the words a refused client reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Guarded {
  /// Settings' *Let automation change agent permissions*.
  PermissionsSetting,
  /// A Connections page's question that confirms what saving or turning on a server widens.
  WideningQuestion,
  /// A currently mounted reviewed integration decision requiring a native answer.
  LocalIntegrationQuestion,
  /// A domain setup widening covered by the human permission opt-in.
  PermissionWidening,
  /// Automatic approvals additionally require the separate approvals opt-in.
  ApprovalWidening,
}

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
  window_control: AtomicBool,
  local_epoch: AtomicUsize,
  controls: Mutex<Controls>,
  workspace: Mutex<Option<orchester_workspace::workspace::Workspace>>,
  /// Window-input calls the server forwarded that have not answered.
  in_flight: AtomicUsize,
  /// lurq's control of its tools once the server started, and the tools the gate denied through it.
  tools: Mutex<Option<(McpHandle, Vec<&'static str>)>>,
}

#[derive(Default)]
struct Controls {
  settings: usize,
  questions: usize,
  permissions: usize,
  approvals: usize,
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
    *self.0.tools.lock() = Some((tools, Vec::new()));
    self.refresh_tools();
  }

  /// Marks the protected control `what` as mounted until the returned guard is dropped; the first one makes lurq
  /// deny its input tools. Called on the event loop, which runs lurq's tool calls.
  pub fn protect(&self, what: Guarded) -> Protected {
    self.0.local_epoch.fetch_add(1, Ordering::AcqRel);
    self.count(what, true);
    self.refresh_tools();
    Protected(self.clone(), what)
  }

  /// Connects the session's stored authority without broadening window-control scopes.
  pub fn bind_permissions(&self, workspace: orchester_workspace::workspace::Workspace) {
    *self.0.workspace.lock() = Some(workspace);
    self.refresh_tools();
  }

  /// Rechecked at delivery, so a forwarded call cannot widen after consent was revoked.
  pub fn widening_allowed(&self, what: Guarded) -> bool {
    let workspace = self.0.workspace.lock();
    let Some(workspace) = workspace.as_ref() else {
      return false;
    };
    match what {
      Guarded::PermissionWidening => workspace.automation_permission_consent(),
      Guarded::ApprovalWidening => workspace.automation_permission_consent() && workspace.automation_approval_consent(),
      Guarded::PermissionsSetting | Guarded::WideningQuestion | Guarded::LocalIntegrationQuestion => false,
    }
  }

  fn count(&self, what: Guarded, add: bool) {
    let mut controls = self.0.controls.lock();
    let count = match what {
      Guarded::PermissionsSetting => &mut controls.settings,
      Guarded::WideningQuestion | Guarded::LocalIntegrationQuestion => &mut controls.questions,
      Guarded::PermissionWidening => &mut controls.permissions,
      Guarded::ApprovalWidening => &mut controls.approvals,
    };
    if add {
      *count += 1;
    } else {
      *count -= 1;
    }
  }

  /// Applies current consent to lurq's event-loop tool denial too, including already queued calls.
  pub fn refresh_tools(&self) {
    let protecting = self.protecting();
    if let Some((tools, denied)) = self.0.tools.lock().as_mut() {
      if protecting {
        for tool in INPUT {
          if !denied.contains(tool) {
            tools.deny_tool(*tool);
            denied.push(tool);
          }
        }
      } else {
        for tool in denied.drain(..) {
          tools.allow_tool(tool);
        }
      }
    }
  }

  async fn refresh_consent(&self) {
    let workspace = self.0.workspace.lock().clone();
    if let Some(workspace) = workspace {
      let read = workspace.clone();
      let result = orchester_workspace::background::run(move || {
        workspace.automation_changes_permissions()?;
        workspace.automation_answers_approvals()?;
        Ok::<(), orchester_store::StoreError>(())
      })
      .await;
      if !matches!(result, Ok(Ok(()))) {
        read.invalidate_automation_consent();
        // Failed background work also invalidates an earlier successful authority read.
        tracing::warn!("automation consent read failed; window widening remains protected");
      }
    }
    self.refresh_tools();
  }

  /// Why window input is refused now: the protected controls mounted, in words for the client.
  fn refusal(&self) -> String {
    let controls = self.0.controls.lock();
    let questions = controls.questions + controls.permissions + controls.approvals;
    let settings = controls.settings;
    let mut words: Vec<&str> = Vec::new();
    if settings > 0 {
      words.push(SETTING_REFUSAL);
    }
    if questions > 0 {
      words.push(QUESTION_REFUSAL);
    }
    words.push(STILL_AVAILABLE);
    words.join(" ")
  }

  /// Whether a protected control is mounted.
  pub fn protecting(&self) -> bool {
    let controls = self.0.controls.lock();
    controls.settings > 0
      || controls.questions > 0
      || (controls.permissions > 0 && !self.widening_allowed(Guarded::PermissionWidening))
      || (controls.approvals > 0 && !self.widening_allowed(Guarded::ApprovalWidening))
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
pub struct Protected(InputGate, Guarded);

impl Drop for Protected {
  fn drop(&mut self) {
    self.0.count(self.1, false);
    self.0.refresh_tools();
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
    self.gate.refresh_consent().await;
    if self.gate.protecting() {
      tracing::info!(tool = %request.name, "window input refused while a person-only control shows");
      let refusal = serde_json::json!({ "code": "person_only", "message": self.gate.refusal() });
      return Ok(CallToolResult::error(vec![ContentBlock::text(refusal.to_string())]).into());
    }
    // Only calls that go on to the window count, so refused calls cannot keep a person's change out.
    self.gate.0.in_flight.fetch_add(1, Ordering::SeqCst);
    let _in_flight = InFlight(self.gate.clone());
    self.tools.call_tool(request, context).await
  }
}
