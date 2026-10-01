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

fn codex(agent_files: CodexAgentFiles<'_>) -> CodexInvocation<'_> {
  CodexInvocation {
    executable: Path::new("/bin/codex"),
    repository: Path::new("/repo"),
    access: StepAccess::ReadOnly,
    network: NetworkAccess::Off,
    writable_roots: &[],
    worktree: Path::new("/repo/.orchester/worktrees/run"),
    platform: Platform::MacOs,
    mcp: None,
    agent_files,
  }
}

/// The agent file overrides among Codex's arguments. App Server starts new and resumed sessions with
/// the same arguments, so both leave the files out alike.
fn agent_file_args(invocation: &CodexInvocation<'_>) -> Vec<String> {
  let args = invocation.args().expect("unicode paths");
  args
    .windows(2)
    .filter(|pair| pair[0] == "-c" && (pair[1].starts_with("project_doc") || pair[1].starts_with("skills.")))
    .map(|pair| pair[1].clone())
    .collect()
}

#[test]
fn codex_loads_the_repository_agent_files_while_the_setting_is_on() {
  assert!(agent_file_args(&codex(CodexAgentFiles::Loaded)).is_empty());
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
  assert_eq!(
    agent_file_args(&codex(CodexAgentFiles::LeftOut { skills: &skills })),
    expected
  );
}

#[test]
fn codex_without_repository_skills_only_leaves_agents_md_out() {
  assert_eq!(
    agent_file_args(&codex(CodexAgentFiles::LeftOut { skills: &[] })),
    ["project_doc_max_bytes=0"]
  );
}

#[cfg(unix)]
#[test]
fn codex_refuses_a_repository_skill_path_that_is_not_unicode() {
  use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

  use orchester_core::{connection::invocation::InvocationError, resource::CommandEnvironment};
  let skills = [PathBuf::from(OsStr::from_bytes(b"/repo/.agents/skills/\xff/SKILL.md"))];
  let error = codex(CodexAgentFiles::LeftOut { skills: &skills })
    .command(Vec::new(), &CommandEnvironment::default())
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
    code_index: None,
    approvals: None,
  }
}

fn strings(invocation: &ClaudeInvocation<'_>) -> Vec<String> {
  invocation
    .args()
    .into_iter()
    .map(|arg| arg.into_string().expect("unicode"))
    .collect()
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
    let without = strings(&claude(&permissions, session, None));
    let setting = without
      .iter()
      .position(|arg| arg == "--setting-sources")
      .expect("present");
    assert_eq!(without[setting + 1], "");
    assert!(!without.contains(&"--add-dir".to_owned()));
    let with = strings(&claude(&permissions, session, Some(skills)));
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
