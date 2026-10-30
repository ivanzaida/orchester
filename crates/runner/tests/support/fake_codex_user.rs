//! The user's own MCP servers in `orchester-fake-codex`'s configuration: `user-global` always, and each name a line of
//! `fake-user-servers` in its working folder lists (the worktree, where a test commits the file), so that a test can
//! give the user a server named like one the step chooses. Codex gets such a chosen server by another name
//! (`orchester-<name>`); the fake's model calls a server by the name its arguments give it ([`argument_name`]).

use std::{collections::BTreeSet, fs};

use serde_json::{Value, json};

/// The server every user of the fake has.
const ALWAYS: &str = "user-global";

/// The user's own servers.
pub fn servers() -> Vec<String> {
  let listed = fs::read_to_string("fake-user-servers").unwrap_or_default();
  std::iter::once(ALWAYS.to_owned())
    .chain(
      listed
        .lines()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned),
    )
    .collect()
}

/// The user's servers a thread's configuration turns off, by the dotted key or by a table.
pub fn turned_off(servers: &[String], config: &Value) -> BTreeSet<String> {
  servers
    .iter()
    .filter(|name| {
      config["mcp_servers"][name.as_str()]["enabled"] == json!(false)
        || config[format!("mcp_servers.{name}.enabled")] == json!(false)
    })
    .cloned()
    .collect()
}

/// A server of `servers` (a thread's `mcp_servers` configuration) Codex 0.159.2 refuses: one with
/// neither a `command` nor a `url` that the user's configuration (`user`) does not define.
pub fn without_transport<'a>(servers: &'a Value, user: &[String]) -> Option<&'a str> {
  servers
    .as_object()
    .into_iter()
    .flatten()
    .find(|(name, table)| !user.contains(name) && table.get("command").is_none() && table.get("url").is_none())
    .map(|(name, _)| name.as_str())
}

/// The name Codex's arguments give the project's server `server`: its own, or `orchester-<server>` when only that
/// one is given.
pub fn argument_name(args: &[String], server: &str) -> String {
  let renamed = format!("orchester-{server}");
  let given = |name: &str| args.iter().any(|arg| arg.starts_with(&format!("mcp_servers.{name}.")));
  if !given(server) && given(&renamed) {
    renamed
  } else {
    server.to_owned()
  }
}
