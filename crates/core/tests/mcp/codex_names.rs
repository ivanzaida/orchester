//! A server the step chooses that Codex's own configuration has a server of the same name with: the names Codex's
//! configuration has, read before the step's Codex starts, and the other name such a server gets in Codex, which its
//! settings, its tools' approvals and what the agent is told go by.

use std::{cell::RefCell, collections::BTreeSet, path::Path};

use orchester_core::{
  command::{Command, CommandError, LineExchange},
  connection::{
    app_server_config::{ConfigReadError, names_read_command, read_configured_names},
    invocation::{CodexMcp, HttpAuthorization, LaunchValue, McpLaunch},
    mcp::{ExemptTools, McpServerName, McpUrl},
  },
  resource::VariableName,
};
use serde_json::{Value, json};

fn name(value: &str) -> McpServerName {
  McpServerName::new(value).expect("valid name")
}

/// `pencil`, a program with a variable whose `get_screenshot` runs without asking, and `docs` over HTTP.
fn launches() -> Vec<McpLaunch> {
  let variable = VariableName::new("PENCIL_MODE").expect("valid name");
  let pencil = McpLaunch::stdio(
    name("pencil"),
    ("pencil-mcp".to_owned(), vec!["--stdio".to_owned()]),
    None,
    vec![(variable, LaunchValue::Text("project".to_owned()))],
  )
  .with_exempt_tools(ExemptTools::parse("get_screenshot").expect("valid tools"));
  let docs = McpLaunch::http(
    name("docs"),
    McpUrl::new("http://localhost:7/mcp").expect("valid URL"),
    HttpAuthorization::None,
  );
  vec![pencil, docs]
}

fn taken(names: &[&str]) -> BTreeSet<String> {
  names.iter().map(|name| (*name).to_owned()).collect()
}

#[test]
fn codex_names_a_server_the_users_configuration_has_apart() {
  let (mcp, _) = CodexMcp::new(&launches());
  let mcp = mcp.named_apart(&taken(&["pencil", "user-notes"]));
  assert_eq!(mcp.codex_name(&name("pencil")), "orchester-pencil");
  assert_eq!(mcp.codex_name(&name("docs")), "docs", "a name nobody else has stays");
  let args = mcp.args();
  for expected in [
    r#"mcp_servers.orchester-pencil.command="pencil-mcp""#,
    r#"mcp_servers.orchester-pencil.args=["--stdio"]"#,
    r#"mcp_servers.orchester-pencil.env.PENCIL_MODE="project""#,
    r#"mcp_servers.orchester-pencil.tools={"get_screenshot"={approval_mode="approve"}}"#,
    r#"mcp_servers.docs.url="http://localhost:7/mcp""#,
  ] {
    assert!(args.iter().any(|arg| arg == expected), "{expected} not in {args:?}");
  }
  assert!(
    !args.iter().any(|arg| arg.starts_with("mcp_servers.pencil.")),
    "{args:?}"
  );
  // The exemption goes by the project's name, as the step's activity and approvals do.
  assert!(mcp.exemptions().exempts("pencil", "get_screenshot"));
  let prompt = mcp.renamed_prompt().expect("a renamed server");
  assert!(
    prompt.contains("The MCP server `pencil` of this step is called `orchester-pencil` in this Codex session"),
    "{prompt}"
  );
}

#[test]
fn codex_names_number_on_while_the_other_name_is_taken_too() {
  let (mcp, _) = CodexMcp::new(&launches());
  let mcp = mcp.named_apart(&taken(&["pencil", "docs", "orchester-pencil", "orchester-docs"]));
  assert_eq!(mcp.codex_name(&name("pencil")), "orchester-pencil-2");
  assert_eq!(mcp.codex_name(&name("docs")), "orchester-docs-2");
  let (untouched, _) = CodexMcp::new(&launches());
  let untouched = untouched.named_apart(&taken(&["user-notes"]));
  assert!(untouched.renamed().is_empty());
  assert_eq!(untouched.renamed_prompt(), None);
}

/// App Server answering with `lines`, until the caller has its answer; the command it was asked with is kept.
struct Answering<'a> {
  lines: &'a [Value],
  asked: RefCell<Option<Command>>,
}

impl LineExchange for Answering<'_> {
  fn exchange(&self, command: &Command, done: &mut dyn FnMut(&str) -> bool) -> Result<Vec<String>, CommandError> {
    self.asked.replace(Some(command.clone()));
    let mut read = Vec::new();
    for line in self.lines {
      let line = line.to_string();
      let answered = done(&line);
      read.push(line);
      if answered {
        break;
      }
    }
    Ok(read)
  }
}

fn read(lines: &[Value]) -> (Result<BTreeSet<String>, ConfigReadError>, Command) {
  let runner = Answering {
    lines,
    asked: RefCell::new(None),
  };
  let codex = Command::new(Path::new("codex"), ["app-server", "--stdio"]);
  let command = names_read_command(codex, Path::new("/repo/worktree"));
  let names = read_configured_names(&runner, &command);
  (names, runner.asked.take().expect("asked"))
}

#[test]
fn codex_names_of_the_configuration_are_read_from_config_read() {
  let answer = json!({"id": 6, "result": {
    "config": {"mcp_servers": {"pencil": {"enabled": true, "command": "user-pencil"}, "user-notes": {"url": "x"}}},
    "layers": []
  }});
  let (names, asked) = read(&[json!({"id": 1, "result": {}}), answer]);
  assert_eq!(names, Ok(taken(&["pencil", "user-notes"])));
  let input = asked.stdin().expect("requests");
  let requests: Vec<Value> = input
    .lines()
    .map(|line| serde_json::from_str(line).expect("JSON"))
    .collect();
  assert_eq!(requests[2]["method"], "config/read");
  assert_eq!(requests[2]["params"]["cwd"], "/repo/worktree");
  assert!(asked.interactive_stdin(), "the input stays open until the answer is in");
  let (none, _) = read(&[json!({"id": 6, "result": {"config": {}}})]);
  assert_eq!(none, Ok(BTreeSet::new()));
}

#[test]
fn codex_names_read_refused_or_unanswered_says_so() {
  let refused = json!({"id": 6, "error": {"code": -32603, "message": "invalid configuration:\nurl is not supported"}});
  assert_eq!(
    read(&[refused]).0,
    Err(ConfigReadError::Refused(
      "invalid configuration: url is not supported".to_owned()
    ))
  );
  assert_eq!(
    read(&[json!({"id": 1, "result": {}})]).0,
    Err(ConfigReadError::NoAnswer)
  );
  assert_eq!(
    read(&[json!({"id": 6, "result": {}})]).0,
    Err(ConfigReadError::MissingConfiguration)
  );
}
