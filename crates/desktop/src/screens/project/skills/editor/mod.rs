//! Skill editor (`ecJtg`, built on 05.01): the list of skills by scope tab, the skill's name,
//! scope, description and body, and its reference files with where it came from. Edits are saved
//! as the draft as they are made; Publish vN publishes the saved draft.

mod fields;
mod files;
mod list;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::ResourceId,
  resource::{Draft, DraftSaved, InstructionScope, Skill, SkillError, SkillFile},
  time::Timestamp,
};

use self::files::add_file_action;
use super::{EditorContext, library::ScopeFilter};
use crate::{
  activity::{Activity, OperationSpec, skill_operations},
  agents::data::VersionState,
  background,
  drafts::{DraftQueue, SaveAction, SaveJob, save_action},
  instructions::form::Picked,
  messages::{self, Message},
  screens::project::{instructions::copy::draft_number, page},
  skills::data::publish_in,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    icon::Icon,
    message_text,
    tab::{Tab, tab_row},
  },
};

/// Scope tab row height (`qpNOl`).
const TABS_HEIGHT: f32 = 42.0;

/// Where the edits stand against what is stored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SaveStatus {
  /// The body of the newest revision handed to a save.
  pub body: Skill,
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

/// The skill's reference files as edited.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Files(pub Vec<SkillFile>);

impl lurq::app::component::DevtoolsInspectable for Files {}

type PublishArgs = (PathBuf, ResourceId, OperationSpec);
type PublishAction = FutureAction<PublishArgs, u64, Message>;

pub struct SkillEditor {
  pub(super) context: EditorContext,
  pub(super) description: Signal<String>,
  pub(super) body: Signal<String>,
  pub(super) scope: Signal<Picked<InstructionScope>>,
  pub(super) files: Signal<Files>,
  /// The list's scope tab.
  pub(super) tab: Signal<ScopeFilter>,
  pub(super) status: Signal<SaveStatus>,
  queue: DraftQueue<Skill>,
}

impl Component for SkillEditor {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    let context = ctx
      .use_context::<EditorContext>()
      .expect("the skills screen provides the editor context");
    let save = save_action::<Skill>(ctx);
    let publish = publish_action(ctx);
    let add = add_file_action(ctx);
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
    let files = ctx.signal(Files(entry.body.files().to_vec()));
    let added = files.clone();
    ctx.watch(&add.state(), move |action| {
      if let Some(Some(file)) = action.data.as_ref().filter(|_| !action.is_pending()) {
        let file = file.0.clone();
        added.update(|Files(files)| files.push(file));
      }
    });
    Self {
      description: ctx.signal(entry.body.description().to_owned()),
      body: ctx.signal(entry.body.body().to_owned()),
      scope: ctx.signal(Picked(entry.body.scope().clone())),
      files,
      tab: ctx.signal(ScopeFilter::All),
      status,
      queue: DraftQueue::default(),
      context,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    // Futures are matched by call order: the actions come first, as in `create`.
    let save = save_action::<Skill>(ctx);
    let publish = publish_action(ctx);
    let add = add_file_action(ctx);
    let parsed = self.parse();
    if let Ok(body) = &parsed {
      self.flush(body, &save);
    }
    let status = self.status.get();
    let live = parsed.as_ref().map_or_else(|_| status.body.clone(), Clone::clone);
    let trailing = self.topbar_actions(ctx, &status, parsed.as_ref().err(), (&save, &publish));
    let body = Row::new()
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .child(self.list(ctx, &live, status.state))
      .child(self.skill_editor(ctx, &live, status.state, parsed.as_ref().err()))
      .child(self.details(ctx, &live, &add));
    let project = self.context.project.project.name().as_str();
    Column::new()
      .id("skill-editor-screen")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .background(ExtraColor::Surface.role())
      .child(page::topbar(
        &ctx.t_args(
          "skills.editorBreadcrumb",
          [("project", project), ("skill", &*self.context.entry.name)],
        ),
        trailing,
      ))
      .child(heading(ctx))
      .child(self.tabs(ctx))
      .child(body)
  }
}

impl SkillEditor {
  /// The skill as edited, or why it cannot be saved.
  fn parse(&self) -> Result<Skill, SkillError> {
    Skill::new(
      self.description.get(),
      self.body.get(),
      self.files.get().0,
      self.scope.get().0,
    )
  }

  /// Stores `body` as the draft when it differs from the newest revision handed to a save.
  fn flush(&self, body: &Skill, save: &SaveAction<Skill>) {
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
      spec: skill_operations::skill_draft(&self.context.project, entry.id, &entry.name),
      repo_root: self.context.project.repo_root.clone(),
      id: entry.id,
      revision: status.revision,
      draft: Draft::new(body.clone(), based_on, Timestamp::now()),
    };
    self.status.set(status);
    if let Some(job) = self.queue.offer(job) {
      save.run((job, self.queue.clone()));
    }
  }

  /// The save state and Publish vN (`fRsDX`, `gdbJb`).
  fn topbar_actions(
    &self,
    ctx: &Ctx,
    status: &SaveStatus,
    error: Option<&SkillError>,
    (save, publish): (&SaveAction<Skill>, &PublishAction),
  ) -> Vec<Element> {
    let saving = save.state().get();
    let publishing = publish.state().get();
    let pending = saving.is_pending() || status.saved_revision < status.revision;
    let failure = saving
      .error
      .clone()
      .filter(|_| !saving.is_pending())
      .or(publishing.error.clone().filter(|_| !publishing.is_pending()));
    let text = match (error, failure) {
      (Some(_), _) => ctx.t("skills.status.notSaved"),
      (None, Some(failure)) => message_text(ctx, &failure),
      (None, None) if pending => ctx.t("skills.status.saving"),
      (None, None) => save_status(ctx, status.state),
    };
    let next = draft_number(status.state);
    let button = ActionButton::new(ctx.t_args("skills.publish", [("version", next.to_string().as_str())]))
      .primary()
      .icon(Icon::Check)
      .id("skill-publish")
      .height(Dimension::Px(page::TOPBAR_BUTTON_HEIGHT));
    let publishable = matches!(status.state, VersionState::Draft { .. })
      && error.is_none()
      && !pending
      && !publishing.is_pending()
      && !status.body.description().trim().is_empty()
      && !status.body.body().trim().is_empty();
    let button = if publishable {
      let (action, repo_root, entry) = (
        publish.clone(),
        self.context.project.repo_root.clone(),
        self.context.entry.clone(),
      );
      let spec = skill_operations::skill_publish(&self.context.project, entry.id, &entry.name, next);
      button.on_click(move || action.run((repo_root.clone(), entry.id, spec.clone())))
    } else {
      button.on_click(|| {}).enabled(false)
    };
    vec![
      Text::new(&text)
        .variant(TypographyStyle::Caption)
        .id("skill-save-status")
        .into(),
      button.render(ctx),
    ]
  }

  /// All skills, Project, Agent roles, Pipelines & steps (`qpNOl`): which skills the list shows.
  fn tabs(&self, ctx: &Ctx) -> Element {
    let shown = self.tab.get();
    let tab = |id: &'static str, key: &str, filter: ScopeFilter| {
      let select = self.tab.clone();
      Tab::new(id, ctx.t(key), Dimension::Pct(100.0))
        .selected(shown == filter)
        .on_click(move || select.set(filter))
    };
    tab_row(
      Dimension::Px(TABS_HEIGHT),
      SpacingSize::Md,
      Some(SpacingSize::Lg),
      [
        tab("skills-tab-all", "skills.tabs.all", ScopeFilter::All),
        tab("skills-tab-project", "skills.tabs.project", ScopeFilter::Project),
        tab("skills-tab-roles", "skills.tabs.roles", ScopeFilter::Roles),
        tab(
          "skills-tab-pipelines",
          "skills.tabs.pipelines",
          ScopeFilter::PipelinesAndSteps,
        ),
      ],
    )
  }
}

/// Page heading (`ARTuF`).
fn heading(ctx: &Ctx) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_top(SpacingSize::Lg)
    .padding_horizontal(SpacingSize::Lg)
    .padding_bottom(ExtraSpacing::Divider.role())
    .spacing(ExtraSpacing::Inline.role())
    .child(Text::new(&ctx.t("skills.title")).variant(ExtraTypography::PageTitle.role()))
    .child(Text::new(&ctx.t("skills.editorDescription")).variant(TypographyStyle::Description))
    .into()
}

/// "Draft saved · Published v3", "Draft saved · Not published", "Published v3", …
fn save_status(ctx: &Ctx, state: VersionState) -> Arc<str> {
  let version = |version: orchester_core::resource::VersionNumber| version.get().to_string();
  match state {
    VersionState::Published(published) => {
      ctx.t_args("skills.status.published", [("version", version(published).as_str())])
    }
    VersionState::Draft {
      published: Some(published),
    } => ctx.t_args("skills.status.draftSaved", [("version", version(published).as_str())]),
    VersionState::Draft { published: None } => ctx.t("skills.status.draftNeverPublished"),
    VersionState::StaleDraft { based_on, published } => ctx.t_args(
      "skills.status.staleDraft",
      [
        ("basedOn", based_on.map(version).unwrap_or_default().as_str()),
        ("version", published.map(version).unwrap_or_default().as_str()),
      ],
    ),
  }
}

fn publish_action(ctx: &mut Ctx) -> PublishAction {
  let activity = Activity::of(ctx);
  ctx.future_action(move |(repo_root, id, spec): PublishArgs| {
    let activity = activity.clone();
    async move {
      let publish = async move {
        let published = background::run(move || publish_in(&repo_root, id)).await?;
        published.map(|number| u64::from(number.get())).map_err(|error| {
          tracing::error!(%error, "cannot publish the skill");
          messages::store_error(&error)
        })
      };
      activity.track(spec, publish).await
    }
  })
}

/// Why a skill cannot be saved, as its field says it.
pub(super) fn skill_error(error: &SkillError) -> Message {
  match error {
    SkillError::DuplicateFile(path) => Message::new("skills.files.errors.duplicate").with("name", path.clone()),
    SkillError::InvalidPath(path) => Message::new("skills.files.errors.name").with("name", path.clone()),
  }
}
