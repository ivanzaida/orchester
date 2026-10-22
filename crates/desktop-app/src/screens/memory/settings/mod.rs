//! The Settings tab of the project's Memory page (`Tab body · Settings` `oNMeD` in `U2teQq`): Project settings ›
//! Memory, whose home it is while Project settings has no Memory tab. How agents' records are saved in task and in
//! project memory, each a choice of two cards that saves when chosen; how many records and tokens each agent
//! invocation gets, saved when its field is left; and its save state at the end of its note, worded as Project
//! settings words it. A save rebases what it changed onto the stored settings (`memory::settings::save_in`), and the
//! page reads its memory again after it. Only a person saves: an MCP client's window input is refused.

mod form;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    events::is_synthetic_input,
  },
  core::Signal,
  node::Element,
};
use orchester_core::memory::{MemoryLimits, MemorySettings, WritePolicy};
use orchester_workspace::{
  background,
  memory::settings,
  messages::{self, Message},
};

use crate::project::ShownProject;

#[derive(Clone, PartialEq)]
pub struct SettingsProps {
  pub project: ShownProject,
  /// The settings as the page read them.
  pub stored: MemorySettings,
}

impl DevtoolsInspectable for SettingsProps {}

/// Settings a save asked for or stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Kept(pub MemorySettings);

impl DevtoolsInspectable for Kept {}

/// What a save asks for: `asked` where it changed `base`, in the project at `repo`.
#[derive(Clone)]
pub struct Asked {
  repo: PathBuf,
  base: MemorySettings,
  asked: MemorySettings,
}

type Save = FutureAction<Asked, Kept, Message>;

/// What the tab says about its changes (`Save status` `myC5w`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Status {
  Saved,
  Unsaved,
  Invalid,
  Saving,
  Failed,
}

/// Which memory's saving a choice changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scope {
  Task,
  Project,
}

#[derive(Clone)]
pub struct SettingsTab {
  records: Signal<String>,
  tokens: Signal<String>,
  /// The settings the fields were filled from: the base of the next save.
  base: Signal<Option<Kept>>,
  /// The settings the page read last time the tab looked.
  seen: Signal<Option<Kept>>,
  /// The settings the running save asked for.
  submitted: Signal<Option<Kept>>,
  /// Synthetic input asked for the last save, which was not taken.
  refused: Signal<bool>,
  save: Save,
  separator: Arc<str>,
}

impl Component for SettingsTab {
  type Props = SettingsProps;

  fn create(ctx: &mut Ctx) -> Self {
    let tab = Self {
      records: ctx.signal(String::new()),
      tokens: ctx.signal(String::new()),
      base: ctx.signal(None),
      seen: ctx.signal(None),
      submitted: ctx.signal(None),
      refused: ctx.signal(false),
      save: action(ctx),
      separator: ctx.t("memory.groupSeparator"),
    };
    let (after, feed) = (tab.clone(), ctx.props::<SettingsProps>().project.feed.clone());
    ctx.watch(&tab.save.state(), move |state| {
      if state.is_pending() || state.error.is_some() {
        return;
      }
      if let Some(Kept(saved)) = state.data {
        after.saved(saved);
        feed.saved();
      }
    });
    tab
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<SettingsProps>().clone();
    self.follow(props.stored);
    let base = self.base.get().map_or(props.stored, |Kept(base)| base);
    form::form(ctx, self, (&props, base))
  }
}

impl SettingsTab {
  /// Fills the fields from the settings the page read when they are first shown, and when they changed since while
  /// nothing here waits to be saved: a change made elsewhere, or this tab's own save read again.
  fn follow(&self, stored: MemorySettings) {
    if self.seen.get_untracked() == Some(Kept(stored)) {
      return;
    }
    self.seen.set(Some(Kept(stored)));
    let waiting = self.save.state().get_untracked().is_pending()
      || self.base.get_untracked().is_some_and(|Kept(base)| self.dirty(base));
    if self.base.get_untracked().is_none() || !waiting {
      self.fill(stored);
    }
  }

  fn fill(&self, settings: MemorySettings) {
    self.records.set(settings.limits.records().to_string());
    self.tokens.set(self.grouped(settings.limits.tokens()));
    self.base.set(Some(Kept(settings)));
  }

  /// Takes what a save stored as the new base, and its value into each field still holding what it submitted.
  fn saved(&self, saved: MemorySettings) {
    if let Some(Kept(sent)) = self.submitted.get_untracked() {
      if self.records.get_untracked() == sent.limits.records().to_string() {
        self.records.set(saved.limits.records().to_string());
      }
      if self.tokens.get_untracked() == self.grouped(sent.limits.tokens()) {
        self.tokens.set(self.grouped(saved.limits.tokens()));
      }
    }
    self.submitted.set(None);
    self.base.set(Some(Kept(saved)));
  }

  fn grouped(&self, tokens: u32) -> String {
    orchester_core::tokens::grouped(u64::from(tokens), &self.separator)
  }

  /// Whether a field holds something other than `base`.
  fn dirty(&self, base: MemorySettings) -> bool {
    self.records.get_untracked() != base.limits.records().to_string()
      || self.tokens.get_untracked() != self.grouped(base.limits.tokens())
  }

  fn read_records(&self) -> Result<u16, Message> {
    settings::read_records(&self.records.get())
  }

  fn read_tokens(&self) -> Result<u32, Message> {
    settings::read_tokens(&self.tokens.get(), &self.separator)
  }

  /// `base` with the limits the fields hold, while both are values.
  fn with_limits(&self, base: MemorySettings) -> Option<MemorySettings> {
    let limits = MemoryLimits::new(self.read_records().ok()?, self.read_tokens().ok()?).ok()?;
    Some(MemorySettings { limits, ..base })
  }

  fn status(&self, base: MemorySettings) -> Status {
    let state = self.save.state().get();
    if state.is_pending() {
      return Status::Saving;
    }
    if self.read_records().is_err() || self.read_tokens().is_err() {
      return Status::Invalid;
    }
    if state.error.is_some() {
      return Status::Failed;
    }
    if self.dirty(base) {
      Status::Unsaved
    } else {
      Status::Saved
    }
  }

  /// Saves the limits the fields hold, once one is left.
  fn commit_limits(&self, repo: PathBuf) {
    let Some(Kept(base)) = self.base.get_untracked() else {
      return;
    };
    if let Some(asked) = self.with_limits(base) {
      self.submit(repo, base, asked);
    }
  }

  /// Saves `policy` for `scope`'s records, with the limits the fields hold while they are values.
  fn choose(&self, repo: PathBuf, (scope, policy): (Scope, WritePolicy)) {
    let Some(Kept(base)) = self.base.get_untracked() else {
      return;
    };
    let mut asked = self.with_limits(base).unwrap_or(base);
    match scope {
      Scope::Task => asked.task_writes = policy,
      Scope::Project => asked.project_writes = policy,
    }
    self.submit(repo, base, asked);
  }

  /// Saves `asked` over `base`. Called in a handler: only a person changes these settings, as they decide whether
  /// agents' records reach every later agent without approval and how much each agent gets. lurq's synthetic input
  /// (an MCP client driving the window) is refused, and the tab says so under its head.
  fn submit(&self, repo: PathBuf, base: MemorySettings, asked: MemorySettings) {
    if asked == base || self.save.state().get_untracked().is_pending() {
      return;
    }
    let synthetic = is_synthetic_input();
    self.refused.set(synthetic);
    if synthetic {
      tracing::warn!("synthetic input asked to change the memory settings; not taken");
      return;
    }
    self.submitted.set(Some(Kept(asked)));
    self.save.run(Asked { repo, base, asked });
  }
}

fn action(ctx: &mut Ctx) -> Save {
  ctx.future_action(move |Asked { repo, base, asked }: Asked| async move {
    background::run(move || settings::save_in(&repo, base, asked))
      .await?
      .map(Kept)
      .map_err(|error| messages::store_error(&error))
  })
}
