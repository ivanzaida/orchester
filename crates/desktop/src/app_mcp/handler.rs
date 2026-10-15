//! The MCP side of the server: what it announces, its tool list, and each call answered with the
//! tool's result or its typed error, as a tool error rather than a protocol error.

use std::{borrow::Cow, sync::Arc};

use orchester_workspace::app_mcp::{self, Services, Translate, call, listed};
use rmcp::{
  ErrorData as McpError, RoleServer, ServerHandler,
  model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, Implementation, ListToolsResult, PaginatedRequestParams,
    ProtocolVersion, ServerCapabilities, ServerConfig,
  },
  service::RequestContext,
};

/// What clients are told about the server.
const INSTRUCTIONS: &str = "Orchester's own MCP server: control Orchester's projects, tasks, runs, \
  approvals and pipelines through data. Every tool goes through the same rules as the app's screens; \
  a refusal comes back as a tool error with a stable `code` and the app's own explanation. Name a \
  project by its id or name, a task by its key (KON-12), a run by its id or number. decide_approval \
  works only while a person has turned on Let automation answer approvals in the app. To look at \
  the window itself (screenshots, the element tree), use the app's other MCP server, lurq's.";

#[derive(Clone)]
pub struct Handler {
  services: Services,
  translate: Arc<Translate>,
}

impl Handler {
  pub fn new(services: Services, translate: Translate) -> Self {
    Self {
      services,
      translate: Arc::new(translate),
    }
  }
}

impl ServerHandler for Handler {
  fn get_info(&self) -> ServerConfig {
    let mut info = ServerConfig::default();
    info.capabilities = ServerCapabilities::builder().enable_tools().build();
    info.server_info = Implementation::new("orchester", env!("CARGO_PKG_VERSION").to_owned());
    info.instructions = Some(INSTRUCTIONS.to_owned());
    info
  }

  fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
    app_mcp::supported_protocol_versions()
  }

  async fn list_tools(
    &self,
    _request: Option<PaginatedRequestParams>,
    _context: RequestContext<RoleServer>,
  ) -> Result<ListToolsResult, McpError> {
    Ok(ListToolsResult {
      tools: listed(),
      ..Default::default()
    })
  }

  async fn call_tool(
    &self,
    request: CallToolRequestParams,
    _context: RequestContext<RoleServer>,
  ) -> Result<CallToolResponse, McpError> {
    let name = request.name.to_string();
    let arguments = request.arguments.unwrap_or_default();
    let Some(result) = call(&self.services, &self.translate, &name, arguments).await else {
      return Err(McpError::invalid_params(format!("unknown tool: {name}"), None));
    };
    let result = match result {
      Ok(value) => CallToolResult::structured(value),
      Err(error) => {
        tracing::info!(tool = %name, ?error, "an MCP tool refused");
        CallToolResult::structured_error(error.to_json(|message| self.translate.text(message)))
      }
    };
    Ok(result.into())
  }
}
