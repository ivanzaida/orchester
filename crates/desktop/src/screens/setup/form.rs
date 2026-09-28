//! The project form of 01.02 / 01.03 (`F5bGe`, `Allwo`): name, project folder and the folder's
//! GitHub repository, then creation through the workspace. Validation is
//! `ProjectDraft::validate`; storage rules belong to the store. Create stays disabled only
//! without a name or folder, or for a folder that is not a git repository (01.C9); GitHub states
//! never block it.

use std::{path::Path, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Spacer, Text},
  core::Signal,
  layout::{Alignment, layout_kind::ScrollState},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  connection::GitHubLogin,
  repository::{Detection, DetectionState},
};

use super::{
  folder::{PickAction, folder_field, pick_action},
  github::SetupGitHub,
  page::{heading, ownership, topbar},
};
use crate::{
  activity::{Activity, operations},
  background,
  messages::Message,
  navigation::Navigation,
  project_setup::{CreateFailure, ErrorPlace, NewProject, ProjectDraft, creatable},
  repository::adopt_in,
  screens::github::actions::{
    ChosenPath, DetectAction, ProgramAction, connection_name, detect_action, finished, program_action,
  },
  theme::ExtraSpacing,
  ui::{
    button::ActionButton,
    field::{FieldNote, field, text_input},
    form::{Submit, form},
    github_mark::github_mark,
    icon::{Icon, IconSize, icon},
    message_text,
    notice::Notice,
  },
  workspace::{OpenProject, Workspace},
};

/// What Create records besides the project: the detection it saw, the account the project
/// expects and the name of the GitHub connection it adds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Creation {
  new: NewProject,
  detection: Option<Detection>,
  expected: Option<GitHubLogin>,
  connection_name: String,
}

type CreateAction = FutureAction<Creation, OpenProject, CreateFailure>;

/// The futures of the form, requested in this order in `create` and `render`.
struct Actions {
  pick: PickAction,
  detect: DetectAction,
  program: ProgramAction,
  create: CreateAction,
}

impl Actions {
  fn of(ctx: &mut Ctx) -> Self {
    let workspace = navigation(ctx).workspace().clone();
    Self {
      pick: pick_action(ctx),
      detect: detect_action(ctx, None),
      program: program_action(ctx),
      create: create_action(ctx, workspace),
    }
  }
}

pub struct ProjectForm {
  name: Signal<String>,
  /// The chosen project folder.
  folder: Signal<Option<ChosenPath>>,
  github: SetupGitHub,
  /// Where the page is scrolled: kept by the component, so a tab that goes inactive and back
  /// keeps it.
  scroll: ScrollState,
}

impl Component for ProjectForm {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    let navigation = navigation(ctx);
    let actions = Actions::of(ctx);
    ctx.watch(&actions.create.state(), move |state| {
      if let Some(project) = state.data.clone() {
        navigation.open_created(project);
      }
    });
    let form = Self {
      name: ctx.signal(String::new()),
      folder: ctx.signal(None),
      github: SetupGitHub::new(ctx),
      scroll: ScrollState::new(),
    };
    // A newly chosen folder is detected at once.
    let (folder, github, detect) = (form.folder.clone(), form.github.clone(), actions.detect.clone());
    ctx.watch(&actions.pick.state(), move |state| {
      let (Some(Some(picked)), false) = (&state.data, state.is_pending()) else {
        return;
      };
      if folder.get_untracked().as_ref() != Some(picked) {
        folder.set(Some(picked.clone()));
        github.check(&detect, picked.0.clone())();
      }
    });
    form
      .github
      .watch(ctx, &actions.detect, &actions.program, form.folder.clone());
    form
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let actions = Actions::of(ctx);
    let folder = self.folder.get().map(|folder| folder.0);
    let create_state = actions.create.state().get();
    let raw_name = self.name.get();
    let name = raw_name.trim();
    let input = ProjectDraft {
      name: &raw_name,
      repository: folder.as_deref(),
    };
    let draft = input.validate();
    // A creation error stays only while the name and folder it was raised for are unchanged.
    let failure = create_state
      .error
      .as_ref()
      .filter(|failure| !create_state.is_pending() && failure.applies_to(&input));
    let failure_at = |place: ErrorPlace| {
      failure
        .filter(|failure| failure.place == place)
        .map(|failure| message_text(ctx, &failure.message))
    };
    let folder_error = failure_at(ErrorPlace::Repository);
    let form_error = failure_at(ErrorPlace::Form);
    // A blank name only keeps Create disabled; a name the rules reject says why.
    let name_error = draft
      .as_ref()
      .err()
      .and_then(|errors| errors.name.as_ref())
      .filter(|_| !name.is_empty())
      .map(|error| message_text(ctx, error));
    // Only a name the rules accept is repeated in the breadcrumb, the notes and the explanation.
    let shown_name = if name_error.is_some() { "" } else { name };
    let project = if shown_name.is_empty() {
      ctx.t("setup.project.unnamed")
    } else {
      shown_name.into()
    };

    let detection = finished(&actions.detect);
    let expected = self.github.expected();
    let state = detection.as_ref().map(|detection| detection.state(expected.as_ref()));
    let blocked = state.as_ref().is_some_and(DetectionState::blocks_project);

    let fields = Column::new()
      .width(Dimension::Pct(100.0))
      .padding_top(SpacingSize::Md)
      .padding_horizontal(ExtraSpacing::Page.role())
      .padding_bottom(SpacingSize::Lg)
      .spacing(SpacingSize::Lg)
      .child(self.name_field(ctx, name_error))
      .child(folder_field(ctx, &actions.pick, folder.as_ref(), folder_error))
      .child(self.github_part(ctx, &actions, folder.as_deref(), &project))
      .child(ownership(ctx, shown_name));
    let pending = create_state.is_pending();
    let submit: Submit = creatable(draft, state.as_ref()).filter(|_| !pending).map(|new| {
      let create = actions.create.clone();
      let creation = Creation {
        connection_name: connection_name(ctx),
        new,
        detection: detection.clone(),
        expected: expected.clone(),
      };
      Arc::new(move || create.run(creation.clone())) as Arc<dyn Fn() + Send + Sync>
    });
    let status = status_line(ctx, folder.is_some(), blocked);
    let footer = Column::new()
      .width(Dimension::Pct(100.0))
      // Create's focus ring reaches above the footer, into the fields' bottom padding.
      .overflow_visible()
      .padding_horizontal(ExtraSpacing::Page.role())
      .padding_bottom(SpacingSize::Section)
      .child(actions_row(ctx, pending, form_error, &status, submit.clone()));
    let page = Column::new()
      .flex(1.0)
      .height(Dimension::Pct(100.0))
      .child(topbar(ctx, shown_name))
      .child(heading(ctx))
      .child(
        ScrollVertical::new(fields)
          .with_scroll_state(self.scroll.clone())
          .width(Dimension::Pct(100.0))
          .flex(1.0),
      )
      .child(footer)
      .child(self.github.panel(ctx, &actions.detect, folder.clone(), &project));
    form(submit, page)
  }
}

/// What the footer says beside Create: to choose a folder, that the folder needs a repository, or
/// what Create does.
fn status_line(ctx: &Ctx, folder_chosen: bool, blocked: bool) -> Arc<str> {
  match (folder_chosen, blocked) {
    (false, _) => ctx.t("setup.project.chooseFolderToContinue"),
    (true, true) => ctx.t("setup.project.needsRepository"),
    (true, false) => ctx.t("setup.project.createExplanation"),
  }
}

impl ProjectForm {
  /// Before a folder is chosen, that GitHub is detected from it (`hfxMm`); then what was
  /// detected (`XcbNT`).
  fn github_part(&self, ctx: &Ctx, actions: &Actions, folder: Option<&Path>, project: &str) -> Element {
    match folder {
      None => Notice::new(
        Icon::FolderGit,
        ctx.t("setup.project.detectedFromFolder"),
        ctx.t("setup.project.detectedFromFolderDetail"),
      )
      .mark(github_mark(ctx, PaletteColor::TextSecondary))
      .id("github-detected-from-folder")
      .into(),
      Some(folder) => self
        .github
        .block(ctx, &actions.detect, &actions.program, folder, project),
    }
  }

  fn name_field(&self, ctx: &Ctx, error: Option<Arc<str>>) -> Element {
    let invalid = error.is_some();
    field(
      &ctx.t("setup.project.name"),
      text_input(
        ctx,
        "project-name",
        self.name.clone(),
        invalid,
        TypographyStyle::FieldLabel,
      )
      .into(),
      error.map_or(FieldNote::None, FieldNote::Error),
    )
  }
}

/// Explanation or creation error, and Create, which stays disabled until the draft is valid.
fn actions_row(ctx: &Ctx, pending: bool, error: Option<Arc<str>>, status: &str, submit: Submit) -> Element {
  let status: Element = match error {
    Some(error) => inline_error(&error),
    None => Text::new(status)
      .id("create-explanation")
      .variant(TypographyStyle::Caption)
      .into(),
  };
  let label = if pending {
    ctx.t("setup.project.creating")
  } else {
    ctx.t("setup.project.create")
  };
  let create = ActionButton::new(label)
    .primary()
    .icon(Icon::ArrowRight)
    .id("create-project")
    .in_form();
  let create = match submit {
    Some(submit) => create.on_click(move || submit()),
    None => create.on_click(|| {}).enabled(false),
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    // Create is as tall as the row; its focus ring reaches past it.
    .overflow_visible()
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .child(status)
    .child(Spacer::new().flex(1.0))
    .child(create.render(ctx))
    .into()
}

fn navigation(ctx: &mut Ctx) -> Navigation {
  ctx
    .use_context::<Navigation>()
    .expect("the root provides Navigation before mounting screens")
}

fn create_action(ctx: &mut Ctx, workspace: Workspace) -> CreateAction {
  let activity = Activity::of(ctx);
  ctx.future_action(move |creation: Creation| {
    let (workspace, activity) = (workspace.clone(), activity.clone());
    async move {
      let Creation {
        new,
        detection,
        expected,
        connection_name,
      } = creation;
      let attempted = new.clone();
      let running = activity.start(operations::project_create(new.project.name().as_str(), &new.repo_root));
      let created = background::run(move || workspace.create(new))
        .await
        .map_err(|message| CreateFailure::new(&attempted, message, ErrorPlace::Form))
        .and_then(|created| {
          created.map_err(|error| {
            tracing::error!(%error, "cannot create the project");
            CreateFailure::from_store(&attempted, &error)
          })
        });
      match &created {
        Ok(_) => running.finish(),
        Err(failure) => running.fail(failure.message.clone()),
      }
      if let (Ok(project), Some(detection)) = (&created, detection) {
        adopt(&activity, project, detection, expected, connection_name).await;
      }
      created
    }
  })
}

/// Records the GitHub connection and link of a created project. Its failure is reported in the
/// status bar; the project exists and opens either way, and can be linked from Project settings.
async fn adopt(
  activity: &Activity,
  project: &OpenProject,
  detection: Detection,
  expected: Option<GitHubLogin>,
  name: String,
) {
  let Some(found) = detection.github() else {
    return;
  };
  let repository = format!("{}/{}", found.repo().owner(), found.repo().name());
  let spec = operations::repository_link(project.project.name().as_str(), &project.repo_root, &repository);
  let repo_root = project.repo_root.clone();
  let work = async move {
    background::run(move || adopt_in(&repo_root, &detection, expected.as_ref(), &name))
      .await?
      .map_err(|error| {
        tracing::error!(%error, "cannot record the detected GitHub repository");
        error.message()
      })
  };
  // The outcome is in the status bar; the created project does not depend on it.
  let _recorded: Result<_, Message> = activity.track(spec, work).await;
}

fn inline_error(text: &str) -> Element {
  Row::new()
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .describe("role", "form-error")
    .child(icon(Icon::AlertCircle, IconSize::Small, PaletteColor::TextPrimary))
    .child(
      Text::new(text)
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextPrimary),
    )
    .into()
}
