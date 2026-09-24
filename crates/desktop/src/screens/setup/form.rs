//! The project form of 01.03 (`Allwo`): name and local repository, then creation through the
//! workspace. Validation is `ProjectDraft::validate`; storage rules belong to the store.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use super::page::{heading, ownership, topbar};
use crate::{
  background, display,
  messages::Message,
  navigation::{Navigation, Screen},
  project_setup::{CreateFailure, ErrorPlace, NewProject, ProjectDraft},
  theme::ExtraSpacing,
  ui::{
    button::ActionButton,
    field::{FieldNote, ValueTone, field, text_input, value_box},
    icon::{Icon, IconSize, icon},
    message_text,
    notice::notice,
  },
  workspace::{OpenProject, Workspace},
};

/// Width of "Change GitHub account or access" (`w5XOc`).
const CHANGE_GITHUB_WIDTH: f32 = 284.0;
/// Longest repository path shown in full; longer paths keep their last folders (`A3kRgn`).
const PATH_CHARS: usize = 64;

/// A folder chosen in the native folder dialog.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Folder(PathBuf);

impl lurq::app::component::DevtoolsInspectable for Folder {}

/// The dialog title and the folder chosen so far, kept when the dialog is cancelled.
type PickArgs = (Arc<str>, Option<Folder>);
type PickAction = FutureAction<PickArgs, Option<Folder>, Message>;
type CreateAction = FutureAction<NewProject, OpenProject, CreateFailure>;

pub struct ProjectForm {
  name: Signal<String>,
}

impl Component for ProjectForm {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    let navigation = navigation(ctx);
    // Requested here so the watch is registered once; `render` requests the same slots again.
    pick_action(ctx);
    let create = create_action(ctx, navigation.workspace().clone());
    ctx.watch(&create.state(), move |state| {
      if let Some(project) = state.data.clone() {
        navigation.show(Screen::Project(project));
      }
    });
    Self {
      name: ctx.signal(String::new()),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let workspace = navigation(ctx).workspace().clone();
    let pick = pick_action(ctx);
    let create = create_action(ctx, workspace);
    let repository = pick.state().get().data.flatten();
    let create_state = create.state().get();
    let raw_name = self.name.get();
    let name = raw_name.trim();
    let input = ProjectDraft {
      name: &raw_name,
      repository: repository.as_ref().map(|folder| folder.0.as_path()),
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
    // Only a name the rules accept is repeated in the breadcrumb and the explanation.
    let shown_name = if name_error.is_some() { "" } else { name };

    // Fields scroll when the window is shorter than the design; the actions stay visible.
    let fields = Column::new()
      .width(Dimension::Pct(100.0))
      .padding_top(SpacingSize::Md)
      .padding_horizontal(ExtraSpacing::Page.role())
      .padding_bottom(SpacingSize::Lg)
      .spacing(SpacingSize::Lg)
      .child(self.name_field(ctx, name_error))
      .child(notice(
        Icon::CloudOff,
        &ctx.t("setup.project.githubUnavailable"),
        &ctx.t("setup.project.githubUnavailableDetail"),
      ))
      .child(repository_row(ctx, &pick, repository.as_ref(), folder_error))
      .child(
        ActionButton::new(ctx.t("setup.project.changeGithub"))
          .icon(Icon::ExternalLink)
          .id("change-github")
          .width(Dimension::Px(CHANGE_GITHUB_WIDTH))
          .render(ctx),
      )
      .child(ownership(ctx, shown_name));
    let actions = Column::new()
      .width(Dimension::Pct(100.0))
      .padding_horizontal(ExtraSpacing::Page.role())
      .padding_bottom(SpacingSize::Section)
      .child(actions(ctx, &create, create_state.is_pending(), form_error, draft.ok()));
    Column::new()
      .flex(1.0)
      .height(Dimension::Pct(100.0))
      .child(topbar(ctx, shown_name))
      .child(heading(ctx))
      .child(ScrollVertical::new(fields).width(Dimension::Pct(100.0)).flex(1.0))
      .child(actions)
  }
}

impl ProjectForm {
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
fn actions(
  ctx: &Ctx,
  create: &CreateAction,
  pending: bool,
  error: Option<Arc<str>>,
  ready: Option<NewProject>,
) -> Element {
  let status: Element = match error {
    Some(error) => inline_error(&error),
    None => Text::new(&ctx.t("setup.project.createExplanation"))
      .variant(TypographyStyle::Caption)
      .into(),
  };
  let label = if pending {
    ctx.t("setup.project.creating")
  } else {
    ctx.t("setup.project.create")
  };
  let enabled = ready.is_some() && !pending;
  let run = {
    let create = create.clone();
    move || {
      if let Some(new) = ready.clone() {
        create.run(new);
      }
    }
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .child(status)
    .child(Spacer::new().flex(1.0))
    .child(
      ActionButton::new(label)
        .primary()
        .icon(Icon::ArrowRight)
        .id("create-project")
        .on_click(run)
        .enabled(enabled)
        .render(ctx),
    )
    .into()
}

fn navigation(ctx: &mut Ctx) -> Navigation {
  ctx
    .use_context::<Navigation>()
    .expect("the root provides Navigation before mounting screens")
}

fn pick_action(ctx: &mut Ctx) -> PickAction {
  ctx.future_action(|(title, current): PickArgs| async move {
    let picked = background::run(move || rfd::FileDialog::new().set_title(&*title).pick_folder()).await?;
    Ok(picked.map(Folder).or(current))
  })
}

fn create_action(ctx: &mut Ctx, workspace: Workspace) -> CreateAction {
  ctx.future_action(move |new: NewProject| {
    let workspace = workspace.clone();
    async move {
      let attempted = new.clone();
      let created = background::run(move || workspace.create(new))
        .await
        .map_err(|message| CreateFailure::new(&attempted, message, ErrorPlace::Form))?;
      created.map_err(|error| {
        tracing::error!(%error, "cannot create the project");
        CreateFailure::from_store(&attempted, &error)
      })
    }
  })
}

fn repository_row(ctx: &Ctx, pick: &PickAction, repository: Option<&Folder>, error: Option<Arc<str>>) -> Element {
  let picking = pick.state().get_untracked().is_pending();
  let tone = if error.is_some() {
    ValueTone::Invalid
  } else {
    ValueTone::Value
  };
  let (path, tone) = match repository {
    Some(folder) => (display::shorten_path(&folder.0, PATH_CHARS), tone),
    None => (ctx.t("setup.project.noRepository").to_string(), ValueTone::Placeholder),
  };
  let choose: Option<(Icon, Box<dyn Fn() + Send + Sync>)> = (!picking).then(|| {
    let pick = pick.clone();
    let title = ctx.t("setup.project.chooseRepositoryTitle");
    let current = repository.cloned();
    let open: Box<dyn Fn() + Send + Sync> = Box::new(move || pick.run((title.clone(), current.clone())));
    (Icon::FolderOpen, open)
  });
  let folder = value_box(ctx, "repository-path", &path, tone, choose);
  let branch = value_box(
    ctx,
    "default-branch",
    &ctx.t("setup.project.branchUnavailable"),
    ValueTone::Unavailable,
    None,
  );
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .child(Column::new().flex(1.0).child(field(
      &ctx.t("setup.project.repository"),
      folder,
      error.map_or(FieldNote::None, FieldNote::Error),
    )))
    .child(
      Column::new()
        .flex(1.0)
        .child(field(&ctx.t("setup.project.defaultBranch"), branch, FieldNote::None)),
    )
    .into()
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
