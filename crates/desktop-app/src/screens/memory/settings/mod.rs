//! Project settings › Memory, as two views edit it: the Settings tab of the project's Memory page (`Tab body ·
//! Settings` `oNMeD` in `U2teQq`), each memory a choice of two cards, and the Memory group of Project settings ›
//! Context (`project_settings::context::memory`), each memory a select. Both share one [`MemoryForm`]: how agents'
//! records are saved in task and in project memory, saved when chosen; how many records and tokens each agent
//! invocation gets, saved when its field is left; and the save state, worded as Project settings words it. A save
//! rebases what it changed onto the stored settings (`memory::settings::save_in`), and the project's screens read its
//! memory again after it. Only a person saves: an MCP client's window input is refused.

mod form;

use std::{
  path::PathBuf,
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
};

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

pub(crate) type Save = FutureAction<Asked, Kept, Message>;

/// What a view says about its changes (`Save status` `myC5w`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Status {
  Saved,
  Unsaved,
  Invalid,
  Saving,
  Failed,
}

/// Which memory's saving a choice changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Scope {
  Task,
  Project,
}

impl Scope {
  /// How `settings` save this memory's records.
  pub(crate) const fn of(self, settings: MemorySettings) -> WritePolicy {
    match self {
      Self::Task => settings.task_writes,
      Self::Project => settings.project_writes,
    }
  }
}

/// The settings being edited in a view and their save.
#[derive(Clone)]
pub(crate) struct MemoryForm {
  pub(crate) records: Signal<String>,
  pub(crate) tokens: Signal<String>,
  /// The policy each memory's select holds, by its stored name ([`WritePolicy::name`]).
  pub(crate) task: Signal<String>,
  pub(crate) project: Signal<String>,
  /// The settings the fields were filled from: the base of the next save.
  base: Signal<Option<Kept>>,
  /// The settings read last time the view looked.
  seen: Signal<Option<Kept>>,
  /// The settings the running save asked for.
  submitted: Signal<Option<Kept>>,
  /// Synthetic input asked for the last save, which was not taken.
  pub(crate) refused: Signal<bool>,
  /// The selects are being put back on settings, not chosen.
  putting_back: Arc<AtomicBool>,
  pub(crate) save: Save,
  pub(crate) separator: Arc<str>,
  repo: PathBuf,
}

impl MemoryForm {
  /// The form of `project`'s memory settings; each save the project's screens read again.
  pub(crate) fn new(ctx: &mut Ctx, project: &ShownProject) -> Self {
    let form = Self {
      records: ctx.signal(String::new()),
      tokens: ctx.signal(String::new()),
      task: ctx.signal(String::new()),
      project: ctx.signal(String::new()),
      base: ctx.signal(None),
      seen: ctx.signal(None),
      submitted: ctx.signal(None),
      refused: ctx.signal(false),
      putting_back: Arc::default(),
      save: action(ctx),
      separator: ctx.t("memory.groupSeparator"),
      repo: project.open.repo_root.clone(),
    };
    let (after, feed) = (form.clone(), project.feed.clone());
    ctx.watch(&form.save.state(), move |state| {
      if state.is_pending() || state.error.is_some() {
        return;
      }
      if let Some(Kept(saved)) = state.data {
        after.saved(saved);
        feed.saved();
      }
    });
    // A select's choice is taken in its handler, so who chose is read while the input is delivered.
    for (scope, chosen) in [(Scope::Task, &form.task), (Scope::Project, &form.project)] {
      let after = form.clone();
      ctx.watch(chosen, move |name| {
        if let Some(policy) = WritePolicy::from_name(name) {
          after.choose((scope, policy));
        }
      });
    }
    form
  }

  /// The settings the fields were filled from, else `stored`.
  pub(crate) fn base(&self, stored: MemorySettings) -> MemorySettings {
    self.base.get().map_or(stored, |Kept(base)| base)
  }

  /// Fills the fields from the settings read when they are first shown, and when they changed since while nothing
  /// here waits to be saved: a change made elsewhere, or this view's own save read again.
  pub(crate) fn follow(&self, stored: MemorySettings) {
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

  /// Puts each select back on `base` where its choice was not taken: refused, failed, or made while a save ran.
  pub(crate) fn settle(&self, base: MemorySettings) {
    if self.save.state().get_untracked().is_pending() {
      return;
    }
    self.choices(base);
  }

  fn fill(&self, settings: MemorySettings) {
    self.records.set(settings.limits.records().to_string());
    self.tokens.set(self.grouped(settings.limits.tokens()));
    // The base first: the selects' watchers compare a choice with it.
    self.base.set(Some(Kept(settings)));
    self.choices(settings);
  }

  /// The selects on `settings`' policies, which their watchers do not take as a choice.
  fn choices(&self, settings: MemorySettings) {
    self.putting_back.store(true, Ordering::Relaxed);
    for (scope, chosen) in [(Scope::Task, &self.task), (Scope::Project, &self.project)] {
      let name = scope.of(settings).name();
      if chosen.get_untracked() != name {
        chosen.set(name.to_owned());
      }
    }
    self.putting_back.store(false, Ordering::Relaxed);
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
    self.choices(saved);
  }

  pub(crate) fn grouped(&self, tokens: u32) -> String {
    orchester_core::tokens::grouped(u64::from(tokens), &self.separator)
  }

  /// Whether a field holds something other than `base`.
  fn dirty(&self, base: MemorySettings) -> bool {
    self.records.get_untracked() != base.limits.records().to_string()
      || self.tokens.get_untracked() != self.grouped(base.limits.tokens())
  }

  pub(crate) fn read_records(&self) -> Result<u16, Message> {
    settings::read_records(&self.records.get())
  }

  pub(crate) fn read_tokens(&self) -> Result<u32, Message> {
    settings::read_tokens(&self.tokens.get(), &self.separator)
  }

  /// `base` with the limits the fields hold, while both are values.
  fn with_limits(&self, base: MemorySettings) -> Option<MemorySettings> {
    let limits = MemoryLimits::new(self.read_records().ok()?, self.read_tokens().ok()?).ok()?;
    Some(MemorySettings { limits, ..base })
  }

  pub(crate) fn status(&self, base: MemorySettings) -> Status {
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
  pub(crate) fn commit_limits(&self) {
    let Some(Kept(base)) = self.base.get_untracked() else {
      return;
    };
    if let Some(asked) = self.with_limits(base) {
      self.submit(base, asked);
    }
  }

  /// Saves `policy` for `scope`'s records over the base, and nothing else: the limits the fields hold are saved only
  /// when a field is left, so a select put back on the base ([`Self::settle`], [`Self::fill`], [`Self::saved`]) asks
  /// for nothing and never saves what a field holds.
  pub(crate) fn choose(&self, (scope, policy): (Scope, WritePolicy)) {
    if self.putting_back.load(Ordering::Relaxed) {
      return;
    }
    let Some(Kept(base)) = self.base.get_untracked() else {
      return;
    };
    let mut asked = base;
    match scope {
      Scope::Task => asked.task_writes = policy,
      Scope::Project => asked.project_writes = policy,
    }
    self.submit(base, asked);
  }

  /// Saves `asked` over `base`. Called in a handler: only a person changes these settings, as they decide whether
  /// agents' records reach every later agent without approval and how much each agent gets. lurq's synthetic input
  /// (an MCP client driving the window) is refused, and the view says so.
  fn submit(&self, base: MemorySettings, asked: MemorySettings) {
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
    self.save.run(Asked {
      repo: self.repo.clone(),
      base,
      asked,
    });
  }
}

/// The Memory page's Settings tab.
pub struct SettingsTab(MemoryForm);

impl Component for SettingsTab {
  type Props = SettingsProps;

  fn create(ctx: &mut Ctx) -> Self {
    let project = ctx.props::<SettingsProps>().project.clone();
    Self(MemoryForm::new(ctx, &project))
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<SettingsProps>().clone();
    self.0.follow(props.stored);
    let base = self.0.base(props.stored);
    form::form(ctx, &self.0, (&props, base))
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
