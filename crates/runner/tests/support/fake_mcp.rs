//! What the fake CLIs report about the MCP servers they were given, on standard error, so tests can
//! check that the configuration and the secrets reached the CLI and how:
//!
//! - Codex: `mcp-arg: <override>` for each `-c mcp_servers.…` override, and `mcp-env: <VAR>=<value>` for each variable
//!   an `env_vars` or `bearer_token_env_var` override hands over, read from the fake's own environment;
//! - Claude Code: `mcp-config-file: <path>` and `mcp-config: <contents>` of the `--mcp-config` file.
//!
//! A server named `broken` is reported as not started, the way each CLI reports it.

#![allow(dead_code, reason = "shared by both fake CLIs, each of which uses one half")]

use std::fs;

const BROKEN: &str = "broken";

/// Reports the servers of Codex arguments `args`; `true` when one of them is `broken`.
pub fn report_codex(args: &[String]) -> bool {
  let mut broken = false;
  for arg in args.iter().filter(|arg| arg.starts_with("mcp_servers.")) {
    eprintln!("mcp-arg: {arg}");
    broken |= arg.starts_with(&format!("mcp_servers.{BROKEN}."));
    let Some((_, names)) = arg
      .split_once(".env_vars=")
      .or_else(|| arg.split_once(".bearer_token_env_var="))
    else {
      continue;
    };
    for name in names
      .trim_matches(['[', ']'])
      .split(',')
      .map(|name| name.trim_matches('"'))
    {
      let value = std::env::var(name).unwrap_or_else(|_| "<unset>".to_owned());
      eprintln!("mcp-env: {name}={value}");
    }
  }
  broken
}

/// Reports the `--mcp-config` file of Claude Code arguments `args` with the project's servers (not
/// the permission prompt's, whose token stays out of the report); `true` when it configures `broken`.
pub fn report_claude(args: &[String]) -> bool {
  let Some(path) = args
    .iter()
    .position(|arg| arg == "--mcp-config")
    .and_then(|index| args.get(index + 1))
    .filter(|path| !fs::read_to_string(path).is_ok_and(|contents| contents.contains(r#""Orchester":"#)))
  else {
    return false;
  };
  eprintln!("mcp-config-file: {path}");
  match fs::read_to_string(path) {
    Ok(contents) => {
      eprintln!("mcp-config: {contents}");
      contents.contains(&format!("\"{BROKEN}\":"))
    }
    Err(error) => {
      eprintln!("mcp-config: cannot read it: {error}");
      false
    }
  }
}
