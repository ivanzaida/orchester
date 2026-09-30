//! Repository agent files (ORC-15): how each CLI is started under the project's setting, on new
//! and resumed sessions, and the skills folder an agent is given.

use std::path::{Path, PathBuf};

use orchester_core::{
  agent_files::{AgentFilesMechanism, RepositoryAgentFiles},
  connection::{
    AgentProvider,
    invocation::{ClaudeInvocation, ClaudeSession, CodexAgentFiles, CodexInvocation, Platform},
  },
  permission::{AgentPermissions, AllowedCommands, CommandPermission, NetworkAccess, RepositoryAccess},
  resource::RepositoryAccess as StepAccess,
};

fn codex<'a>(resume: Option<&'a str>, agent_files: CodexAgentFiles<'a>) -> CodexInvocation<'a> {
  CodexInvocation {
    executable: Path::new("/bin/codex"),
    model: "gpt-5.5",
    reasoning_effort: None,
    access: StepAccess::ReadOnly,
    network: NetworkAccess::Off,
    writable_roots: &[],
    worktree: Path::new("/repo/.orchester/worktrees/run"),
    platform: Platform::MacOs,
    resume,
    output_schema: None,
    mcp: None,
    agent_files,
  }
}

fn strings(args: Vec<std::ffi::OsString>) -> Vec<String> {
  args
    .into_iter()
    .map(|arg| arg.into_string().expect("unicode"))
    .collect()
}

/// The arguments between the sandbox and what ends the command line (session or `-C`).
fn agent_file_args(args: &[String]) -> Vec<String> {
  args
    .windows(2)
    .filter(|pair| pair[0] == "-c" && (pair[1].starts_with("project_doc") || pair[1].starts_with("skills.")))
    .map(|pair| pair[1].clone())
    .collect()
}

#[test]
fn codex_loads_the_repository_agent_files_while_the_setting_is_on() {
  for resume in [None, Some("session-1")] {
    let args = strings(codex(resume, CodexAgentFiles::Loaded).args());
    assert!(agent_file_args(&args).is_empty(), "{args:?}");
  }
}

#[test]
fn codex_leaves_agents_md_and_each_repository_skill_out_while_the_setting_is_off() {
  let skills = [
    PathBuf::from("/repo/.orchester/worktrees/run/.agents/skills/deploy/SKILL.md"),
    PathBuf::from("/repo/.orchester/worktrees/run/.agents/skills/group/\"quoted\"/SKILL.md"),
  ];
  let expected = [
    "project_doc_max_bytes=0".to_owned(),
    "skills.config=[{path=\"/repo/.orchester/worktrees/run/.agents/skills/deploy/SKILL.md\",enabled=false},\
     {path=\"/repo/.orchester/worktrees/run/.agents/skills/group/\\\"quoted\\\"/SKILL.md\",enabled=false}]"
      .to_owned(),
  ];
  for resume in [None, Some("session-1")] {
    let args = strings(codex(resume, CodexAgentFiles::LeftOut { skills: &skills }).args());
    assert_eq!(agent_file_args(&args), expected, "{args:?}");
    // The prompt still ends the command line: the session (resume) or `-C` come right before it.
    assert_eq!(args.last().map(String::as_str), Some("-"));
    if let Some(session) = resume {
      assert_eq!(args[args.len() - 2], session);
    }
  }
}

#[test]
fn codex_without_repository_skills_only_leaves_agents_md_out() {
  let args = strings(codex(None, CodexAgentFiles::LeftOut { skills: &[] }).args());
  assert_eq!(agent_file_args(&args), ["project_doc_max_bytes=0"]);
}

#[cfg(unix)]
#[test]
fn codex_refuses_a_repository_skill_path_that_is_not_unicode() {
  use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

  use orchester_core::{connection::invocation::InvocationError, resource::CommandEnvironment};
  let skills = [PathBuf::from(OsStr::from_bytes(b"/repo/.agents/skills/\xff/SKILL.md"))];
  let error = codex(None, CodexAgentFiles::LeftOut { skills: &skills })
    .command(String::new(), Vec::new(), &CommandEnvironment::default())
    .expect_err("not unicode");
  assert!(matches!(error, InvocationError::NotUnicode(_)));
}

fn claude<'a>(
  permissions: &'a orchester_core::permission::capability::EnforcedPermissions,
  session: ClaudeSession<'a>,
  skills: Option<&'a Path>,
) -> ClaudeInvocation<'a> {
  ClaudeInvocation {
    executable: Path::new("/bin/claude"),
    model: "haiku",
    effort: None,
    permissions,
    writable_roots: &[],
    worktree: Path::new("/repo/.orchester/worktrees/run"),
    session,
    mcp: None,
    skills,
    approvals: None,
  }
}

#[test]
fn claude_code_never_loads_project_settings_and_reads_the_skills_folder_it_is_given() {
  let permissions = AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    CommandPermission::Allowed(AllowedCommands::default()),
    NetworkAccess::Off,
  )
  .enforced_on(AgentProvider::ClaudeCodeCli, Platform::current())
  .expect("enforceable");
  let skills = Path::new("/repo/.orchester/skills/e1");
  for session in [ClaudeSession::New(uuid::Uuid::nil()), ClaudeSession::Resume("s-1")] {
    let without = strings(claude(&permissions, session, None).args());
    let setting = without
      .iter()
      .position(|arg| arg == "--setting-sources")
      .expect("present");
    assert_eq!(without[setting + 1], "");
    assert!(!without.contains(&"--add-dir".to_owned()));
    let with = strings(claude(&permissions, session, Some(skills)).args());
    let add = with
      .iter()
      .position(|arg| arg == "--add-dir")
      .expect("the skills folder");
    assert_eq!(with[add + 1], "/repo/.orchester/skills/e1");
  }
}

#[test]
fn each_cli_says_how_it_treats_the_repository_agent_files() {
  use AgentFilesMechanism as M;
  let cases = [
    (AgentProvider::CodexCli, RepositoryAgentFiles::On, M::CodexLoads),
    (AgentProvider::CodexCli, RepositoryAgentFiles::Off, M::CodexLeftOut),
    (
      AgentProvider::ClaudeCodeCli,
      RepositoryAgentFiles::On,
      M::ClaudeNoProjectSettings,
    ),
    (
      AgentProvider::ClaudeCodeCli,
      RepositoryAgentFiles::Off,
      M::ClaudeNoProjectSettings,
    ),
  ];
  for (provider, setting, mechanism) in cases {
    assert_eq!(setting.mechanism(provider), mechanism);
  }
  assert_eq!(RepositoryAgentFiles::default(), RepositoryAgentFiles::On);
}
