//! `navigate` to Settings (`route` settings): the tab, and the sub-page or question in place it opens on, so a QA
//! copy reaches each of them without a click. An account's page or *Add account*, a team server's page, *Connect a
//! server* at a step, an invite's landing, and *Rotate the token?*, *Sign out…?* and *Decline the invite?* asked in
//! place. Opening a page or a question is going somewhere; what it asks stays a person's to answer, as the window's
//! input gate keeps it. An account and a server are named by their id or their name, read from the session's data.

use orchester_core::id::{AgentAccountId, ServerId};
use orchester_workspace::{sharing::Servers, workspace::Workspace};
use serde_json::{Map, Value, json};

use super::{ToolError, string};
use crate::routes::{
  AccountsPanel, AccountsView, ConnectStep, JoinTarget, OrchesterAsk, OrchesterPanel, OrchesterView, Route,
};

/// The parameters besides `route` that settings takes.
pub const PARAMS: &[&str] = &["tab", "account", "panel", "server", "step", "ask", "invite", "project"];

/// Each tab, and how a refusal names it: `settings tab accounts` takes no `step`.
const TABS: [(&str, &str); 7] = [
  ("automation", "settings tab automation"),
  ("general", "settings tab general"),
  ("notifications", "settings tab notifications"),
  ("accounts", "settings tab accounts"),
  ("orchester", "settings tab orchester"),
  ("connect", "settings tab connect"),
  ("invite", "settings tab invite"),
];

/// The session's data an account or a server is looked up in by its name.
#[derive(Clone)]
pub struct Lookup {
  workspace: Workspace,
  servers: Servers,
}

impl Lookup {
  pub fn of(services: &crate::services::Services) -> Self {
    Self {
      workspace: services.workspace.clone(),
      servers: services.servers.clone(),
    }
  }
}

/// The properties of navigate's schema that settings takes besides `tab`.
pub fn properties() -> Map<String, Value> {
  let steps: Vec<&str> = ConnectStep::ALL
    .map(ConnectStep::name)
    .into_iter()
    .chain(["username"])
    .collect();
  let properties = json!({
    "account": {
      "type": "string",
      "description": "settings tab accounts: the account whose page opens, by its id or name, or claude-default / codex-default.",
    },
    "panel": {
      "type": "string",
      "description": "settings tab accounts: add (Add account), or with `account` sign_in, rename or remove (asked in place).",
    },
    "server": {
      "type": "string",
      "description": "settings tab orchester: the team server whose page opens; tab connect: the server whose address Connect a server starts at; tab invite: the server of the invite's project. By its id, name or host.",
    },
    "step": {
      "type": "string",
      "enum": steps,
      "description": "settings tab connect, with `server`: the step Connect a server opens on; a server with its own certificate stays at compare, which only a person answers.",
    },
    "ask": {
      "type": "string",
      "enum": ["rotate-token", "sign-out", "decline"],
      "description": "settings: the question asked in place: automation rotate-token; orchester sign-out (the cloud account, or with `server` that server) or decline with `invite`. Answering stays a person's.",
    },
    "invite": { "type": "string", "description": "settings tab orchester with ask decline: the invite's id." },
    "project": {
      "type": "string",
      "description": "The project's id (a UUID), for every screen of a project; settings tab invite: the slug of the invite's project on `server`.",
    },
  });
  match properties {
    Value::Object(properties) => properties,
    _ => Map::new(),
  }
}

/// Settings at its `tab` (automation, the default), with what that tab opens on.
pub fn route(args: &Map<String, Value>, lookup: Option<&Lookup>) -> Result<Route, ToolError> {
  let tab = string(args, "tab")?.unwrap_or("automation");
  let takes: &[&str] = match tab {
    "automation" => &["ask"],
    "accounts" => &["account", "panel"],
    "orchester" => &["server", "ask", "invite"],
    "connect" => &["server", "step"],
    "invite" => &["server", "project"],
    _ => &[],
  };
  if let Some(param) = PARAMS
    .iter()
    .find(|param| **param != "tab" && args.contains_key(**param) && !takes.contains(param))
  {
    return Err(ToolError::UnexpectedParam {
      of: TABS
        .iter()
        .find(|(name, _)| *name == tab)
        .map_or("settings", |(_, of)| of),
      param: (*param).to_owned(),
    });
  }
  match tab {
    "automation" => automation(args),
    "general" => Ok(Route::General),
    "notifications" => Ok(Route::Notifications),
    "accounts" => accounts(args, lookup).map(Route::Accounts),
    "orchester" => orchester(args, lookup).map(Route::Orchester),
    "connect" => connect(args, lookup).map(Route::Orchester),
    "invite" => landing(args, lookup).map(Route::Invite),
    other => Err(ToolError::UnknownTab {
      name: other.to_owned(),
      known: TABS.map(|(name, _)| name).to_vec(),
    }),
  }
}

/// The invite landing of the project `project` (its slug) on `server`: which local checkout to link to it.
fn landing(args: &Map<String, Value>, lookup: Option<&Lookup>) -> Result<JoinTarget, ToolError> {
  let named = string(args, "server")?.ok_or(ToolError::Missing { param: "server" })?;
  let slug = string(args, "project")?.ok_or(ToolError::Missing { param: "project" })?;
  JoinTarget::new(server(named, lookup)?, slug).ok_or(ToolError::Missing { param: "project" })
}

/// Automation, or with `ask` rotate-token asking *Rotate the token?* under the access token.
fn automation(args: &Map<String, Value>) -> Result<Route, ToolError> {
  match string(args, "ask")? {
    None => Ok(Route::Settings),
    Some("rotate-token") => Ok(Route::RotateToken),
    Some(other) => Err(ToolError::InvalidParam {
      param: "ask",
      reason: format!("`{other}` is not rotate-token, the one question Automation asks"),
    }),
  }
}

/// Accounts: the list, an account's page (`account`) and the panel it opens on (`panel`), or *Add account*.
fn accounts(args: &Map<String, Value>, lookup: Option<&Lookup>) -> Result<AccountsView, ToolError> {
  let panel = match string(args, "panel")? {
    None => AccountsPanel::Account,
    Some(name) => AccountsPanel::from_name(name).ok_or_else(|| ToolError::InvalidParam {
      param: "panel",
      reason: format!(
        "`{name}` is not one of {}",
        AccountsPanel::ALL.map(AccountsPanel::name).join(", ")
      ),
    })?,
  };
  let account = string(args, "account")?
    .map(|named| account(named, lookup))
    .transpose()?;
  if account.is_none() && !matches!(panel, AccountsPanel::Account | AccountsPanel::Add) {
    return Err(ToolError::Missing { param: "account" });
  }
  Ok(AccountsView { account, panel })
}

/// The account `named` names: its id, its name (any case), or claude-default / codex-default.
fn account(named: &str, lookup: Option<&Lookup>) -> Result<AgentAccountId, ToolError> {
  if let Ok(id) = named.parse() {
    return Ok(id);
  }
  match named {
    "claude-default" => return Ok(orchester_core::account::AccountCli::ClaudeCode.default_account()),
    "codex-default" => return Ok(orchester_core::account::AccountCli::Codex.default_account()),
    _ => {}
  }
  let lookup = lookup.ok_or(ToolError::NoSession)?;
  let accounts = lookup
    .workspace
    .agent_accounts()
    .map_err(|error| ToolError::InvalidParam {
      param: "account",
      reason: format!("the accounts can't be read: {error}"),
    })?;
  accounts
    .iter()
    .find(|account| {
      account
        .name()
        .is_some_and(|name| name.as_str().eq_ignore_ascii_case(named))
    })
    .map(orchester_core::account::AgentAccount::id)
    .ok_or_else(|| ToolError::InvalidParam {
      param: "account",
      reason: format!("no account is called `{named}`; name one by its id or name, or claude-default / codex-default"),
    })
}

/// Orchester Cloud: its column or a team server's page (`server`), with the question it asks in place (`ask`
/// sign-out, or decline with `invite`).
fn orchester(args: &Map<String, Value>, lookup: Option<&Lookup>) -> Result<OrchesterView, ToolError> {
  let server = string(args, "server")?.map(|named| server(named, lookup)).transpose()?;
  let invite = string(args, "invite")?
    .map(|id| {
      id.parse().map_err(|error: uuid::Error| ToolError::InvalidParam {
        param: "invite",
        reason: format!("`{id}` is not an invite id ({error})"),
      })
    })
    .transpose()?;
  let ask = match (string(args, "ask")?, invite) {
    (None, None) => None,
    (Some("sign-out"), None) => Some(OrchesterAsk::SignOut),
    (Some("decline"), Some(invite)) if server.is_none() => Some(OrchesterAsk::Decline(invite)),
    (Some("decline"), None) => return Err(ToolError::Missing { param: "invite" }),
    (Some("decline"), Some(_)) => {
      return Err(ToolError::InvalidParam {
        param: "ask",
        reason: "an invite is declined in the tab's column, not on a server's page".to_owned(),
      });
    }
    (None | Some("sign-out"), Some(_)) => {
      return Err(ToolError::InvalidParam {
        param: "invite",
        reason: "it goes with ask decline".to_owned(),
      });
    }
    (Some(other), _) => {
      return Err(ToolError::InvalidParam {
        param: "ask",
        reason: format!("`{other}` is not sign-out or decline"),
      });
    }
  };
  Ok(OrchesterView {
    server,
    ask,
    ..OrchesterView::default()
  })
}

/// *Connect a server*: empty, or at the address of `server` on `step`.
fn connect(args: &Map<String, Value>, lookup: Option<&Lookup>) -> Result<OrchesterView, ToolError> {
  let server = string(args, "server")?.map(|named| server(named, lookup)).transpose()?;
  let step = match string(args, "step")? {
    None => ConnectStep::Address,
    Some(name) => ConnectStep::from_name(name).ok_or_else(|| ToolError::InvalidParam {
      param: "step",
      reason: format!("`{name}` is not address, compare, join or username"),
    })?,
  };
  if step != ConnectStep::Address && server.is_none() {
    return Err(ToolError::Missing { param: "server" });
  }
  Ok(OrchesterView {
    server,
    panel: OrchesterPanel::Connect,
    step,
    ask: None,
  })
}

/// The team server `named` names: its id, or its name or address's host (any case), among this account's servers.
fn server(named: &str, lookup: Option<&Lookup>) -> Result<ServerId, ToolError> {
  if let Ok(id) = named.parse() {
    return Ok(id);
  }
  let lookup = lookup.ok_or(ToolError::NoSession)?;
  let entries = lookup.servers.entries().map_err(|error| ToolError::InvalidParam {
    param: "server",
    reason: format!("the servers can't be read: {error}"),
  })?;
  entries
    .iter()
    .find(|entry| {
      entry.stored.name.eq_ignore_ascii_case(named) || entry.address.authority().eq_ignore_ascii_case(named)
    })
    .map(|entry| entry.stored.id)
    .ok_or_else(|| ToolError::InvalidParam {
      param: "server",
      reason: format!("no team server is called `{named}`; name one by its id, name or host"),
    })
}
