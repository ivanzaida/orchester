//! The Orchestrator chat's domain tools (`orchester_workspace::orchestrator::tools`), served to the chat's agent on
//! a loopback server of their own: one per conversation, started when its first message is sent, with a bearer
//! token of its own that only the project's runner is given, for the agent's MCP configuration. It serves none of
//! the window's tools and none of the app's other tools, and its token is not the app server's, so the agent never
//! reaches the app's own server. Each call goes through the same tool an MCP client of the app calls
//! ([`super::domain`]), so it is refused or carried out exactly as for that client. The servers stop with the
//! session.

use std::{collections::HashMap, sync::Arc};

use lurq::{
  app::i18n::I18n,
  mcp::rmcp::{
    ErrorData as McpError, ServerHandler,
    model::{
      CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation, ListToolsResult,
      PaginatedRequestParams, ServerCapabilities, ServerConfig,
    },
    service::RequestContext,
  },
};
use orchester_core::id::ChatId;
use orchester_ipc::{ChatToken, ChatTools};
use orchester_workspace::{
  app_mcp::{self, Translate},
  orchestrator::tools::{self, ChatScope},
  runner::Runners,
};
use parking_lot::Mutex;
use serde_json::Value;

use super::{
  APP_NAME, domain,
  server::{BindError, Listening, endpoint},
  token::BearerToken,
};
use crate::services::Services;

/// What the chat's agent is told about the server.
const INSTRUCTIONS: &str = "Orchester's tools for one project's Orchestrator chat: read the project's tasks, runs, \
  approvals and pipelines, create and change task drafts, run a task, pause and resume a run. Name the project by \
  its id or name. A refusal is a tool error with a stable `code` and the app's own explanation in `reasons`.";

/// The servers of the session's conversations.
#[derive(Clone, Default)]
pub struct ChatServers {
  running: Arc<Mutex<HashMap<ChatId, Running>>>,
}

struct Running {
  /// Dropping it stops the server.
  _listening: Listening,
  tools: ChatTools,
}

impl ChatServers {
  /// Where the agent of `scope`'s conversation reaches its tools, and its token; the server starts when first
  /// asked for. Tools explain a refusal in the words of `i18n`.
  pub fn tools_for(&self, services: &Services, i18n: I18n, scope: ChatScope) -> Result<ChatTools, BindError> {
    let mut running = self.running.lock();
    if let Some(running) = running.get(&scope.chat) {
      return Ok(running.tools.clone());
    }
    let chat = scope.chat;
    let handler = ChatHandler {
      services: Arc::new(domain::on(services)),
      translate: Arc::new(domain::translate(i18n)),
      runners: services.runners.clone(),
      scope: Arc::new(scope),
    };
    let token = BearerToken::generate();
    let listening = Listening::serve_sessions(&services.runtime, None, move || handler.clone(), token.clone())?;
    let tools = ChatTools {
      url: endpoint(listening.port()),
      token: ChatToken::new(token.expose().to_owned()),
    };
    running.insert(
      chat,
      Running {
        _listening: listening,
        tools: tools.clone(),
      },
    );
    Ok(tools)
  }
}

/// One client session of a conversation's server.
#[derive(Clone)]
struct ChatHandler {
  services: Arc<app_mcp::Services>,
  translate: Arc<Translate>,
  runners: Runners,
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
