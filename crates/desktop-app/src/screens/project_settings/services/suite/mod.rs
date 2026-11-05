//! A test suite's page under Project settings › Services, for a new suite and an existing one (`04 · Test suite
//! editor` `uxYcD`; `docs/architecture/agent-testing.md`, "Test suite"): the crumb back to Services through Test
//! suites and the head, then in a 720 pane Run (a new suite's name, the program with whether it is found, the
//! arguments one per line, the working folder and the variables) ([`run`]), Report (the format and where it is read
//! from) and Layer and needs (the layer, and nothing, services or a stack started before it with the order they start
//! in and the slots they take) ([`report`]), Artifacts and Limits (the timeout, the filter's argument and whether every
//! Test step runs it) ([`limits`]), and at the pane's end *Delete suite* with the draft's state and Publish; beside the
//! pane *Last runs*, which says plainly that no run's results are kept yet, and *Agents use it with* ([`aside`]).
//!
//! The draft saves as a person leaves a field or makes a choice, as the rest of Project settings does; a new suite is
//! made with its first save, and its page then opens under its id. Every value is checked by the core type it becomes
//! and refused in place (`orchester_workspace::agent_testing::suite_editor::form`). A suite runs its program with the
//! person's rights, so lurq's synthetic input (an MCP client driving the window) changes what it runs or publishes it
//! only under *Let automation change agent permissions*; what such input typed into those fields otherwise is put back
//! as the draft next saves ([`fields`]).

mod aside;
mod fields;
mod limits;
mod report;
mod run;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction, FutureState},
    theme::SpacingSize,
  },
  components::{Column, Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ResourceId, resource::TestSuite};
use orchester_workspace::{
  agent_testing::suite_editor::{
    SuiteRead,
    form::{SuiteForm, SuiteProblems},
    load_suite_in, publish_suite_in, save_suite_in,
  },
  background,
  messages::Message,
};

use self::fields::SuiteFields;
use super::{
  super::{
    Status, TabProps,
    kit::{back_through, page_head},
    page::column_scroll_wide,
  },
  delete::{Deleted, dialog},
  editor::{ASIDE_TOP, Commit, PAGE_WIDTH, PANE_WIDTH},
  lifecycle::{next_version, stale_reason, state_words},
};
use crate::{
  mcp::input_gate::{Guarded, InputGate, input_may_decide},
  navigation::Navigation,
  routes::{ProjectPage, ProjectSettingsTab, Route, ServicesPage},
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::Icon,
    message::message_text,
    state::{load_error, loading},
  },
};

/// The page of suite `id`, or of a new one.
pub(super) fn editor(ctx: &mut Ctx, props: &TabProps, id: Option<ResourceId>) -> Element {
  let key = id.map_or_else(
    || "settings-suite-editor-new".to_owned(),
    |id| format!("settings-suite-editor-{id}"),
  );
  ctx.mount_keyed::<SuiteEditor>(&key, SuiteProps { tab: props.clone(), id })
}

#[derive(Clone)]
struct SuiteProps {
  tab: TabProps,
  id: Option<ResourceId>,
}

impl PartialEq for SuiteProps {
  fn eq(&self, other: &Self) -> bool {
    self.tab == other.tab && self.id == other.id
  }
}

impl DevtoolsInspectable for SuiteProps {}

/// What is stored: the page as it was read, or as the last save or publish read it back.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Read(SuiteRead);

impl DevtoolsInspectable for Read {}

/// A save asked for: the project's folder, the suite (none for a new one) with a new one's name, its body and the
/// revision it is made on.
type SaveAsk = (PathBuf, (Option<ResourceId>, String), TestSuite, Option<String>);
type Save = FutureAction<SaveAsk, Read, Message>;
type Publish = FutureAction<(PathBuf, ResourceId, TestSuite), Read, Message>;

/// What the fields make of themselves against what is stored, for the parts of the page.
struct View {
  stored: SuiteRead,
  built: Result<TestSuite, Box<SuiteProblems>>,
  /// The fields hold a change that is not saved, valid or not.
  changed: bool,
}

impl View {
  fn base(&self) -> Option<&TestSuite> {
    self.stored.entry.as_ref().map(|entry| &entry.body)
  }

  fn problems(&self) -> Option<&SuiteProblems> {
    self.built.as_ref().err().map(AsRef::as_ref)
  }

  /// The suite as the fields make it, or as stored while they hold a mistake.
  fn suite(&self) -> Option<&TestSuite> {
    self.built.as_ref().ok().or_else(|| self.base())
  }
}

/// The form of what `stored` holds, which the fields are compared with and put back to.
fn shown(stored: &SuiteRead) -> SuiteForm {
  let stacks = stored.stack_names();
  stored
    .entry
    .as_ref()
    .map(|entry| SuiteForm::of(&entry.body, &stacks))
    .unwrap_or_default()
}

#[derive(Clone)]
struct SuiteEditor {
  fields: SuiteFields,
  stored: Signal<Option<Read>>,
  /// Counts *Try again*s after a failed read.
  retry: Signal<u64>,
  save: Save,
  publish: Publish,
  /// A field was left or a choice made while a save was under way: the page saves again once it lands.
  again: Signal<bool>,
  /// An MCP client's input asked for what only a person decides here; nothing was saved or published.
  refused: Signal<bool>,
  /// The delete question shows.
  deleting: Signal<bool>,
  gate: Option<InputGate>,
}

impl Component for SuiteEditor {
  type Props = SuiteProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<SuiteProps>().clone();
    let editor = Self {
      fields: SuiteFields::new(ctx),
      stored: ctx.signal(None),
      retry: ctx.signal(0),
      save: ctx.future_action(|(root, (id, name), suite, expected): SaveAsk| async move {
        background::run(move || {
          let saved = save_suite_in(&root, (id, &name), suite, expected.as_deref())?;
          load_suite_in(&root, Some(saved.id)).map(Read)
        })
        .await?
      }),
      publish: ctx.future_action(|(root, id, suite): (PathBuf, ResourceId, TestSuite)| async move {
        background::run(move || {
          publish_suite_in(&root, id, &suite)?;
          load_suite_in(&root, Some(id)).map(Read)
        })
        .await?
      }),
      again: ctx.signal(false),
      refused: ctx.signal(false),
      deleting: ctx.signal(false),
      gate: ctx.use_context::<InputGate>(),
    };
    editor.fields.watch_automation(ctx, editor.gate.as_ref());
    editor.follow(ctx, &props);
    editor.watch_selects(ctx, &props);
    editor
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<SuiteProps>().clone();
    let root = props.tab.project.open.repo_root.clone();
    let read = ctx
      .future((root.clone(), props.id, self.retry.get()), |(root, id, _)| async move {
        background::run(move || load_suite_in(&root, id).map(Read)).await?
      })
      .state()
      .get();
    if self.stored.get_untracked().is_none()
      && let Some(read) = &read.data
    {
      self.fill(read);
    }
    let Some(Read(stored)) = self.stored.get() else {
      return self.pending(ctx, &read);
    };
    if self.again.get_untracked() && !self.save.state().get_untracked().is_pending() {
      self.again.set(false);
      self.save_now(&root);
    }
    self.fields.follow();
    let view = self.view(stored);
    self.status(&props.tab, &view);
    let commit = self.commit(root.clone());
    let fields = &self.fields;
    let pane = Column::new()
      .id("settings-suite-editor")
      .width(Dimension::Px(PANE_WIDTH))
      .flex_shrink(0.0)
      .spacing(ExtraSpacing::Roomy.role())
      .child(crumb(ctx, &props, &view, &fields.name.get()))
      .child(head(ctx, &view))
      .child(run::run_group(ctx, fields, (&view, &root), &commit))
      .child(report::report_group(ctx, fields, &view, &commit))
      .child(report::needs_group(ctx, fields, &view, &commit))
      .child(limits::artifacts_group(ctx, fields, &view, &commit))
      .child(limits::limits_group(ctx, fields, &view, &commit))
      .child(self.actions(ctx, &props, &view));
    let aside = Column::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .padding(Padding::new().top(Dimension::Px(ASIDE_TOP)))
      .spacing(ExtraSpacing::Tabs.role())
      .child(aside::last_runs_card(ctx))
      .with_children(aside::tools_card(ctx, &view, &fields.name.get()));
    let deleting = view.stored.entry.as_ref().filter(|_| self.deleting.get()).map(|entry| {
      dialog(
        ctx,
        &props.tab,
        (Deleted::SUITE, entry.id, entry.name.clone()),
        &self.deleting,
      )
    });
    let page = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Xl)
      .align_items(Alignment::Start)
      .child(pane)
      .child(aside)
      .with_children(deleting);
    Element::from(column_scroll_wide(page, PAGE_WIDTH))
  }
}

impl SuiteEditor {
  /// After a save or a publish: what it read back is stored, the page's status says so, and the project's other
  /// screens read again; a new suite's page opens under its id, once a save asked for meanwhile landed too.
  fn follow(&self, ctx: &mut Ctx, props: &SuiteProps) {
    let navigation = Navigation::of(ctx);
    let project = props.tab.project.open.project.id();
    let (stored, status, feed, new, again) = (
      self.stored.clone(),
      props.tab.status.clone(),
      props.tab.project.feed.clone(),
      props.id.is_none(),
      self.again.clone(),
    );
    ctx.watch(&self.save.state(), move |state| {
      if state.is_pending() {
        status.set(Status::Saving);
        return;
      }
      if state.is_rejected() {
        again.set(false);
        status.set(Status::Failed);
        return;
      }
      let Some(read) = state.data.clone().filter(|_| state.is_fulfilled()) else {
        return;
      };
      let id = read.0.entry.as_ref().map(|entry| entry.id);
      stored.set(Some(read));
      status.set(Status::Saved);
      feed.saved();
      if again.get_untracked() {
        return;
      }
      if let (true, Some(id)) = (new, id) {
        navigation.replace(Route::Project(project, ProjectPage::Services(ServicesPage::Suite(id))));
      }
    });
    let (stored, feed) = (self.stored.clone(), props.tab.project.feed.clone());
    ctx.watch(&self.publish.state(), move |state| {
      if let Some(read) = state.data.clone().filter(|_| state.is_fulfilled()) {
        stored.set(Some(read));
        feed.saved();
      }
    });
  }

  /// A service chosen in *Add a service* is needed after the others, and a stack chosen is the one it needs; both save.
  fn watch_selects(&self, ctx: &mut Ctx, props: &SuiteProps) {
    let commit = self.commit(props.tab.project.open.repo_root.clone());
    let fields = self.fields.clone();
    ctx.watch(&self.fields.adding, move |chosen: &String| {
      if chosen.is_empty() {
        return;
      }
      let chosen = chosen.clone();
      fields.adding.set(String::new());
      fields.choose(|choices| {
        if !choices.services.contains(&chosen) {
          choices.services.push(chosen);
        }
      });
      commit();
    });
    let (commit, stored) = (
      self.commit(props.tab.project.open.repo_root.clone()),
      self.stored.clone(),
    );
    ctx.watch(&self.fields.stack, move |_: &String| {
      // Filling the page from what is stored sets the stack too; only a choice made after saves.
      if stored.get_untracked().is_some() {
        commit();
      }
    });
  }

  fn fill(&self, read: &Read) {
    self.fields.fill(&shown(&read.0));
    self.stored.set(Some(read.clone()));
  }

  fn view(&self, stored: SuiteRead) -> View {
    let form = self.fields.form();
    let base = stored.entry.as_ref().map(|entry| &entry.body);
    let built = form.suite(base);
    let changed = match &built {
      Ok(suite) => base != Some(suite),
      Err(_) => form != shown(&stored),
    };
    View { stored, built, changed }
  }

  /// The page's save status, unless a save is under way or failed and says its own.
  fn status(&self, props: &TabProps, view: &View) {
    let state = self.save.state().get();
    if state.is_pending() || (state.is_rejected() && view.changed) {
      return;
    }
    props.status.set(match (&view.built, view.changed) {
      (_, false) => Status::Saved,
      (Err(_), true) => Status::Invalid,
      (Ok(_), true) => Status::Unsaved,
    });
  }

  fn commit(&self, root: PathBuf) -> Commit {
    let editor = self.clone();
    Arc::new(move || editor.save_now(&root))
  }

  /// Saves the fields as they are now. What an MCP client changed of what the suite runs, while only a person decides
  /// that, is put back first to what is stored, whoever's input saves; a save asked for while one is under way follows
  /// it.
  fn save_now(&self, root: &std::path::Path) {
    let Some(Read(stored)) = self.stored.get_untracked() else {
      return;
    };
    let put_back = self.fields.put_back(&shown(&stored));
    if put_back {
      tracing::warn!("synthetic input changed what a test suite runs; put back");
      self.refused.set(true);
    }
    if self.save.state().get_untracked().is_pending() {
      self.again.set(true);
      return;
    }
    let base = stored.entry.as_ref().map(|entry| &entry.body);
    let Ok(suite) = self.fields.form().suite(base) else {
      return;
    };
    if base == Some(&suite) {
      return;
    }
    if base.is_none() && !input_may_decide(self.gate.as_ref(), Guarded::PermissionWidening) {
      tracing::warn!("synthetic input asked to make a test suite; not saved");
      self.refused.set(true);
      return;
    }
    if !put_back {
      self.refused.set(false);
    }
    let id = stored.entry.as_ref().map(|entry| entry.id);
    let name = self.fields.name.get_untracked();
    self.save.run((root.to_path_buf(), (id, name), suite, stored.revision));
  }

  /// Before the suite was ever read: why reading failed with Try again, or that it is being read.
  fn pending(&self, ctx: &mut Ctx, read: &FutureState<Read, Message>) -> Element {
    let shown = match &read.error {
      Some(error) => {
        let retry = self.retry.clone();
        load_error(
          ctx,
          "settings-suite-editor-load-error",
          (
            &ctx.t("project_settings.services.suite.loadFailed"),
            &message_text(ctx, error),
          ),
          move || retry.update(|count| *count += 1),
        )
      }
      None => loading(
        "settings-suite-editor-loading",
        &ctx.t("project_settings.services.suite.reading"),
      )
      .into(),
    };
    Element::from(column_scroll_wide(shown, PAGE_WIDTH))
  }

  /// The pane's last line (`Actions` `f3ZKT`): *Delete suite* at its start, and at its end where the suite stands with
  /// *Publish vN* while there is a draft to publish; under it that an MCP client's input was not taken and why a save
  /// or a publish failed.
  fn actions(&self, ctx: &mut Ctx, props: &SuiteProps, view: &View) -> Element {
    let delete = view
      .stored
      .entry
      .as_ref()
      .map(|_| Deleted::SUITE.button(ctx, &self.deleting));
    let stored = view.stored.entry.as_ref().map(|entry| entry.state);
    let line = Row::new()
      .id("settings-suite-actions")
      .width(Dimension::Pct(100.0))
      .padding(Padding::new().top(SpacingSize::Xs))
      .spacing(ExtraSpacing::Loose.role())
      .align_items(Alignment::Center)
      .with_children(delete)
      .child(Spacer::new().flex(1.0))
      .child(
        Text::new(&state_words(ctx, stored))
          .id("settings-suite-state")
          .variant(ExtraTypography::Note.role()),
      )
      .with_children(self.publish_button(ctx, props, view));
    let mut said = Vec::new();
    if self.refused.get() {
      said.push(Element::from(field_error(
        "settings-suite-person-only",
        &ctx.t("project_settings.services.suite.personOnly"),
      )));
    }
    for (id, state) in [
      ("settings-suite-save-error", self.save.state().get()),
      ("settings-suite-publish-error", self.publish.state().get()),
    ] {
      if let Some(error) = state.error.as_ref().filter(|_| state.is_rejected()) {
        said.push(Element::from(field_error(id, &message_text(ctx, error))));
      }
    }
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .child(line)
      .with_children(said)
      .into()
  }

  /// *Publish vN*: it waits, and says why, for a draft from an older version, a mistake and a change not saved yet; an
  /// MCP client's press publishes only under *Let automation change agent permissions*.
  fn publish_button(&self, ctx: &Ctx, props: &SuiteProps, view: &View) -> Option<Element> {
    let entry = view.stored.entry.as_ref()?;
    let next = next_version(entry.state, view.changed)?;
    let button = ActionButton::new(
      "settings-suite-publish",
      ButtonKind::Primary,
      ctx.t_args(
        "project_settings.services.editor.publish",
        [("version", next.get().to_string())],
      ),
    )
    .icon(Icon::Upload);
    let waits = stale_reason(ctx, entry.state).or_else(|| {
      if view.built.is_err() {
        Some(ctx.t("project_settings.services.editor.publishWaitsFix"))
      } else if view.changed || self.save.state().get().is_pending() {
        Some(ctx.t("project_settings.services.editor.publishWaitsSave"))
      } else {
        None
      }
    });
    let button = match (waits, &view.built) {
      (Some(reason), _) => button.unavailable(reason),
      (None, _) if self.publish.state().get().is_pending() => button.loading(),
      (None, Ok(suite)) => {
        let (editor, root, id, suite) = (
          self.clone(),
          props.tab.project.open.repo_root.clone(),
          entry.id,
          suite.clone(),
        );
        button.on_click(move || editor.publish_now(root.clone(), id, suite.clone()))
      }
      (None, Err(_)) => button.disabled(),
    };
    Some(button.into())
  }

  fn publish_now(&self, root: PathBuf, id: ResourceId, suite: TestSuite) {
    if self.publish.state().get_untracked().is_pending() {
      return;
    }
    if !input_may_decide(self.gate.as_ref(), Guarded::PermissionWidening) {
      tracing::warn!("synthetic input asked to publish a test suite; not published");
      self.refused.set(true);
      return;
    }
    self.refused.set(false);
    self.publish.run((root, id, suite));
  }
}

/// The crumb back to Services through Test suites, with the suite's name or *New suite* after it (`Crumb` `ARZRG`).
fn crumb(ctx: &mut Ctx, props: &SuiteProps, view: &View, typed: &str) -> Element {
  let here = match &view.stored.entry {
    Some(entry) => entry.name.to_string(),
    None if !typed.trim().is_empty() => typed.trim().to_owned(),
    None => ctx.t("project_settings.services.newSuite").to_string(),
  };
  back_through(
    "settings-suite-editor-crumb",
    (
      &ctx.t("project_settings.services.tab"),
      &ctx.t("project_settings.services.suites"),
      &here,
    ),
    Navigation::of(ctx).to(Route::Project(
      props.tab.project.open.project.id(),
      ProjectPage::Settings(ProjectSettingsTab::Services),
    )),
  )
}

/// The suite's name over when the Test step runs it (`Head` `kd56b`).
fn head(ctx: &Ctx, view: &View) -> Element {
  let (title, lead) = match &view.stored.entry {
    Some(entry) => (entry.name.to_string(), ctx.t("project_settings.services.suite.lead")),
    None => (
      ctx.t("project_settings.services.newSuite").to_string(),
      ctx.t("project_settings.services.suite.newLead"),
    ),
  };
  page_head("settings-suite-editor-head", &title, Some(&lead))
}
