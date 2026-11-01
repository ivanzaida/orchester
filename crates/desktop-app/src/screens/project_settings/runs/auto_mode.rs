//! Approvals (`Auto mode` `WSQqT` in `02 · Runs`, its auto-mode parts superseded by `m0eUB` / `tvmsJ` of
//! `docs/design/approval-modes.md`): *Allow Autopilot for agents in this project* and Manual's reviewer, saved as soon
//! as they change, like the page's other settings, and the project's approval rules. Choosing another reviewer widens
//! what is approved in the person's place in every Manual run, so only a person's input does it (or automation's under
//! both of its opt-ins, [`Guarded::ApprovalWidening`]); such a change from other input is put back and said under the
//! group.
mod autopilot;
mod controls;
mod feedback;
mod rules;

use std::sync::{
  Arc,
  atomic::{AtomicBool, Ordering},
};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  components::Row,
  core::Signal,
  node::Element,
};
use orchester_core::{run::review::AutoModeSettings, time::Timestamp};
use orchester_workspace::project_settings::{self, Settings};

use super::super::{
  Status, TabProps,
  work::{self, Change, Save},
};
use crate::{
  mcp::input_gate::{Guarded, InputGate, input_may_decide},
  screens::reviewer_trouble,
  ui::unavailable::reason_line,
};

#[derive(Clone)]
pub struct Props {
  pub tab: TabProps,
  pub settings: Settings,
  pub parent_read: Signal<Option<work::Read>>,
  pub refresh: Signal<u64>,
  /// General's own saves, which a reviewer saved here is one of.
  pub own: work::OwnSaves,
}
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.tab == other.tab && self.settings == other.settings
  }
}
impl DevtoolsInspectable for Props {}
#[derive(Clone, PartialEq)]
struct Stored(Settings);
impl DevtoolsInspectable for Stored {}
#[derive(Clone, PartialEq)]
struct Asked(Option<AutoModeSettings>);
impl DevtoolsInspectable for Asked {}

#[derive(Clone)]
pub struct AutoMode {
  save: Save,
  connection: Signal<String>,
  /// Whether the reviewer's connection menu is open.
  connection_open: Signal<bool>,
  model: Signal<String>,
  effort: Signal<String>,
  stored: Signal<Stored>,
  supplied: Signal<Stored>,
  attempted: Signal<Asked>,
  failed: Signal<bool>,
  gate: Option<InputGate>,
  /// An MCP client's input asked to allow Autopilot and was refused.
  autopilot_refused: Signal<bool>,
  /// Whether the input that made the last edit may choose another reviewer, read while it was delivered.
  may_widen: Arc<AtomicBool>,
  /// A widening from input that may not make it was put back; said under the group until the next change.
  refused: Signal<bool>,
}
impl Component for AutoMode {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    let settings = ctx.props::<Props>().settings.clone();
    let (connection, model, effort) = values(&settings);
    let tab = Self {
      save: work::action(ctx, ctx.props::<Props>().tab.project.open.project.id()),
      connection: ctx.signal(connection),
      connection_open: ctx.signal(false),
      model: ctx.signal(model),
      effort: ctx.signal(effort),
      stored: ctx.signal(Stored(settings.clone())),
      supplied: ctx.signal(Stored(settings)),
      attempted: ctx.signal(Asked(None)),
      failed: ctx.signal(false),
      gate: ctx.use_context::<InputGate>(),
      autopilot_refused: ctx.signal(false),
      may_widen: Arc::new(AtomicBool::new(true)),
      refused: ctx.signal(false),
    };
    let save = tab.save.clone();
    tab.follow(ctx, &save);
    let after = tab.clone();
    ctx.watch(&tab.connection, move |connection| {
      if *connection == values(&after.stored.get_untracked().0).0 {
        return;
      }
      after.model.set(project_settings::default_model(
        &after.stored.get_untracked().0,
        connection,
      ));
      after.effort.set(String::new());
    });
    let after = tab.clone();
    ctx.watch(&tab.model, move |model| {
      if *model != values(&after.stored.get_untracked().0).1 {
        after.effort.set(String::new());
      }
    });
    // A select's choice is taken in its handler, so who chose is read while the input is delivered.
    for chosen in [&tab.connection, &tab.model, &tab.effort] {
      let after = tab.clone();
      ctx.watch(chosen, move |_| after.note_input());
    }
    tab
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let save = self.save.clone();
    self.refresh(&props.settings, save.state().get().is_pending());
    let outside = self.outside();
    if !outside {
      self.commit(&props, &save);
    }
    let settings = self.stored.get().0;
    let cannot_run = cannot_run(ctx, &props, &settings);
    let mut group = controls::group(ctx, (self, &props), (&settings, cannot_run), &save);
    if self.refused.get() {
      group = group.after(reason_line(
        "settings-auto-person-only",
        &ctx.t("project_settings.autoPersonOnly"),
      ));
    }
    if self.failed.get() {
      group = group.after(
        Row::new()
          .spacing(SpacingSize::Sm)
          .align_items(lurq::layout::Alignment::Center)
          .with_children(work::error(ctx, &save))
          .child(self.retry(ctx, &props, &save)),
      );
    }
    if outside {
      group = group.after(self.reload(ctx, &props));
    }
    // A neighboring save must not label this unfinished reviewer as saved.
    props.tab.status.get();
    if save.state().get().is_pending() {
      props.tab.status.set(Status::Saving);
    } else if outside {
      props.tab.status.set(Status::Unsaved);
    } else if self.failed.get() {
      props.tab.status.set(Status::Failed);
    }
    Element::from(group)
  }
}
impl AutoMode {
  /// Notes whether the input being delivered may choose another reviewer: a person's always; an MCP client's only
  /// under the stored opt-ins. Called from a control's handler, or a watcher it sets off.
  fn note_input(&self) {
    let may = input_may_decide(self.gate.as_ref(), Guarded::ApprovalWidening);
    self.may_widen.store(may, Ordering::SeqCst);
  }
  fn dirty(&self) -> bool {
    let stored = self.stored.get_untracked().0;
    (
      self.connection.get_untracked(),
      self.model.get_untracked(),
      self.effort.get_untracked(),
    ) != values(&stored)
      || self.failed.get_untracked()
  }
  fn outside(&self) -> bool {
    let (stored, supplied) = (self.stored.get().0, self.supplied.get().0);
    stored.auto_mode != supplied.auto_mode || stored.connections != supplied.connections
  }
  fn refresh(&self, supplied: &Settings, pending: bool) {
    if pending || *supplied == self.supplied.get_untracked().0 {
      return;
    }
    self.supplied.set(Stored(supplied.clone()));
    if !self.dirty() {
      self.fill(supplied.clone());
      self.attempted.set(Asked(None));
    } else {
      // Unrelated settings and run authority may refresh without discarding the reviewed baseline.
      let mut stored = supplied.clone();
      stored.auto_mode = self.stored.get_untracked().0.auto_mode;
      stored.connections = self.stored.get_untracked().0.connections;
      self.stored.set(Stored(stored));
    }
  }
  fn fill(&self, settings: Settings) {
    let (connection, model, effort) = values(&settings);
    // Publish the baseline before changing the select signals their watchers read.
    self.stored.set(Stored(settings));
    self.connection.set(connection);
    self.model.set(model);
    self.effort.set(effort);
  }
  fn follow(&self, ctx: &mut Ctx, save: &Save) {
    let (tab, status, feed) = (
      self.clone(),
      ctx.props::<Props>().tab.status.clone(),
      ctx.props::<Props>().tab.project.feed.clone(),
    );
    let props = ctx.props::<Props>().clone();
    let begun = std::sync::atomic::AtomicBool::new(false);
    ctx.watch(&save.state(), move |state| {
      props.own.saving(state.is_pending(), &begun);
      if state.is_pending() {
        status.set(Status::Saving);
      } else if state.error.is_some() {
        tab.failed.set(true);
        status.set(Status::Failed);
      } else if let Some(saved) = &state.data {
        tab.fill(saved.0.clone());
        // Invalidate older parent reads before they can replace this saved reviewer or a new edit.
        props.parent_read.set(Some(work::Read(saved.0.clone(), u64::MAX)));
        props.own.took(saved);
        props.refresh.update(|revision| *revision += 1);
        tab.failed.set(false);
        status.set(Status::Saved);
        feed.saved();
      }
    });
  }
  fn draft(&self) -> Result<AutoModeSettings, orchester_workspace::messages::Message> {
    let settings = self.stored.get().0;
    let reviewer =
      project_settings::reviewer(&settings, &self.connection.get(), &self.model.get(), &self.effort.get())?;
    let mut asked = settings.auto_mode;
    asked.reviewer = reviewer;
    Ok(asked)
  }
  fn commit(&self, props: &Props, save: &Save) {
    if save.state().get().is_pending() {
      return;
    }
    let settings = self.stored.get().0;
    let Ok(asked) = self.draft() else {
      return;
    };
    if asked == settings.auto_mode {
      if self.failed.get_untracked()
        && self
          .attempted
          .get_untracked()
          .0
          .is_some_and(|attempt| attempt.reviewer != asked.reviewer)
      {
        self.failed.set(false);
        self.attempted.set(Asked(None));
        save.state().set(lurq::app::ctx::FutureState::idle());
        props.tab.status.set(Status::Saved);
      }
      return;
    }
    if self.attempted.get_untracked().0.as_ref() == Some(&asked) {
      return;
    }
    self.request(props, save, asked);
  }
  /// Saves `asked` at once; a widening from input that may not make it is put back and said instead.
  fn request(&self, props: &Props, save: &Save, asked: AutoModeSettings) {
    let settings = self.stored.get_untracked().0;
    let base = settings.auto_mode.clone();
    let confirmed = self.may_widen.load(Ordering::SeqCst);
    self.failed.set(false);
    self.refused.set(false);
    if !confirmed && project_settings::reviewer_widens(&base, &asked) {
      tracing::warn!("input that may not choose another reviewer changed it; put back");
      self.attempted.set(Asked(None));
      self.fill(settings);
      self.refused.set(true);
      return;
    }
    self.attempted.set(Asked(Some(asked.clone())));
    // The workspace refuses a widening it was not told a person (or automation under its opt-ins) made.
    save.run((
      props.tab.project.open.repo_root.clone(),
      Change::Auto { base, asked, confirmed },
    ));
  }
}
/// Why the stored reviewer cannot run, said in its row's hint's place (ORC-611): its account cannot, or its last
/// review failed. Read again whenever the reviewer or the project's connections change.
fn cannot_run(ctx: &mut Ctx, props: &Props, settings: &Settings) -> Option<Arc<str>> {
  let root = props.tab.project.open.repo_root.clone();
  let key = format!("{:?}|{:?}", settings.auto_mode, settings.connections);
  let health = reviewer_trouble::health(ctx, root, key)?;
  let now = Timestamp::now();
  let (key, trouble) = match health.account.trouble(now) {
    Some(trouble) => ("project_settings.reviewerCannotRun", trouble),
    None => ("project_settings.reviewerLastFailed", health.reviews(now)?),
  };
  let reason = reviewer_trouble::reason(ctx, trouble, now);
  Some(ctx.t_args(key, [("reason", &*reason)]))
}
fn values(settings: &Settings) -> (String, String, String) {
  match &settings.auto_mode.reviewer {
    Some(choice) => (
      choice.connection.to_string(),
      choice.model.as_str().to_owned(),
      choice.effort.clone().unwrap_or_default(),
    ),
    None => (
      String::new(),
      project_settings::review_options(settings, "", "")
        .resolved
        .map(|r| r.model.as_str().to_owned())
        .unwrap_or_default(),
      String::new(),
    ),
  }
}
