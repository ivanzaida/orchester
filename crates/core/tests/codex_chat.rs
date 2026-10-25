//! The Orchestrator chat's turn on Codex (`CodexChat`): App Server read-only in the project folder, with the chat's
//! domain tools as its only MCP server, their token only in the process environment, and every tool of Codex's own
//! that reads, runs or fetches anything turned off, the same on Windows and macOS; and what its system prompt says of
//! reading the project. `cargo test -p orchester-core --test codex_chat codex_chat`.

use std::{
  ffi::OsString,
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_core::{
  command::Command,
  connection::{
    invocation::{
      CODEX_CHAT_CONFINEMENT, CODEX_ISOLATION, CODEX_SUB_AGENTS_OFF, CodexChat, CodexMcp, HttpAuthorization, McpLaunch,
      Platform,
    },
    mcp::{McpServerName, McpUrl},
  },
  id::ProjectId,
  orchestrator::instructions::{ProjectReads, SERVER, system_prompt},
  resource::CommandEnvironment,
  secret::SecretValue,
};

const TOKEN: &str = "s3cr3t-chat-token";

fn tools() -> CodexMcp {
  let launch = McpLaunch::http(
    McpServerName::new(SERVER).expect("valid name"),
    McpUrl::new("http://127.0.0.1:9/mcp").expect("valid URL"),
    HttpAuthorization::Bearer(Arc::new(SecretValue::new(TOKEN.to_owned()).expect("not blank"))),
  );
  let (mcp, left_out) = CodexMcp::new(&[launch]);
  assert!(left_out.is_empty(), "{left_out:?}");
  mcp
}

fn chat<'a>(folder: &'a Path, mcp: &'a CodexMcp, platform: Platform) -> CodexChat<'a> {
  CodexChat {
    executable: Path::new("/bin/codex"),
    folder,
    platform,
    mcp,
  }
}

/// The `-c` overrides of `args`, in order.
fn overrides(args: &[String]) -> Vec<&str> {
  args
    .windows(2)
    .filter(|pair| pair[0] == "-c")
    .map(|pair| pair[1].as_str())
    .collect()
}

#[test]
fn codex_chat_has_no_tool_of_its_own_that_reads_or_runs_anything_on_either_platform() {
  let mcp = tools();
  for (platform, folder) in [
    (Platform::Windows, PathBuf::from(r"H:\Projects\Kontur")),
    (Platform::MacOs, PathBuf::from("/Users/me/Kontur")),
  ] {
    let args = chat(&folder, &mcp, platform).args().expect("a Unicode folder");
    assert_eq!(args[..2], ["app-server", "--stdio"], "{args:?}");
    let overrides = overrides(&args);
    let confinement = &overrides[overrides.len() - CODEX_CHAT_CONFINEMENT.len()..];
    assert_eq!(
      confinement, CODEX_CHAT_CONFINEMENT,
      "the confinement comes last, so nothing before it turns a tool back on"
    );
    for off in [
      "features.shell_tool=false",
      "features.unified_exec=false",
      "features.view_image=false",
      "features.hooks=false",
      "web_search=\"disabled\"",
    ] {
      assert!(confinement.contains(&off), "{off} on {platform:?}: {args:?}");
    }
    for setting in CODEX_ISOLATION.into_iter().chain(CODEX_SUB_AGENTS_OFF) {
      assert!(overrides.contains(&setting), "{setting} on {platform:?}: {args:?}");
    }
    assert!(
      overrides.contains(&"sandbox_workspace_write.writable_roots=[]"),
      "nothing is writable: {args:?}"
    );
    assert!(
      overrides.contains(&"project_doc_max_bytes=0"),
      "the repository's AGENTS.md is left out: {args:?}"
    );
    assert!(
      !overrides
        .iter()
        .any(|setting| setting.starts_with("sandbox_workspace_write.network_access")),
      "no network setting: the read-only sandbox has none: {args:?}"
    );
    assert!(
      overrides
        .iter()
        .any(|setting| setting.starts_with("projects={") && setting.ends_with("={trust_level=\"untrusted\"}}")),
      "the folder's own .codex never loads: {args:?}"
    );
    assert!(
      overrides.contains(&r#"mcp_servers.orchester.url="http://127.0.0.1:9/mcp""#),
      "the chat's tools are its MCP server: {args:?}"
    );
    let servers: Vec<&&str> = overrides
      .iter()
      .filter(|setting| setting.starts_with("mcp_servers.") && setting.contains(".url="))
      .collect();
    assert_eq!(servers.len(), 1, "no other server: {args:?}");
  }
}

#[test]
fn codex_chat_uses_the_elevated_sandbox_on_windows_only() {
  let mcp = tools();
  let windows = chat(Path::new(r"H:\Projects\Kontur"), &mcp, Platform::Windows)
    .args()
    .expect("a Unicode folder");
  assert!(overrides(&windows).contains(&"windows.sandbox=elevated"), "{windows:?}");
  let macos = chat(Path::new("/Users/me/Kontur"), &mcp, Platform::MacOs)
    .args()
    .expect("a Unicode folder");
  assert!(
    !overrides(&macos).iter().any(|setting| setting.starts_with("windows.")),
    "{macos:?}"
  );
}

#[test]
fn codex_chat_keeps_the_tools_token_in_the_environment_only_and_starts_in_the_project_folder() {
  let mcp = tools();
  let folder = Path::new("/Users/me/Kontur");
  let environment = [
    (OsString::from("PATH"), OsString::from("/usr/bin")),
    (OsString::from("GH_TOKEN"), OsString::from("gho_secret")),
  ];
  let command: Command = chat(folder, &mcp, Platform::MacOs)
    .command(environment, &CommandEnvironment::default())
    .expect("a Unicode folder");
  assert_eq!(command.current_dir(), Some(folder));
  assert!(
    command.interactive_stdin(),
    "the runner speaks App Server's protocol on it"
  );
  assert_eq!(
    command.args(),
    chat(folder, &mcp, Platform::MacOs).args().expect("a Unicode folder"),
    "the command runs exactly the confined arguments"
  );
  assert!(
    !command.args().iter().any(|arg| arg.contains(TOKEN)),
    "the token is never on the command line"
  );
  assert!(!format!("{command:?}").contains(TOKEN));
  assert!(
    command
      .secret_env()
      .iter()
      .any(|variable| variable.name() == "ORCHESTER_MCP_TOKEN_ORCHESTER" && variable.expose() == TOKEN),
    "the token reaches the server through the environment"
  );
  assert!(
    !command.env().iter().any(|(name, _)| name == "GH_TOKEN"),
    "the environment is cleaned as for a step"
  );
}

#[test]
fn codex_chat_prompt_reads_the_project_through_the_git_tools() {
  let project = ProjectId::generate();
  let git = system_prompt("Kontur", project, ProjectReads::GitTools);
  assert!(
    git.contains("You have no tool that reads files or runs commands"),
    "{git}"
  );
  assert!(git.contains("git_show with a file"), "{git}");
  assert!(!git.contains("You may read and search it"), "{git}");
  let files = system_prompt("Kontur", project, ProjectReads::FileTools);
  assert!(files.contains("You may read and search it"), "{files}");
}
