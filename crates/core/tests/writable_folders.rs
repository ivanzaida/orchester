//! Extra writable folders: which folders an agent may also write to, how they are stored and
//! narrowed, and the exact arguments that give them to Codex and Claude Code.

use std::{
  fs,
  path::{Path, PathBuf},
};

use orchester_core::{
  connection::{
    AgentProvider,
    invocation::{ClaudeInvocation, ClaudeSession, CodexAgentFiles, CodexInvocation, Platform},
  },
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, FolderBounds, NetworkAccess, RepositoryAccess,
    StepPermissions, WritableFolder, WritableFolderError, WritableFolders, WritableFoldersError, capability::Mechanism,
  },
  resource::{AgentProfile, ResourceBody},
};
use tempfile::TempDir;
use uuid::Uuid;

/// A temporary layout: `repo` with a checkout folder `src` and `.orchester/worktrees`, the home
/// folder `users/me` and an existing folder `outside`.
struct Layout {
  // Removed with the layout.
  _root: TempDir,
  bounds: FolderBounds,
}

impl Layout {
  fn new() -> Self {
    let root = tempfile::tempdir().expect("temporary folder");
    for folder in ["repo/src", "repo/.orchester/worktrees/run", "users/me", "outside"] {
      fs::create_dir_all(root.path().join(folder)).expect("folder");
    }
    fs::write(root.path().join("outside/file.txt"), "x").expect("file");
    let bounds = FolderBounds::new(&root.path().join("repo"), Some(&root.path().join("users/me"))).expect("bounds");
    Self { _root: root, bounds }
  }

  /// The canonical path of `relative` under the layout.
  fn path(&self, relative: &str) -> PathBuf {
    self
      .bounds
      .repository()
      .parent()
      .expect("repo has a parent")
      .join(relative)
  }

  fn check(&self, entry: &str) -> Result<WritableFolder, WritableFolderError> {
    WritableFolder::new(entry, &self.bounds)
  }
}

fn text(path: &Path) -> String {
  path.to_str().expect("unicode").to_owned()
}

#[test]
fn writable_folder_relative_entries_resolve_against_the_repository_root() {
  let layout = Layout::new();
  let target = layout.check(".orchester/target").expect("creatable under .orchester");
  assert_eq!(target.as_str(), ".orchester/target");
  let resolved = target.resolve(&layout.bounds).expect("resolves");
  assert_eq!(resolved.path(), layout.path("repo/.orchester/target"));
  assert!(resolved.is_missing());
  let outside = layout
    .check("../outside")
    .expect("an existing folder outside the repository");
  let resolved = outside.resolve(&layout.bounds).expect("resolves");
  assert_eq!(resolved.path(), layout.path("outside"));
  assert!(!resolved.is_missing());
  let absolute = layout
    .check(&format!("  {}  ", text(&layout.path("outside"))))
    .expect("absolute");
  assert_eq!(absolute.as_str(), text(&layout.path("outside")));
  assert_eq!(
    absolute.lexical_path(layout.bounds.repository()),
    layout.path("outside")
  );
}

#[test]
fn writable_folder_refuses_the_repository_its_checkout_and_orchester_folders() {
  let layout = Layout::new();
  let repo = layout.path("repo");
  assert_eq!(layout.check("."), Err(WritableFolderError::RepositoryRoot));
  assert_eq!(layout.check(&text(&repo)), Err(WritableFolderError::RepositoryRoot));
  assert_eq!(
    layout.check(".."),
    Err(WritableFolderError::ContainsRepository(layout.path("")))
  );
  assert_eq!(
    layout.check("src"),
    Err(WritableFolderError::InCheckout(repo.join("src")))
  );
  assert_eq!(
    layout.check("target"),
    Err(WritableFolderError::InCheckout(repo.join("target")))
  );
  assert_eq!(
    layout.check(".orchester/../src"),
    Err(WritableFolderError::InCheckout(repo.join("src")))
  );
  for reserved in [
    ".orchester",
    ".orchester/worktrees",
    ".orchester/worktrees/run",
    ".orchester/logs",
  ] {
    assert!(
      matches!(layout.check(reserved), Err(WritableFolderError::OrchesterFolder(_))),
      "{reserved} is Orchester's own"
    );
  }
}

#[test]
fn writable_folder_refuses_the_home_folder_and_what_contains_it() {
  let layout = Layout::new();
  assert_eq!(
    layout.check(&text(&layout.path("users/me"))),
    Err(WritableFolderError::Home)
  );
  assert_eq!(
    layout.check(&text(&layout.path("users"))),
    Err(WritableFolderError::ContainsHome(layout.path("users")))
  );
  let root = layout.path("").ancestors().last().expect("a root").to_path_buf();
  assert_eq!(layout.check(&text(&root)), Err(WritableFolderError::FilesystemRoot));
  layout
    .check(&text(&layout.path("users/me/.cargo-shared")))
    .expect_err("missing outside .orchester");
  fs::create_dir_all(layout.path("users/me/cache")).expect("folder");
  layout
    .check(&text(&layout.path("users/me/cache")))
    .expect("a folder inside home is fine");
}

#[test]
fn writable_folder_must_exist_unless_it_is_under_orchester() {
  let layout = Layout::new();
  assert_eq!(
    layout.check("../missing"),
    Err(WritableFolderError::Missing(layout.path("missing")))
  );
  assert_eq!(
    layout.check("../outside/file.txt"),
    Err(WritableFolderError::NotAFolder(layout.path("outside/file.txt")))
  );
  assert_eq!(layout.check("   "), Err(WritableFolderError::Blank));
  assert_eq!(layout.check("a\tb"), Err(WritableFolderError::ControlCharacter));
  // A stored folder is checked for its spelling only; every use resolves it again.
  let stored: WritableFolder = serde_json::from_str("\"../missing\"").expect("spelling is valid");
  assert!(matches!(
    stored.resolve(&layout.bounds),
    Err(WritableFolderError::Missing(_))
  ));
  assert!(serde_json::from_str::<WritableFolder>("\"\"").is_err());
}

#[test]
fn writable_folders_refuse_repeats_and_resolve_each_folder_once() {
  let layout = Layout::new();
  let target = layout.check(".orchester/target").expect("valid");
  let absolute = layout
    .check(&text(&layout.path("repo/.orchester/target")))
    .expect("valid");
  assert_eq!(
    WritableFolders::new(vec![target.clone(), target.clone()]),
    Err(WritableFoldersError::Duplicate(target.clone()))
  );
  let folders = WritableFolders::new(vec![target, absolute]).expect("two spellings");
  let resolved = folders.resolve(&layout.bounds).expect("both resolve");
  assert_eq!(resolved.len(), 1);
  let bad = WritableFolders::new(vec![serde_json::from_str("\"src\"").expect("spelled")]).expect("one");
  let (folder, error) = bad.resolve(&layout.bounds).expect_err("inside the checkout");
  assert_eq!(folder.as_str(), "src");
  assert!(matches!(error, WritableFolderError::InCheckout(_)));
}

fn folders(entries: &[&str]) -> WritableFolders {
  WritableFolders::new(
    entries
      .iter()
      .map(|entry| serde_json::from_str(&format!("{entry:?}")).expect("spelled"))
      .collect(),
  )
  .expect("unique")
}

fn edit_permissions(entries: &[&str]) -> AgentPermissions {
  let commands = AllowedCommands::new(vec![CommandPrefix::new("cargo").expect("prefix")]).expect("list");
  AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::Allowed(commands),
    NetworkAccess::On,
  )
  .with_writable_folders(folders(entries))
}

#[test]
fn writable_folders_apply_only_while_agents_edit_files() {
  let permissions = edit_permissions(&[".orchester/target"]);
  assert_eq!(
    permissions.granted_folders().map(WritableFolders::entries),
    Some(folders(&[".orchester/target"]).entries())
  );
  let narrowed = permissions.narrowed(RepositoryAccess::ReadOnly, &StepPermissions::default());
  assert_eq!(narrowed.granted_folders(), None);
  assert_eq!(narrowed.writable_folders(), permissions.writable_folders());
  let kept = permissions.narrowed(RepositoryAccess::Write, &StepPermissions::default());
  assert_eq!(kept.granted_folders(), permissions.granted_folders());
  let any = AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::AnyInWorktree,
    NetworkAccess::On,
  )
  .with_writable_folders(folders(&[".orchester/target"]));
  let codex = any
    .enforced_on(AgentProvider::CodexCli, Platform::Windows)
    .expect("enforceable");
  assert_eq!(codex.writable_folders(), Some(Mechanism::CodexWritableRoots));
  let claude = permissions
    .enforced_on(AgentProvider::ClaudeCodeCli, Platform::MacOs)
    .expect("enforceable");
  assert_eq!(claude.writable_folders(), Some(Mechanism::ClaudeAddDir));
  let read_only = narrowed
    .enforced_on(AgentProvider::ClaudeCodeCli, Platform::MacOs)
    .expect("enforceable");
  assert_eq!(read_only.writable_folders(), None);
}

#[test]
fn writable_folders_are_stored_only_when_there_are_some() {
  let none = AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::AnyInWorktree,
    NetworkAccess::Off,
  );
  let json = serde_json::to_string(&none).expect("serializes");
  assert!(
    !json.contains("writable_folders"),
    "the shape before folders existed: {json}"
  );
  assert_eq!(serde_json::from_str::<AgentPermissions>(&json).expect("reads"), none);
  let some = none.with_writable_folders(folders(&[".orchester/target"]));
  let json = serde_json::to_string(&some).expect("serializes");
  assert!(json.contains(r#""writable_folders":[".orchester/target"]"#), "{json}");
  assert_eq!(serde_json::from_str::<AgentPermissions>(&json).expect("reads"), some);
  assert_eq!(AgentProfile::SCHEMA, 4);
  let body = serde_json::json!({"any": "schema 3 body"});
  assert_eq!(AgentProfile::upgrade(3, body.clone()), Some(body));
}

const ROOTS: [&str; 2] = [r"C:\repo\.orchester\target", "/shared/it's \"here\""];

fn roots() -> Vec<PathBuf> {
  ROOTS.iter().map(PathBuf::from).collect()
}

fn codex_args(access: RepositoryAccess, resume: Option<&str>, roots: &[PathBuf]) -> Vec<String> {
  CodexInvocation {
    executable: Path::new("/bin/codex"),
    model: "gpt-5.5",
    reasoning_effort: None,
    access,
    network: NetworkAccess::Off,
    writable_roots: roots,
    worktree: Path::new("/repo/.orchester/worktrees/run"),
    platform: Platform::Windows,
    resume,
    output_schema: None,
    mcp: None,
    agent_files: CodexAgentFiles::Loaded,
  }
  .args()
  .into_iter()
  .map(|arg| arg.into_string().expect("unicode"))
  .collect()
}

const CODEX_ROOTS: &str =
  r#"sandbox_workspace_write.writable_roots=["C:\\repo\\.orchester\\target","/shared/it's \"here\""]"#;

#[test]
fn writable_roots_reach_codex_in_new_and_resumed_sessions() {
  let new = codex_args(RepositoryAccess::Write, None, &roots());
  let sandbox = new.iter().position(|arg| arg == "--sandbox").expect("sandbox");
  assert_eq!(
    new[sandbox..],
    [
      "--sandbox",
      "workspace-write",
      "-c",
      "windows.sandbox=elevated",
      "-c",
      CODEX_ROOTS,
      "-C",
      "/repo/.orchester/worktrees/run",
      "-",
    ]
  );
  let session = "01a0d831-b556-7730-be9d-2491a9359250";
  let resumed = codex_args(RepositoryAccess::Write, Some(session), &roots());
  let mode = resumed
    .iter()
    .position(|arg| arg == "sandbox_mode=workspace-write")
    .expect("sandbox mode");
  assert_eq!(
    resumed[mode..],
    [
      "sandbox_mode=workspace-write",
      "-c",
      "windows.sandbox=elevated",
      "-c",
      CODEX_ROOTS,
      session,
      "-",
    ]
  );
  for args in [
    codex_args(RepositoryAccess::ReadOnly, None, &roots()),
    codex_args(RepositoryAccess::Write, None, &[]),
  ] {
    assert!(!args.iter().any(|arg| arg.contains("writable_roots")), "{args:?}");
  }
}

fn claude_args(permissions: &AgentPermissions, session: ClaudeSession<'_>, roots: &[PathBuf]) -> Vec<String> {
  let enforced = permissions
    .enforced_on(AgentProvider::ClaudeCodeCli, Platform::Windows)
    .expect("enforceable");
  ClaudeInvocation {
    executable: Path::new("/bin/claude"),
    model: "haiku",
    effort: None,
    permissions: &enforced,
    writable_roots: roots,
    worktree: Path::new("/repo/.orchester/worktrees/run"),
    session,
    mcp: None,
    skills: None,
  }
  .args()
  .into_iter()
  .map(|arg| arg.into_string().expect("unicode"))
  .collect()
}

#[test]
fn writable_roots_reach_claude_as_one_add_dir_each_in_new_and_resumed_sessions() {
  let permissions = edit_permissions(&[".orchester/target", "../shared"]);
  let id = Uuid::from_u128(1);
  for (session, tail) in [
    (ClaudeSession::New(id), ["--session-id".to_owned(), id.to_string()]),
    (ClaudeSession::Resume("s-1"), ["--resume".to_owned(), "s-1".to_owned()]),
  ] {
    let args = claude_args(&permissions, session, &roots());
    let first = args.iter().position(|arg| arg == "--add-dir").expect("add-dir");
    let mut expected: Vec<String> = ["--add-dir", ROOTS[0], "--add-dir", ROOTS[1]]
      .map(str::to_owned)
      .to_vec();
    expected.extend(tail);
    assert_eq!(args[first..], expected);
  }
  let read_only = AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    CommandPermission::Allowed(AllowedCommands::default()),
    NetworkAccess::On,
  )
  .with_writable_folders(folders(&[".orchester/target"]));
  let args = claude_args(&read_only, ClaudeSession::New(id), &roots());
  assert!(!args.contains(&"--add-dir".to_owned()), "{args:?}");
}
