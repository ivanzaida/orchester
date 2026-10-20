//! Auto mode's reviewer and the input-locked, in-place person-only question, and the project's approval rules.
mod controls;
mod feedback;
mod rules;

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  components::Column,
  core::Signal,
  node::Element,
};
use orchester_core::run::review::AutoModeSettings;
use orchester_workspace::project_settings::{self, Settings};

use super::super::{
  Status, TabProps,
  work::{self, Change, Save},
};
use crate::screens::connections::widen::{WidenConfirm, WidenProps};

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
  asking: Signal<Asked>,
  attempted: Signal<Asked>,
  failed: Signal<bool>,
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
      asking: ctx.signal(Asked(None)),
      attempted: ctx.signal(Asked(None)),
      failed: ctx.signal(false),
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
    let mut content = Column::new()
      .spacing(SpacingSize::Md)
      .child(controls::section(ctx, self, &props, &settings, &save))
      .with_children(self.failed.get().then(|| work::error(ctx, &save)).flatten())
      .with_children(self.failed.get().then(|| self.retry(ctx, &props, &save)))
      .with_children(outside.then(|| self.reload(ctx, &props)));
    // A neighboring General save must not label this unfinished reviewer as saved.
    props.tab.status.get();
    if save.state().get().is_pending() {
      props.tab.status.set(Status::Saving);
    } else if outside || self.asking.get().0.is_some() {
      props.tab.status.set(Status::Unsaved);
    } else if self.failed.get() {
      props.tab.status.set(Status::Failed);
    }
    if let Some(asked) = self.asking.get().0 {
      content = content.child(self.question(ctx, &props, &save, asked));
    }
    content
  }
}
impl AutoMode {
  fn dirty(&self) -> bool {
    let stored = self.stored.get_untracked().0;
    (
      self.connection.get_untracked(),
      self.model.get_untracked(),
      self.effort.get_untracked(),
    ) != values(&stored)
      || self.asking.get_untracked().0.is_some()
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
        tab.asking.set(Asked(None));
        status.set(Status::Failed);
      } else if let Some(saved) = &state.data {
        tab.fill(saved.0.clone());
        // Invalidate older parent reads before they can replace this saved reviewer or a new edit.
        props.parent_read.set(Some(work::Read(saved.0.clone(), u64::MAX)));
        props.own.took(saved);
        props.refresh.update(|revision| *revision += 1);
        tab.asking.set(Asked(None));
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
    if save.state().get().is_pending() || self.asking.get().0.is_some() {
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
  fn request(&self, props: &Props, save: &Save, asked: AutoModeSettings) {
    let settings = self.stored.get_untracked().0;
    let base = settings.auto_mode;
    self.failed.set(false);
    self.attempted.set(Asked(Some(asked.clone())));
    if project_settings::auto_widens(&base, &asked, settings.auto_runs) {
      self.asking.set(Asked(Some(asked)));
      props.tab.status.set(Status::Unsaved);
    } else {
      save.run((
        props.tab.project.open.repo_root.clone(),
        Change::Auto {
          base,
          asked,
          confirmed: false,
        },
      ));
    }
  }
  fn question(&self, ctx: &mut Ctx, props: &Props, save: &Save, asked: AutoModeSettings) -> Element {
    let (tab, save, root, base, status) = (
      self.clone(),
      save.clone(),
      props.tab.project.open.repo_root.clone(),
      self.stored.get_untracked().0.auto_mode,
      props.tab.status.clone(),
    );
    ctx.mount_keyed::<WidenConfirm>(
      "settings-auto-confirm",
      WidenProps {
        id: "settings-auto-question".into(),
        card: false,
        affirming: false,
        automation: crate::mcp::input_gate::Guarded::ApprovalWidening,
        title: ctx.t("project_settings.confirmAuto"),
        reasons: vec![ctx.t("project_settings.confirmAutoDetail")].into(),
        keep: ctx.t("project_settings.keep"),
        confirm: ctx.t("project_settings.confirm"),
        on_keep: Arc::new(move || {
          tab.fill(tab.stored.get_untracked().0);
          tab.attempted.set(Asked(None));
          tab.asking.set(Asked(None));
          status.set(Status::Saved);
        }),
        on_confirm: Arc::new(move || {
          if !save.state().get_untracked().is_pending() {
            save.run((
              root.clone(),
              Change::Auto {
                base: base.clone(),
                asked: asked.clone(),
                confirmed: true,
              },
            ));
          }
        }),
      },
    )
  }
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
