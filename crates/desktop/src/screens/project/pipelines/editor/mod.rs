//! Pipeline editor (03.01 `Q21HOA`): the step graph with the selected step's inspector, or the
//! pipeline's own inspector when no step is selected (03.06 `angoF`). Edits are saved as the
//! draft automatically while the draft rules allow it; Publish opens a confirmation (03.02
//! `zfjsK`) and publishes the stored draft (03.03 `yf6XH`). What blocks publishing is listed
//! under the toolbar (03.B `UjHDL`). Version history shows a published version read-only on
//! the canvas beside the list of versions ([`history_panel`]), where it can be compared with the
//! draft or restored as the draft. Running a pipeline does not exist yet, so Run is unavailable.

mod autosave;
mod canvas;
mod header;
mod history_panel;
mod pipeline_inspector;
mod publish_dialog;
mod step_card;
mod step_fields;
mod step_inspector;
mod step_permissions;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, SpacingSize},
  },
  components::{Column, Modal, ModalTarget, Row},
  core::Signal,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  id::ResourceId,
  resource::{Pipeline, PipelineEdit, StepKey, VersionNumber},
  time::Timestamp,
};
use orchester_store::StoreError;

use self::{
  autosave::watch_saves,
  canvas::{Canvas, CanvasProps},
  history_panel::{Current, HistoryPanel, HistoryPanelProps, HistoryState},
  pipeline_inspector::{PipelineInspector, PipelineInspectorProps},
  publish_dialog::publish_dialog,
  step_inspector::{StepInspector, StepInspectorProps},
};
use super::PipelinesNav;
use crate::{
  activity::{Activity, OperationSpec, operations},
  background,
  drafts::{DraftQueue, SaveAction, SaveJob, save_action},
  messages::{self, Message},
  navigation::route::PipelineView,
  pipelines::{
    data::{OpenError, OpenedPipeline, publish_in},
    editor::EditorState,
    history::{PublishedVersion, VersionHistory},
    problems::publish_error,
  },
  screens::project::ProjectNav,
  theme::ExtraColor,
  ui::{backdrop::backdrop, form::form, icon::Icon, message_text, panel::RecoveryPanel},
  workspace::OpenProject,
};

/// Inspector width (`yCO8P`).
const INSPECTOR_WIDTH: f32 = 304.0;

#[derive(Clone, Debug, PartialEq)]
pub struct EditorProps {
  pub project: OpenProject,
  pub id: ResourceId,
  pub view: PipelineView,
}

impl lurq::app::component::DevtoolsInspectable for EditorProps {}

/// What the inspector shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selection {
  Pipeline,
  Step(StepKey),
}

impl lurq::app::component::DevtoolsInspectable for Selection {}

/// The version a publish created, and what it contains.
#[derive(Clone, Debug, PartialEq, Eq)]
struct PublishedDraft {
  number: VersionNumber,
  at: Timestamp,
  body: Pipeline,
}

impl lurq::app::component::DevtoolsInspectable for PublishedDraft {}

type PublishArgs = (PathBuf, ResourceId, Pipeline, OperationSpec);
type PublishAction = FutureAction<PublishArgs, PublishedDraft, Message>;

struct Actions {
  save: SaveAction<Pipeline>,
  publish: PublishAction,
}

/// The open project, as a signal carries it; `None` until the first render.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Project(Option<OpenProject>);

impl lurq::app::component::DevtoolsInspectable for Project {}

/// The pipeline as it was read, as the load future carries it.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Loaded(Arc<OpenedPipeline>);

impl lurq::app::component::DevtoolsInspectable for Loaded {}

/// The published versions as the history future carries them.
#[derive(Clone, Debug, PartialEq, Eq)]
struct LoadedHistory(Arc<VersionHistory>);

impl lurq::app::component::DevtoolsInspectable for LoadedHistory {}

/// Provided by the editor: its state and the edits children make to it.
#[derive(Clone)]
pub struct EditorHandle {
  state: Signal<Option<EditorState>>,
  selection: Signal<Selection>,
  /// Changes when steps are replaced as a whole, so step inspectors start again from them.
  epoch: Signal<u64>,
  save: SaveAction<Pipeline>,
  queue: DraftQueue<Pipeline>,
  /// Set by the editor's render: props are not available when the handle is created.
  project: Signal<Project>,
}

impl EditorHandle {
  /// Applies `change` to the steps and saves the draft when it can be saved.
  pub fn edit<R>(&self, change: impl FnOnce(&mut PipelineEdit) -> R) -> Option<R> {
    let mut state = self.state.get();
    let result = state.as_mut().map(|state| state.apply(change));
    self.state.set(state);
    self.flush();
    result
  }

  /// Marks a field of `step` as holding text that cannot be used, or no longer.
  pub fn set_field_invalid(&self, step: &StepKey, field: &'static str, invalid: bool) {
    self.state.update(|state| {
      if let Some(state) = state {
        state.set_field_invalid(step, field, invalid);
      }
    });
    self.flush();
  }

  pub fn select(&self, selection: Selection) {
    if let Selection::Step(previous) = self.selection.get() {
      self.state.update(|state| {
        if let Some(state) = state {
          state.clear_invalid_fields(&previous);
        }
      });
    }
    self.selection.set(selection);
    self.flush();
  }

  /// Replaces every step, e.g. with the starter's; open inspectors start again.
  pub fn replace(&self, change: impl FnOnce(&mut PipelineEdit)) {
    self.edit(change);
    self.epoch.update(|epoch| *epoch += 1);
  }

  /// Replaces the steps with those of a published version, `body`, which autosave stores as the
  /// draft (see [`EditorState::restore`]); the first restored step is selected.
  pub fn restore(&self, body: &Pipeline) {
    self.state.update(|state| {
      if let Some(state) = state {
        state.restore(body);
      }
    });
    let first = body.steps().first().map(|step| step.key.clone());
    self.selection.set(first.map_or(Selection::Pipeline, Selection::Step));
    self.flush();
    self.epoch.update(|epoch| *epoch += 1);
  }

  /// Stores the current revision as the draft, unless there is nothing that can be saved; while
  /// a save runs, the revision waits for it (see [`autosave`]).
  fn flush(&self) {
    let Some(state) = self.state.get() else {
      return;
    };
    let Project(Some(project)) = self.project.get() else {
      return;
    };
    if let Some((revision, draft)) = state.draft_to_save(Timestamp::now()) {
      let job = SaveJob {
        spec: operations::pipeline_draft(&project, state.id(), state.name()),
        repo_root: project.repo_root,
        id: state.id(),
        revision,
        draft,
      };
      if let Some(job) = self.queue.offer(job) {
        self.save.run((job, self.queue.clone()));
      }
    }
  }
}

pub struct PipelineEditor {
  handle: EditorHandle,
  confirm_publish: Signal<bool>,
}

impl Component for PipelineEditor {
  type Props = EditorProps;

  fn create(ctx: &mut Ctx) -> Self {
    let save = save_action(ctx);
    let publish = publish_action(ctx);
    let handle = EditorHandle {
      state: ctx.signal(None),
      selection: ctx.signal(Selection::Pipeline),
      epoch: ctx.signal(0),
      save: save.clone(),
      queue: DraftQueue::default(),
      project: ctx.signal(Project::default()),
    };
    let confirm_publish = ctx.signal(false);
    watch_saves(ctx, &save, &handle);
    watch_publishes(ctx, &publish, &handle, &confirm_publish);
    Self {
      handle,
      confirm_publish,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<EditorProps>().clone();
    let save = save_action(ctx);
    let publish = publish_action(ctx);
    let handle = self.handle.clone();
    if handle.project.get().0.as_ref() != Some(&props.project) {
      handle.project.set(Project(Some(props.project.clone())));
    }
    ctx.provide(handle.clone());
    let deps = (props.project.repo_root.clone(), props.id);
    let load = ctx.future(deps, |(repo_root, id)| async move {
      let opened = background::run(move || OpenedPipeline::load_in(&repo_root, id)).await?;
      opened.map(|opened| Loaded(Arc::new(opened))).map_err(|error| {
        tracing::error!(%error, "cannot open the pipeline");
        match error {
          OpenError::Missing => Message::new("pipelines.editor.missing"),
          OpenError::Store(error) => messages::store_error(&error),
        }
      })
    });
    let history = history_state(ctx, &props, &handle);
    let loaded = load.state().get();
    if let Some(error) = loaded.error {
      return failed(ctx, &error);
    }
    let Some(Loaded(opened)) = loaded.data else {
      return Column::new().into();
    };
    let state = match handle.state.get() {
      Some(state) => state,
      None => {
        let state = EditorState::new(opened);
        let first = state.edit().steps().first().map(|step| step.key.clone());
        handle.selection.set(first.map_or(Selection::Pipeline, Selection::Step));
        handle.state.set(Some(state.clone()));
        state
      }
    };
    self.editor(ctx, &props, &state, &Actions { save, publish }, &history)
  }
}

impl PipelineEditor {
  fn editor(
    &self,
    ctx: &mut Ctx,
    props: &EditorProps,
    state: &EditorState,
    actions: &Actions,
    history: &HistoryState,
  ) -> Element {
    let Actions { save, publish } = actions;
    let handle = &self.handle;
    let nav = ctx
      .use_context::<PipelinesNav>()
      .expect("the pipelines screen provides PipelinesNav");
    let saving = save.state().get();
    let saving = header::Saving {
      pending: saving.is_pending(),
      error: saving.error.clone().filter(|_| !saving.is_pending()),
    };
    let project = props.project.project.name().as_str();
    let shown = shown_version(props, history, &nav);
    let confirm = self.confirm_publish.clone();
    let mut column = Column::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(header::topbar(ctx, project, state, &nav, &saving, move || {
        confirm.set(true)
      }));
    let (canvas, panel) = match props.view.version() {
      Some(number) => {
        column = column.child(header::version_toolbar(ctx, state, &nav, number));
        let canvas = shown.as_ref().map_or_else(
          || CanvasProps::draft(state, &Selection::Pipeline),
          |version| CanvasProps::version(state, &version.body),
        );
        (canvas, history_panel(ctx, props, state, history))
      }
      None => {
        column = column.child(header::toolbar(ctx, state, handle, &nav));
        if let Some(strip) = header::problems_strip(ctx, state, handle) {
          column = column.child(strip);
        }
        let selection = handle.selection.get();
        (
          CanvasProps::draft(state, &selection),
          inspector(ctx, handle, state, &selection),
        )
      }
    };
    let canvas = ctx.mount_keyed::<Canvas>("pipeline-canvas", canvas);
    let side_panel = Column::new()
      .width(Dimension::Px(INSPECTOR_WIDTH))
      .height(Dimension::Pct(100.0))
      .background(ExtraColor::Surface.role())
      .border_left(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(panel);
    let column = column
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .flex(1.0)
          .child(Column::new().flex(1.0).height(Dimension::Pct(100.0)).child(canvas))
          .child(side_panel),
      )
      .child(self.publish_modal(ctx, state, publish, &props.project));
    // One form for the whole editor; only the inspector's fields and buttons take focus.
    // Edits are saved as the draft as they are made: there is nothing for Enter to submit.
    form(None, column)
  }

  fn publish_modal(&self, ctx: &Ctx, state: &EditorState, publish: &PublishAction, project: &OpenProject) -> Element {
    if !self.confirm_publish.get() {
      return Column::new().into();
    }
    let action = publish.state().get();
    let run = {
      let (publish, repo_root, id) = (publish.clone(), project.repo_root.clone(), state.id());
      let spec = operations::pipeline_publish(project, id, state.name(), state.draft_number());
      let body = state.edit().validate().ok();
      move || {
        if let Some(body) = &body {
          publish.run((repo_root.clone(), id, body.clone(), spec.clone()));
        }
      }
    };
    let dialog = publish_dialog(
      ctx,
      state,
      &self.confirm_publish,
      run,
      action.is_pending(),
      action.error.clone().filter(|_| !action.is_pending()),
    );
    Modal::new(backdrop(dialog))
      .target(ModalTarget::Root)
      .open(self.confirm_publish.clone())
      .into()
  }
}

/// The selected step's inspector, or the pipeline's when no step is selected.
fn inspector(ctx: &mut Ctx, handle: &EditorHandle, state: &EditorState, selection: &Selection) -> Element {
  let selected_step = match selection {
    Selection::Step(key) => state.edit().step(key).cloned(),
    Selection::Pipeline => None,
  };
  match selected_step {
    Some(step) => {
      let key = format!("step-inspector-{}-{}", step.key, handle.epoch.get());
      ctx.mount_keyed::<StepInspector>(&key, StepInspectorProps::new(state, step))
    }
    None => ctx.mount_keyed::<PipelineInspector>(
      &format!("pipeline-inspector-{}", handle.epoch.get()),
      PipelineInspectorProps::new(state),
    ),
  }
}

fn history_panel(ctx: &mut Ctx, props: &EditorProps, state: &EditorState, history: &HistoryState) -> Element {
  let current = Current {
    draft: state.draft_number(),
    has_changes: state.has_changes(),
    body: state.edit().validate().ok().map(Arc::new),
  };
  ctx.mount_keyed::<HistoryPanel>(
    "pipeline-history-panel",
    HistoryPanelProps {
      pipeline: props.id,
      view: props.view,
      history: history.clone(),
      current,
    },
  )
}

/// The published versions, read again whenever a new one is published.
fn history_state(ctx: &mut Ctx, props: &EditorProps, handle: &EditorHandle) -> HistoryState {
  let latest = handle
    .state
    .get()
    .and_then(|state| state.latest().map(|latest| latest.number));
  let deps = (props.project.repo_root.clone(), props.id, latest);
  let history = ctx.future(deps, |(repo_root, id, _)| async move {
    let history = background::run(move || VersionHistory::load_in(&repo_root, id)).await?;
    history
      .map(|history| LoadedHistory(Arc::new(history)))
      .map_err(|error: StoreError| {
        tracing::error!(%error, "cannot read the pipeline's versions");
        messages::store_error(&error)
      })
  });
  let history = history.state().get();
  match (history.data, history.error) {
    (_, Some(error)) => HistoryState::Failed(error),
    (Some(LoadedHistory(history)), None) => HistoryState::Ready(history),
    (None, None) => HistoryState::Loading,
  }
}

/// The published version the view shows, once the versions are read. A version the pipeline
/// does not have leads back to the draft in place of the entry that named it.
fn shown_version(props: &EditorProps, history: &HistoryState, nav: &PipelinesNav) -> Option<PublishedVersion> {
  let number = props.view.version()?;
  let HistoryState::Ready(history) = history else {
    return None;
  };
  let found = history.version(number).cloned();
  if found.is_none() {
    nav.replace(props.id, PipelineView::Draft);
  }
  found
}

/// The starter's steps with the project's profiles and tools, for "Use starter steps".
fn starter_edit(ctx: &Ctx, handle: &EditorHandle) -> PipelineEdit {
  let assignables = handle
    .state
    .get()
    .map(|state| state.assignables().clone())
    .unwrap_or_default();
  PipelineEdit::new(super::starter_text(ctx).pipeline(&assignables))
}

fn failed(ctx: &Ctx, error: &Message) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .padding(SpacingSize::Section)
    .child(RecoveryPanel::new(
      Icon::AlertCircle,
      &ctx.t("pipelines.editor.openFailed"),
      &message_text(ctx, error),
    ))
    .into()
}

fn publish_action(ctx: &mut Ctx) -> PublishAction {
  let activity = Activity::of(ctx);
  ctx.future_action(move |(repo_root, id, body, spec): PublishArgs| {
    let activity = activity.clone();
    async move {
      let publish = async move {
        let published = background::run(move || publish_in(&repo_root, id)).await?;
        published
          .map(|(number, at)| PublishedDraft { number, at, body })
          .map_err(|error| {
            tracing::error!(%error, "cannot publish the pipeline");
            publish_error(&error)
          })
      };
      activity.track(spec, publish).await
    }
  })
}

fn watch_publishes(ctx: &mut Ctx, publish: &PublishAction, handle: &EditorHandle, confirm: &Signal<bool>) {
  let (state, confirm) = (handle.state.clone(), confirm.clone());
  let pipelines = ctx
    .use_context::<PipelinesNav>()
    .expect("the pipelines screen provides PipelinesNav");
  let project = ctx
    .use_context::<ProjectNav>()
    .expect("the project shell provides ProjectNav");
  ctx.watch(&publish.state(), move |action| {
    let Some(published) = action.data.clone().filter(|_| !action.is_pending()) else {
      return;
    };
    state.update(|editor| {
      if let Some(editor) = editor
        && editor.latest().is_none_or(|latest| latest.number < published.number)
      {
        editor.published(published.number, published.at, published.body.clone());
      }
    });
    confirm.set(false);
    pipelines.changed();
    project.tasks_changed();
  });
}
