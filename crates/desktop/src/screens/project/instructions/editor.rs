//! Instruction editor (`LIOyl` in `yRKeF`) with the page around it: the rule's name and state,
//! where it applies, whom it reaches and when, its prompt, and the preview of the prompt it takes
//! part in (`RGk5C`). Edits are saved as the draft as they are made; Publish vN publishes the
//! saved draft.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::TypographyStyle,
  },
  components::{Column, Row, Text},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::ResourceId,
  resource::{DraftSaved, InjectionEvent, Instruction},
  run::InstructionVersion,
  time::Timestamp,
};
use parking_lot::Mutex;

use super::{
  EditorContext, copy,
  frame::{Frame, frame},
  library::{Library, library},
  variables::CatalogView,
};
use crate::{
  activity::{OperationSpec, operations},
  agents::data::VersionState,
  background,
  drafts::{DraftQueue, SaveAction, SaveJob, save_action},
  instructions::{
    data::{InstructionEntry, entry_in},
    editor::{WriteError, publish_in},
    form::{Events, FormErrors, InstructionForm, Picked, ScopeChoice, Target},
    preview::{EditedInstruction, PreviewChoice},
  },
  messages::Message,
  screens::project::page::TOPBAR_BUTTON_HEIGHT,
  ui::{button::ActionButton, command_row::Copied, icon::Icon, message_text},
};

/// Where the edits stand against what is stored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SaveStatus {
  /// The body of the newest revision handed to a save.
  pub body: Instruction,
  pub revision: u64,
  pub saved_revision: u64,
  pub state: VersionState,
}

impl lurq::app::component::DevtoolsInspectable for SaveStatus {}

impl SaveStatus {
  /// `revision` was stored as the draft, or it repeated the published version and no draft
  /// remains.
  fn saved(&mut self, revision: u64, outcome: DraftSaved) {
    self.saved_revision = self.saved_revision.max(revision);
    let published = self.state.published();
    self.state = match (outcome, published) {
      (DraftSaved::Discarded, Some(version)) => VersionState::Published(version),
      _ if self.state.has_draft() => self.state,
      _ => VersionState::Draft { published },
    };
  }
}

type PublishArgs = (PathBuf, ResourceId, OperationSpec);
type PublishAction = FutureAction<PublishArgs, u64, Message>;

pub struct InstructionEditor {
  pub(super) context: EditorContext,
  pub(super) text: Signal<String>,
  pub(super) targets: Signal<Vec<Target>>,
  /// The targets as last normalized, to tell which choice a change added.
  targets_before: Arc<Mutex<Vec<Target>>>,
  pub(super) events: Signal<Events>,
  pub(super) scope: Signal<ScopeChoice>,
  pub(super) preview: Signal<PreviewChoice>,
  pub(super) preview_event: Signal<Picked<InjectionEvent>>,
  pub(super) viewer_open: Signal<bool>,
  /// The source shown in the resolved prompt, by position.
  pub(super) viewer_source: Signal<usize>,
  pub(super) variables_open: Signal<bool>,
  pub(super) status: Signal<SaveStatus>,
  /// The last prompt copied from the resolved prompt, and whether the clipboard took it.
  pub(super) copied: Signal<Copied>,
  pub(super) catalog_view: Signal<CatalogView>,
  pub(super) catalog_search: Signal<String>,
  queue: DraftQueue<Instruction>,
}

impl Component for InstructionEditor {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    let context = ctx
      .use_context::<EditorContext>()
      .expect("the instructions screen provides the editor context");
    let save = save_action::<Instruction>(ctx);
    let publish = publish_action(ctx);
    let entry = &context.entry;
    let status = ctx.signal(SaveStatus {
      body: entry.body.clone(),
      revision: 0,
      saved_revision: 0,
      state: entry.state,
    });
    let saved = status.clone();
    ctx.watch(&save.state(), move |action| {
      if let Some(done) = action.data.as_ref().filter(|_| !action.is_pending()) {
        saved.update(|status| status.saved(done.revision, done.outcome));
      }
    });
    let reload = context.revision.clone();
    ctx.watch(&publish.state(), move |action| {
      if action.data.is_some() && !action.is_pending() {
        reload.update(|revision| *revision += 1);
      }
    });
    let targets = Target::of(entry.body.targets());
    Self {
      text: ctx.signal(entry.body.text().as_str().to_owned()),
      targets_before: Arc::new(Mutex::new(targets.clone())),
      targets: ctx.signal(targets),
      events: ctx.signal(Events::of(entry.body.events())),
      scope: ctx.signal(ScopeChoice::of(entry.body.scope())),
      preview: ctx.signal(PreviewChoice::default()),
      preview_event: ctx.signal(Picked(InjectionEvent::Start)),
      viewer_open: ctx.signal(false),
      viewer_source: ctx.signal(0),
      variables_open: ctx.signal(false),
      status,
      copied: ctx.signal(Copied::default()),
      catalog_view: ctx.signal(CatalogView::default()),
      catalog_search: ctx.signal(String::new()),
      queue: DraftQueue::default(),
      context,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    // Futures are matched by call order: the actions come first, as in `create`.
    let save = save_action::<Instruction>(ctx);
    let publish = publish_action(ctx);
    let catalog = self.catalog(ctx);
    self.normalize_targets();
    let parsed = self.parse();
    if let Ok(body) = &parsed {
      self.flush(body, &save);
    }
    let status = self.status.get();
    let live = InstructionEntry {
      body: parsed.as_ref().map_or_else(|_| status.body.clone(), Clone::clone),
      state: status.state,
      ..self.context.entry.clone()
    };
    let context = &self.context;
    let list = library(
      ctx,
      &Library {
        project: context.project.project.name().as_str(),
        data: &context.data,
        tab: context.tab,
        edited: Some(&live),
        nav: &context.nav,
        creating: &context.creating,
        scroll: &context.list_scroll,
      },
    );
    let trailing = self.topbar_actions(ctx, &status, parsed.as_ref().err(), &save, &publish);
    let preview = self.preview_panel(ctx, &live, &catalog);
    let body = Row::new()
      .flex(1.0)
      .height(Dimension::Pct(100.0))
      .child(self.rule_editor(ctx, &live, parsed.as_ref().err()))
      .child(preview);
    let page = frame(
      ctx,
      Frame {
        project: &context.project,
        tab: context.tab,
        nav: &context.nav,
        trailing,
        library: Some(list),
        body: body.into(),
      },
    );
    Column::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(page)
      .child(self.viewer_modal(ctx, &live, &catalog))
      .child(self.variables_modal(ctx, &catalog))
  }
}

impl InstructionEditor {
  /// The rule as edited, or why it cannot be saved.
  fn parse(&self) -> Result<Instruction, FormErrors> {
    let (text, targets, scope) = (self.text.get(), self.targets.get(), self.scope.get());
    InstructionForm {
      text: &text,
      targets: &targets,
      events: self.events.get(),
      scope: &scope,
    }
    .instruction()
  }

  /// Keeps "Apply to" to one kind of choice: the newest choice decides (see
  /// [`Target::after_change`]).
  fn normalize_targets(&self) {
    let current = self.targets.get();
    let mut before = self.targets_before.lock();
    if *before == current {
      return;
    }
    let normalized = Target::after_change(&before, current.clone());
    before.clone_from(&normalized);
    drop(before);
    if normalized != current {
      self.targets.set(normalized);
    }
  }

  /// Stores `body` as the draft when it differs from the newest revision handed to a save.
  fn flush(&self, body: &Instruction, save: &SaveAction<Instruction>) {
    let mut status = self.status.get();
    if status.body == *body {
      return;
    }
    status.body = body.clone();
    status.revision += 1;
    let based_on = match status.state {
      VersionState::Draft { published } => published,
      VersionState::StaleDraft { based_on, .. } => based_on,
      VersionState::Published(version) => Some(version),
    };
    let entry = &self.context.entry;
    let job = SaveJob {
      spec: operations::instruction_draft(&self.context.project, entry.id, &entry.name),
      repo_root: self.context.project.repo_root.clone(),
      id: entry.id,
      revision: status.revision,
      draft: orchester_core::resource::Draft::new(body.clone(), based_on, Timestamp::now()),
    };
    self.status.set(status);
    if let Some(job) = self.queue.offer(job) {
      save.run((job, self.queue.clone()));
    }
  }

  /// The edited rule as the preview substitutes it.
  pub(super) fn edited<'a>(&'a self, live: &'a InstructionEntry) -> EditedInstruction<'a> {
    let version = if live.state.has_draft() {
      InstructionVersion::Draft {
        based_on: live.state.published(),
      }
    } else {
      InstructionVersion::Published {
        number: live
          .state
          .published()
          .expect("an instruction without a draft is published"),
      }
    };
    EditedInstruction {
      id: live.id,
      name: &live.name,
      slug: &live.slug,
      body: &live.body,
      version,
    }
  }

  /// The save state and Publish vN (`RErO3`, `zJqtY`).
  fn topbar_actions(
    &self,
    ctx: &Ctx,
    status: &SaveStatus,
    errors: Option<&FormErrors>,
    save: &SaveAction<Instruction>,
    publish: &PublishAction,
  ) -> Vec<Element> {
    let saving = save.state().get();
    let publishing = publish.state().get();
    let pending = saving.is_pending() || status.saved_revision < status.revision;
    let error = saving
      .error
      .clone()
      .filter(|_| !saving.is_pending())
      .or(publishing.error.clone().filter(|_| !publishing.is_pending()));
    let text = match (errors, error) {
      (Some(_), _) => ctx.t("instructions.status.notSaved"),
      (None, Some(error)) => message_text(ctx, &error),
      (None, None) if pending => ctx.t("instructions.status.saving"),
      (None, None) => save_status(ctx, status.state),
    };
    let next = copy::draft_number(status.state);
    let button = ActionButton::new(ctx.t_args("instructions.publish", [("version", next.to_string().as_str())]))
      .primary()
      .icon(Icon::Check)
      .id("instruction-publish")
      .height(Dimension::Px(TOPBAR_BUTTON_HEIGHT));
    let publishable = matches!(status.state, VersionState::Draft { .. })
      && errors.is_none()
      && !pending
      && !publishing.is_pending()
      && !status.body.text().is_blank();
    let button = if publishable {
      let (action, repo_root, entry) = (
        publish.clone(),
        self.context.project.repo_root.clone(),
        self.context.entry.clone(),
      );
      let spec = operations::instruction_publish(&self.context.project, entry.id, &entry.name, next);
      button.on_click(move || action.run((repo_root.clone(), entry.id, spec.clone())))
    } else {
      button.on_click(|| {}).enabled(false)
    };
    vec![
      Text::new(&text)
        .variant(TypographyStyle::Caption)
        .id("instruction-save-status")
        .into(),
      button.render(ctx),
    ]
  }
}

/// "Draft saved · Published v2", "Draft saved · Not published", "Published v2", …
fn save_status(ctx: &Ctx, state: VersionState) -> Arc<str> {
  let version = |version: orchester_core::resource::VersionNumber| version.get().to_string();
  match state {
    VersionState::Published(published) => ctx.t_args(
      "instructions.status.published",
      [("version", version(published).as_str())],
    ),
    VersionState::Draft {
      published: Some(published),
    } => ctx.t_args(
      "instructions.status.draftSaved",
      [("version", version(published).as_str())],
    ),
    VersionState::Draft { published: None } => ctx.t("instructions.status.draftNeverPublished"),
    VersionState::StaleDraft { based_on, published } => ctx.t_args(
      "instructions.status.staleDraft",
      [
        ("basedOn", based_on.map(version).unwrap_or_default().as_str()),
        ("version", published.map(version).unwrap_or_default().as_str()),
      ],
    ),
  }
}

fn publish_action(ctx: &mut Ctx) -> PublishAction {
  let activity = crate::activity::of(ctx);
  ctx.future_action(move |(repo_root, id, spec): PublishArgs| {
    let activity = activity.clone();
    async move {
      let publish = async move {
        let published = background::run(move || publish_in(&repo_root, &entry_in(&repo_root, id)?)).await?;
        published
          .map(|entry| {
            let number = entry.state.published().expect("a published instruction has a version");
            u64::from(number.get())
          })
          .map_err(|error: WriteError| {
            tracing::error!(%error, "cannot publish the instruction");
            error.message()
          })
      };
      activity.track(spec, publish).await
    }
  })
}
