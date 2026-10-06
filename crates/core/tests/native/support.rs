//! A temporary repository layout for the native scope, and helpers that script the native loop.

use std::{
  fs,
  path::{Path, PathBuf},
};

use orchester_core::{
  id::ConversationId,
  native::{
    command::CommandPolicy,
    item::{Item, ToolCallId},
    model::{ModelRequest, ModelResponse, StopReason},
    scope::{FileScope, ScopeRoots},
    session::{LoopAction, LoopEnd, LoopSettings, NativeLoop, PlannedTool, ToolOutcome},
    tools::ToolSet,
  },
  permission::{AllowedCommands, CommandPermission, CommandPrefix, FolderBounds, RepositoryAccess, WritableFolder},
  resource::ModelId,
  run::TokenUsage,
};
use tempfile::TempDir;

/// `repo` with a run worktree, the code index, a skills folder and a writable folder under `.orchester/`,
/// and a folder `outside` with `secret.txt`. The worktree holds `src/main.rs`, a `.git` file and a nested
/// `vendor/lib/.git` folder.
pub struct Layout {
  // Removed with the layout.
  _root: TempDir,
  pub root: PathBuf,
  pub worktree: PathBuf,
  pub index: PathBuf,
  pub skills: PathBuf,
  pub cache: PathBuf,
  pub outside: PathBuf,
}

impl Layout {
  pub fn new() -> Self {
    let dir = tempfile::tempdir().expect("temporary folder");
    let root = plain(&fs::canonicalize(dir.path()).expect("canonical root"));
    let orchester = root.join("repo/.orchester");
    let worktree = orchester.join("worktrees/run");
    for folder in [
      worktree.join("src"),
      worktree.join("vendor/lib/.git/hooks"),
      orchester.join("index/rev"),
      orchester.join("skills/exec"),
      orchester.join("cache"),
      root.join("outside"),
    ] {
      fs::create_dir_all(folder).expect("folder");
    }
    fs::write(worktree.join("src/main.rs"), "fn main() {}\n").expect("file");
    fs::write(worktree.join(".git"), "gitdir: elsewhere\n").expect("file");
    fs::write(orchester.join("index/rev/map.md"), "# map\n").expect("file");
    fs::write(root.join("outside/secret.txt"), "secret\n").expect("file");
    Self {
      worktree,
      index: orchester.join("index/rev"),
      skills: orchester.join("skills/exec"),
      cache: orchester.join("cache"),
      outside: root.join("outside"),
      root,
      _root: dir,
    }
  }

  /// The scope of a step with `access`, granted the writable folder `.orchester/cache`.
  pub fn scope(&self, access: RepositoryAccess) -> FileScope {
    let bounds = FolderBounds::new(&self.root.join("repo"), None).expect("bounds");
    let folder = WritableFolder::new(".orchester/cache", &bounds).expect("writable folder");
    let resolved = [folder.resolve(&bounds).expect("resolved folder")];
    FileScope::new(&ScopeRoots {
      worktree: &self.worktree,
      access,
      code_index: Some(&self.index),
      skills: Some(&self.skills),
      writable_folders: &resolved,
    })
    .expect("scope")
  }
}

/// A canonical Windows path (`\\?\C:\…`) in the form a person writes; other paths unchanged.
fn plain(path: &Path) -> PathBuf {
  let text = text(path);
  PathBuf::from(text.strip_prefix(r"\\?\").unwrap_or(&text))
}

/// `path` as a string a tool call would carry.
pub fn text(path: &Path) -> String {
  path.to_str().expect("UTF-8 path").to_owned()
}

pub fn allowed(entries: &[&str]) -> CommandPermission {
  let entries = entries
    .iter()
    .map(|entry| CommandPrefix::new(entry).expect("prefix"))
    .collect();
  CommandPermission::Allowed(AllowedCommands::new(entries).expect("list"))
}

pub fn call(id: &str) -> ToolCallId {
  ToolCallId::new(id).expect("call id")
}

pub fn tool_call(id: &str, name: &str, input: serde_json::Value) -> Item {
  Item::ToolCall {
    id: call(id),
    name: name.to_owned(),
    input,
  }
}

pub fn assistant(text: &str) -> Item {
  Item::Assistant { text: text.to_owned() }
}

pub fn response(items: Vec<Item>, stop: StopReason, input: u64) -> ModelResponse {
  ModelResponse {
    items,
    stop,
    usage: TokenUsage {
      input,
      cached_input: 0,
      output: 10,
    },
    served_by: model(),
  }
}

pub fn model() -> ModelId {
  ModelId::new("claude-opus-5-5").expect("model id")
}

pub fn settings(access: RepositoryAccess, context_window: u64) -> LoopSettings {
  LoopSettings {
    conversation: ConversationId::generate(),
    model: model(),
    effort: None,
    max_output: 1000,
    context_window,
    system: "You are a native agent.".to_owned(),
    tools: ToolSet::for_access(access),
    shell: None,
  }
}

/// A loop started in `layout` with `access` and the allowed commands `list`.
pub fn start(layout: &Layout, access: RepositoryAccess, list: &[&str]) -> (NativeLoop, Vec<LoopAction>) {
  start_with(layout, access, settings(access, 200_000), list)
}

pub fn start_with(
  layout: &Layout,
  access: RepositoryAccess,
  settings: LoopSettings,
  list: &[&str],
) -> (NativeLoop, Vec<LoopAction>) {
  let policy = CommandPolicy::new(&allowed(list)).expect("policy");
  NativeLoop::start(settings, layout.scope(access), policy, "Do the task.".to_owned())
}

pub fn ok(content: &str) -> ToolOutcome {
  ToolOutcome {
    content: content.to_owned(),
    is_error: false,
    activity: String::new(),
    exit_code: None,
    changes: Vec::new(),
    detail: None,
  }
}

/// The calls of the `Run` action among `actions`.
pub fn run_ids(actions: &[LoopAction]) -> Vec<String> {
  planned(actions)
    .iter()
    .map(|tool| tool.call.as_str().to_owned())
    .collect()
}

pub fn planned(actions: &[LoopAction]) -> Vec<PlannedTool> {
  actions
    .iter()
    .find_map(|action| match action {
      LoopAction::Run(batch) => Some(batch.clone()),
      _ => None,
    })
    .unwrap_or_default()
}

pub fn request(actions: &[LoopAction]) -> Option<&ModelRequest> {
  actions.iter().find_map(|action| match action {
    LoopAction::CallModel(request) => Some(request),
    _ => None,
  })
}

pub fn ended(actions: &[LoopAction]) -> Option<&LoopEnd> {
  actions.iter().find_map(|action| match action {
    LoopAction::End(end) => Some(end),
    _ => None,
  })
}

/// Every item the actions have stored, in order.
pub fn persisted(actions: &[LoopAction]) -> Vec<Item> {
  actions
    .iter()
    .filter_map(|action| match action {
      LoopAction::Persist(items) => Some(items.clone()),
      _ => None,
    })
    .flatten()
    .collect()
}
