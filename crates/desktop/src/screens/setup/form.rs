//! The project form of 01.03 (`Allwo`): name, key and local repository, then creation through
//! the workspace. Validation is `ProjectDraft::validate`; storage rules belong to the store.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction, FutureState},
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};

use crate::{
  background,
  messages::{self, Message},
  navigation::{Navigation, Screen},
  project_setup::{DraftErrors, NewProject, ProjectDraft},
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    field::{FieldNote, ValueTone, field, text_input, value_box},
    icon::{Icon, IconSize, icon},
    message_text,
    notice::notice,
  },
  workspace::{OpenProject, Workspace},
};

/// Topbar height (`MYfj5`).
const TOPBAR_HEIGHT: f32 = 64.0;

/// A folder chosen in the native folder dialog.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Folder(PathBuf);

impl lurq::app::component::DevtoolsInspectable for Folder {}

/// The dialog title and the folder chosen so far, kept when the dialog is cancelled.
type PickArgs = (Arc<str>, Option<Folder>);
type PickAction = FutureAction<PickArgs, Option<Folder>, Message>;
type CreateAction = FutureAction<NewProject, OpenProject, Message>;

pub struct ProjectForm {
  name: Signal<String>,
  key: Signal<String>,
  errors: Signal<DraftErrors>,
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
    let form = Self {
      name: ctx.signal(String::new()),
      key: ctx.signal(String::new()),
      errors: ctx.signal(DraftErrors::default()),
    };
    // A field's error describes the value it was raised for; editing the field retires it.
    let errors = form.errors.clone();
    ctx.watch(&form.name, move |_| errors.update(|errors| errors.name = None));
    let errors = form.errors.clone();
    ctx.watch(&form.key, move |_| errors.update(|errors| errors.key = None));
    form
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let workspace = navigation(ctx).workspace().clone();
    let pick = pick_action(ctx);
    let create = create_action(ctx, workspace);
    let repository = pick.state().get().data.flatten();
    let errors = self.errors.get();
    let create_state = create.state().get();
    let name = self.name.get();
    let name = name.trim();
    // Fields scroll when the window is shorter than the design; the actions stay visible.
    let fields = Column::new()
      .width(Dimension::Pct(100.0))
      .padding_top(SpacingSize::Md)
      .padding_horizontal(ExtraSpacing::Page.role())
      .padding_bottom(SpacingSize::Lg)
      .spacing(SpacingSize::Lg)
      .child(self.identity_row(ctx, &errors))
      .child(notice(
        Icon::CloudOff,
        &ctx.t("setup.project.githubUnavailable"),
        &ctx.t("setup.project.githubUnavailableDetail"),
      ))
      .child(repository_row(ctx, &pick, repository.as_ref(), &errors))
      .child(
        ActionButton::new(ctx.t("setup.project.changeGithub"))
          .icon(Icon::ExternalLink)
          .id("change-github")
          .render(ctx),
      )
      .child(ownership(ctx, name));
    let actions = Column::new()
      .width(Dimension::Pct(100.0))
      .padding_horizontal(ExtraSpacing::Page.role())
      .padding_bottom(SpacingSize::Section)
      .child(self.actions(ctx, &create, &create_state, repository));
    Column::new()
      .flex(1.0)
      .height(Dimension::Pct(100.0))
      .child(topbar(ctx, name))
      .child(heading(ctx))
      .child(ScrollVertical::new(fields).width(Dimension::Pct(100.0)).flex(1.0))
      .child(actions)
  }
}

impl ProjectForm {
  /// Project name as designed, with the project key beside it: the key is required by
  /// `orchester_core::project::Project` and has no field in the design yet.
  fn identity_row(&self, ctx: &Ctx, errors: &DraftErrors) -> Element {
    let error = |message: Option<&Message>| {
      message.map_or(FieldNote::None, |message| FieldNote::Error(message_text(ctx, message)))
    };
    let name = field(
      &ctx.t("setup.project.name"),
      text_input(ctx, "project-name", self.name.clone()),
      error(errors.name.as_ref()),
    );
    let key = field(
      &ctx.t("setup.project.key"),
      text_input(ctx, "project-key", self.key.clone()),
      error(errors.key.as_ref()),
    );
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Lg)
      .child(Column::new().flex(1.0).child(name))
      .child(Column::new().flex(1.0).child(key))
      .into()
  }

  fn actions(
    &self,
    ctx: &Ctx,
    create: &CreateAction,
    state: &FutureState<OpenProject, Message>,
    repository: Option<Folder>,
  ) -> Element {
    let pending = state.is_pending();
    let status: Element = match state.error.as_ref().filter(|_| !pending) {
      Some(error) => inline_error(&message_text(ctx, error)),
      None => Text::new(&ctx.t("setup.project.createExplanation"))
        .variant(TypographyStyle::Caption)
        .into(),
    };
    let label = if pending {
      ctx.t("setup.project.creating")
    } else {
      ctx.t("setup.project.create")
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
          .on_click(self.submit(create.clone(), repository))
          .enabled(!pending)
          .render(ctx),
      )
      .into()
  }

  fn submit(&self, create: CreateAction, repository: Option<Folder>) -> impl Fn() + Send + Sync + 'static {
    let (name, key, errors) = (self.name.clone(), self.key.clone(), self.errors.clone());
    move || {
      let name = name.get_untracked();
      let key = key.get_untracked();
      let draft = ProjectDraft {
        name: &name,
        key: &key,
        repository: repository.as_ref().map(|folder| folder.0.as_path()),
      };
      match draft.validate() {
        Ok(new) => {
          errors.set(DraftErrors::default());
          create.run(new);
        }
        Err(problems) => errors.set(problems),
      }
    }
  }
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
      background::run(move || workspace.create(new)).await?.map_err(|error| {
        tracing::error!(%error, "cannot create the project");
        messages::store_error(&error)
      })
    }
  })
}

fn repository_row(ctx: &Ctx, pick: &PickAction, repository: Option<&Folder>, errors: &DraftErrors) -> Element {
  let picking = pick.state().get_untracked().is_pending();
  let (path, tone) = match repository {
    Some(folder) => (folder.0.display().to_string(), ValueTone::Value),
    None => (ctx.t("setup.project.noRepository").to_string(), ValueTone::Placeholder),
  };
  let choose: Option<(Icon, Box<dyn Fn() + Send + Sync>)> = (!picking).then(|| {
    let pick = pick.clone();
    let title = ctx.t("setup.project.chooseRepositoryTitle");
    let current = repository.cloned();
    let open: Box<dyn Fn() + Send + Sync> = Box::new(move || pick.run((title.clone(), current.clone())));
    (Icon::FolderOpen, open)
  });
  let note = errors
    .repository
    .as_ref()
    .filter(|_| repository.is_none())
    .map_or(FieldNote::None, |error| FieldNote::Error(message_text(ctx, error)));
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
    .child(
      Column::new()
        .flex(1.0)
        .child(field(&ctx.t("setup.project.repository"), folder, note)),
    )
    .child(
      Column::new()
        .flex(1.0)
        .child(field(&ctx.t("setup.project.defaultBranch"), branch, FieldNote::None)),
    )
    .into()
}

/// Topbar (`MYfj5`) with the breadcrumb. There is no setup draft, so no save status.
fn topbar(ctx: &Ctx, name: &str) -> Element {
  let breadcrumb = if name.is_empty() {
    ctx.t("setup.project.breadcrumb")
  } else {
    ctx.t_args("setup.project.breadcrumbNamed", [("name", name)])
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(TOPBAR_HEIGHT))
    .padding_horizontal(SpacingSize::Section)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(Text::new(&breadcrumb).variant(TypographyStyle::Description))
    .into()
}

/// Page heading (`b0m9CN`).
fn heading(ctx: &Ctx) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_top(ExtraSpacing::Rail.role())
    .padding_horizontal(SpacingSize::Section)
    .padding_bottom(SpacingSize::Lg)
    .spacing(ExtraSpacing::Stack.role())
    .child(Text::new(&ctx.t("setup.project.title")).variant(TypographyStyle::Heading))
    .child(Text::new(&ctx.t("setup.project.description")).variant(TypographyStyle::Description))
    .into()
}

fn ownership(ctx: &Ctx, name: &str) -> Element {
  let detail = if name.is_empty() {
    ctx.t("setup.project.ownershipDetail")
  } else {
    ctx.t_args("setup.project.ownershipDetailNamed", [("name", name)])
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_vertical(SpacingSize::Lg)
    .spacing(ExtraSpacing::Stack.role())
    .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(Text::new(&ctx.t("setup.project.ownershipTitle")).variant(ExtraTypography::Subheading.role()))
    .child(
      Text::new(&detail)
        .variant(TypographyStyle::Body)
        .color(PaletteColor::TextSecondary),
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
