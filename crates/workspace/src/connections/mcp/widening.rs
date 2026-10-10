//! What a change to an MCP server widens: what the agents that get it run, reach or may call without a
//! person asking. A new server runs a program or reaches an endpoint of automation's choosing; a server's
//! other program, arguments, folder, endpoint or bearer secret change what runs or where agents connect;
//! a variable but the harmless ones can change what the program loads or trusts, a removal included; a
//! tool that runs without asking is one fewer request a person sees; turning a server on gives it back to
//! the profiles that choose it. A variable that takes a secret's value, under any name, hands the server that
//! secret. Renaming widens too: the answers a person gave in a run (*Approve similar*) name a server's tools by
//! the server's name, so another server renamed onto it would inherit them. Dropping a bearer secret, removing
//! exempt tools and turning a server off narrow. Automation may make these changes only while a person lets it (*Let
//! automation change agent permissions*); a person's own input in the window may always.

use std::collections::{BTreeMap, BTreeSet};

use orchester_core::{
  connection::mcp::{McpEnvValue, McpEnvironment, McpServer, McpTransport},
  permission::widening::harmless_variable,
};

use super::McpServerDraft;
use crate::messages::Message;

/// Why storing `draft` over `stored` (`None`: as a new server) widens what agents may do; empty when it
/// does not.
pub fn widenings(stored: Option<&McpServer>, draft: &McpServerDraft) -> Vec<Message> {
  let Some(stored) = stored else {
    return vec![Message::new("appMcp.widens.mcpServerAdded").with("server", draft.name.as_str().to_owned())];
  };
  let mut reasons = Vec::new();
  if stored.name() != &draft.name {
    reasons.push(Message::new("appMcp.widens.mcpRename").with("server", draft.name.as_str().to_owned()));
  }
  reasons.extend(transport(stored.transport(), &draft.transport));
  reasons.extend(variables(stored.environment(), &draft.environment));
  let added: Vec<&str> = draft
    .exempt_tools
    .tools()
    .iter()
    .map(|tool| tool.as_str())
    .filter(|tool| !stored.exempt_tools().contains(tool))
    .collect();
  if !added.is_empty() {
    reasons.push(Message::new("appMcp.widens.mcpExemptTools").with("tools", added.join(", ")));
  }
  reasons
}

/// What a save may widen, checked against the server as it is stored when the save is made: another change made
/// meanwhile (by automation, or in another window) may widen what the person did not see.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Allowed {
  /// Whatever it widens: a person lets automation widen (*Let automation change agent permissions*).
  Anything,
  /// Only what these reasons say, which a person confirmed; none for a save that asked nothing.
  These(Vec<Message>),
}

impl Allowed {
  /// Whether every one of `reasons` is allowed.
  pub fn covers(&self, reasons: &[Message]) -> bool {
    match self {
      Self::Anything => true,
      Self::These(allowed) => reasons.iter().all(|reason| allowed.contains(reason)),
    }
  }
}

/// Why turning `server` on widens: it was off, and the profiles that choose it get it again.
pub fn enabling(server: &McpServer, enabled: bool) -> Vec<Message> {
  if enabled && !server.enabled() {
    vec![Message::new("appMcp.widens.mcpServerEnabled").with("server", server.name().as_str().to_owned())]
  } else {
    Vec::new()
  }
}

fn transport(stored: &McpTransport, draft: &McpTransport) -> Vec<Message> {
  match (stored, draft) {
    (McpTransport::Stdio(stored), McpTransport::Stdio(draft)) => {
      let mut reasons = Vec::new();
      if stored.program() != draft.program() {
        reasons.push(Message::new("appMcp.widens.mcpProgram").with("program", draft.program().as_str().to_owned()));
      }
      if stored.args() != draft.args() {
        reasons.push(Message::new("appMcp.widens.mcpArguments"));
      }
      if stored.working_directory() != draft.working_directory() {
        reasons.push(Message::new("appMcp.widens.mcpFolder"));
      }
      reasons
    }
    (McpTransport::Http(stored), McpTransport::Http(draft)) => {
      let mut reasons = Vec::new();
      if stored.url() != draft.url() {
        reasons.push(Message::new("appMcp.widens.mcpEndpoint").with("endpoint", draft.url().as_str().to_owned()));
      }
      if draft.bearer().is_some() && stored.bearer() != draft.bearer() {
        reasons.push(Message::new("appMcp.widens.mcpBearer"));
      }
      reasons
    }
    (_, McpTransport::Stdio(draft)) => {
      vec![Message::new("appMcp.widens.mcpProgram").with("program", draft.program().as_str().to_owned())]
    }
    (_, McpTransport::Http(draft)) => {
      vec![Message::new("appMcp.widens.mcpEndpoint").with("endpoint", draft.url().as_str().to_owned())]
    }
  }
}

/// A variable that changes, is added or is removed, but for the harmless ones, and any variable that newly takes a
/// secret's value, whatever its name; a secret is compared by which one it is, never by its value.
fn variables(stored: &McpEnvironment, draft: &McpEnvironment) -> Vec<Message> {
  let old: BTreeMap<String, &McpEnvValue> = stored.iter().map(|(name, value)| (name.to_string(), value)).collect();
  let new: BTreeMap<String, &McpEnvValue> = draft.iter().map(|(name, value)| (name.to_string(), value)).collect();
  old
    .keys()
    .chain(new.keys())
    .collect::<BTreeSet<_>>()
    .into_iter()
    .filter(|name| {
      let (before, after) = (old.get(*name), new.get(*name));
      let secret = matches!(after, Some(McpEnvValue::Secret(_)));
      before != after && (secret || !harmless_variable(name))
    })
    .map(|name| Message::new("appMcp.widens.variable").with("name", name.clone()))
    .collect()
}
