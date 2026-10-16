//! The Orchestrator chat's domain tools (`orchester_workspace::orchestrator::tools`), served to the chat's agent on
//! a loopback server of their own: one per turn, started when its message is sent and stopped once the turn ended,
//! with a bearer token of its own that only the project's runner is given, for the agent's MCP configuration; a
//! token stops working with its turn. It serves none of
//! the window's tools and none of the app's other tools, and its token is not the app server's, so the agent never
//! reaches the app's own server. Each call goes through the same tool an MCP client of the app calls
//! ([`super::domain`]), so it is refused or carried out exactly as for that client; while this computer is signed out
//! of Orchester Cloud every call is refused, as there (`input_gate::signed_out`). The servers still under way stop with
//! the session.

use std::{
  borrow::Cow,
  collections::HashMap,
  path::{Path, PathBuf},
  sync::Arc,
  time::Duration,
};

use lurq::{
  app::i18n::I18n,
  mcp::rmcp::{
    ErrorData as McpError, ServerHandler,
    model::{
      CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation, ListToolsResult,
      PaginatedRequestParams, ProtocolVersion, ServerCapabilities, ServerConfig,
    },
    service::RequestContext,
  },
};
use orchester_core::{id::ChatMessageId, orchestrator::TurnState};
use orchester_ipc::{ChatToken, ChatTools};
use orchester_store::ProjectDb;
use orchester_workspace::{
  app_mcp::{self, Translate},
  orchestrator::tools::{self, ChatScope},
  runner::Runners,
  sharing::cloud::Cloud,
};
use parking_lot::Mutex;
use serde_json::Value;

use super::{
  APP_NAME, domain,
  input_gate::signed_out,
  server::{BindError, Listening, endpoint},
  token::BearerToken,
};
use crate::services::Services;

/// What the chat's agent is told about the server.
const INSTRUCTIONS: &str = "Orchester's tools for one project's Orchestrator chat: read the project's tasks, runs, \
  approvals and pipelines, create and change task drafts, run a task, pause and resume a run. Name the project by \
  its id or name. A refusal is a tool error with a stable `code` and the app's own explanation in `reasons`.";

/// How long a served turn waits for a change before it is read once more: a runner that went away says nothing.
const RECHECK: Duration = Duration::from_secs(5);

/// The servers of the turns under way.
#[derive(Clone, Default)]
pub struct ChatServers {
  running: Arc<Mutex<HashMap<ChatMessageId, Running>>>,
}

struct Running {
  /// Dropping it stops the server.
  _listening: Listening,
  tools: ChatTools,
}

impl ChatServers {
  /// Serves the tools of the turn that answers `message` in `scope`'s conversation: a server and token of its own,
  /// for as long as the turn runs ([`Self::end_with_turn`]); the same turn asked for again gets the same. Tools
  /// explain a refusal in the words of `i18n`, in the locale it has at each call.
  pub fn serve_turn(
    &self,
    services: &Services,
    i18n: I18n,
    (scope, message): (ChatScope, ChatMessageId),
  ) -> Result<ChatTools, BindError> {
    let mut running = self.running.lock();
    if let Some(running) = running.get(&message) {
      return Ok(running.tools.clone());
    }
    let handler = ChatHandler {
      services: Arc::new(domain::on(services)),
      translate: Arc::new(domain::translate(i18n)),
      runners: services.runners.clone(),
      cloud: services.cloud.clone(),
      scope: Arc::new(scope),
    };
    let token = BearerToken::generate();
    let listening = Listening::serve_sessions(&services.runtime, None, move || handler.clone(), token.clone())?;
    let tools = ChatTools {
      url: endpoint(listening.port()),
      token: ChatToken::new(token.expose().to_owned()),
    };
    running.insert(
      message,
      Running {
        _listening: listening,
        tools: tools.clone(),
      },
    );
    Ok(tools)
  }

  /// Stops serving the turn that answers `message` once it no longer thinks in the project at `root`: its token
  /// stops working with it.
  pub fn end_with_turn(&self, services: &Services, root: PathBuf, message: ChatMessageId) {
    let (servers, runners) = (self.clone(), services.runners.clone());
    services.runtime.spawn(async move {
      let mut changes = runners.chats_of(&root);
      while thinking(&root, message).await {
        match tokio::time::timeout(RECHECK, changes.changed()).await {
          Ok(Ok(())) | Err(_) => {}
          // The project's counters went away with its runner's link: only the time says when to read again.
          Ok(Err(_)) => tokio::time::sleep(RECHECK).await,
        }
      }
      servers.end(message);
    });
  }

  /// Stops serving the turn that answers `message` now: the runner never took it.
  pub fn end(&self, message: ChatMessageId) {
    self.running.lock().remove(&message);
  }
}

/// Whether the turn that answers `message` still thinks in the project at `root`. One that cannot be read is taken
/// as thinking: it is read again later.
async fn thinking(root: &Path, message: ChatMessageId) -> bool {
  let root = root.to_path_buf();
  match tokio::task::spawn_blocking(move || ProjectDb::open(&root)?.chat_turn(message)).await {
    Ok(Ok(turn)) => turn.is_some_and(|turn| turn.turn == TurnState::Thinking),
    Ok(Err(error)) => {
      tracing::warn!(%message, %error, "cannot read a turn of the Orchestrator chat; its tools stay served");
      true
    }
    Err(error) => {
      tracing::warn!(%message, %error, "reading a turn of the Orchestrator chat did not finish; its tools stay served");
      true
    }
  }
}

/// One client session of a conversation's server.
#[derive(Clone)]
struct ChatHandler {
  services: Arc<app_mcp::Services>,
  translate: Arc<Translate>,
  runners: Runners,
  /// Signed out of the cloud, every call is refused.
  cloud: Cloud,
  scope: Arc<ChatScope>,
}

impl ServerHandler for ChatHandler {
  fn get_info(&self) -> ServerConfig {
    let mut info = ServerConfig::default();
    info.capabilities = ServerCapabilities::builder().enable_tools().build();
    info.server_info = Implementation::new(format!("{APP_NAME}-orchestrator"), env!("CARGO_PKG_VERSION").to_owned());
    info.instructions = Some(INSTRUCTIONS.to_owned());
    info
  }

  fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
    app_mcp::supported_protocol_versions()
  }

  async fn list_tools(
    &self,
    _request: Option<PaginatedRequestParams>,
    _context: RequestContext<lurq::mcp::rmcp::RoleServer>,
  ) -> Result<ListToolsResult, McpError> {
    Ok(ListToolsResult {
      tools: tools::listed(),
      ..ListToolsResult::default()
    })
  }

  async fn call_tool(
    &self,
    request: CallToolRequestParams,
    _context: RequestContext<lurq::mcp::rmcp::RoleServer>,
  ) -> Result<CallToolResponse, McpError> {
    if !self.cloud.is_signed_in() {
      tracing::info!(tool = %request.name, "a tool of the Orchestrator refused while Orchester is signed out");
      let refusal = serde_json::json!({ "code": signed_out::CODE, "message": signed_out::REFUSAL });
      return Ok(CallToolResult::error(vec![ContentBlock::text(refusal.to_string())]).into());
    }
    let arguments = request.arguments.unwrap_or_default();
    let called = tools::call(
      (&self.services, &self.translate, &self.runners),
      &self.scope,
      &request.name,
      arguments,
    )
    .await;
    let result = match called {
      Ok(value) => {
        let text = serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
        let mut result = CallToolResult::success(vec![ContentBlock::text(text)]);
        result.structured_content = Some(value);
        result
      }
      Err(error) => {
        tracing::info!(tool = %request.name, ?error, "a tool of the Orchestrator refused");
        let refused = error.to_json(|message| self.translate.text(message));
        let shown = refused.get("error").cloned().unwrap_or(Value::Null);
        CallToolResult::error(vec![ContentBlock::text(shown.to_string())])
      }
    };
    Ok(result.into())
  }
}
