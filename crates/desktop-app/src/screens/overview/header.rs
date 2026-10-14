//! The Overview's heading (`Header` `PCFyu`): the project's key, its repository and branch as git names
//! them, over its name and what the page shows, with Import resources, not built yet and so locked with
//! why under it, and New task. A project without tasks says what a task is, with the same actions and
//! what is ready to run under them (`Empty project` `h2DTY7`, its list in [`super::ready`]).

use std::path::PathBuf;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  components::{Column, Row, Text, TextOverflow},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::id::ProjectId;
use orchester_workspace::{background, tasks::checkout::Checkout};

use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ProjectPage, Route},
  services::Services,
  theme::{ExtraSpacing, ExtraTypography, OverviewText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    message::message_text,
    state::EmptyState,
    unavailable::with_reason,
  },
};

pub(super) fn header(ctx: &mut Ctx, project: &ShownProject) -> Row {
  let id = project.open.project.id();
  let name = project.open.project.name().as_str();
  let description = ctx.t_args("overview.description", [("project", name)]);
  let eyebrow = EyebrowProps {
    repo_root: project.open.repo_root.clone(),
    key: project.open.project.key().as_str().into(),
    revision: project.feed.revision.get(),
  };
  Row::new()
    .id("overview-header")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::End)
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Snug.role())
        .child(ctx.mount_keyed::<Eyebrow>("overview-eyebrow", eyebrow))
        .child(
          Text::new(name)
            .id("overview-title")
            .variant(ExtraTypography::Display.role()),
        )
        .child(Text::new(&description).variant(OverviewText::Subtitle.role())),
    )
    .child(actions(ctx, id, "overview", true))
}

#[derive(Clone, PartialEq)]
struct EyebrowProps {
  repo_root: PathBuf,
  key: Box<str>,
  /// The project feed's revision: git is asked again whenever the project's data is saved.
  revision: u64,
}

impl DevtoolsInspectable for EyebrowProps {}

/// The line over the project's name: its key alone until git was asked, then with what git named.
struct Eyebrow;

impl Component for Eyebrow {
  type Props = EyebrowProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let EyebrowProps {
      repo_root,
      key,
      revision,
    } = ctx.props::<EyebrowProps>().clone();
    let programs = Services::of(ctx).programs;
    let read = ctx.future((repo_root, key.clone(), revision), move |(repo_root, key, _)| {
      let programs = programs.clone();
      async move { background::run(move || Checkout::read(&programs, &repo_root).eyebrow(&key)).await }
    });
    // Git's parts are extra to the key: while they are read, and when the background work did not finish
    // (logged there), the line is the key alone, as when git names neither.
    let line = read
      .state()
      .get()
      .data
      .unwrap_or_else(|| Checkout::default().eyebrow(&key));
    Text::new(&message_text(ctx, &line))
      .id("overview-eyebrow")
      .variant(OverviewText::Eyebrow.role())
      .width(Dimension::Pct(100.0))
      .nowrap()
      .text_overflow(TextOverflow::Elipsis)
  }
}

/// The explanation's measure in the empty project's card (`F7LdL`).
const EMPTY_MEASURE: f32 = 520.0;

/// No task yet: what a task is, the heading's actions and what is ready to run.
pub(super) fn empty_project(ctx: &mut Ctx, project: &ShownProject) -> Element {
  let actions = actions(ctx, project.open.project.id(), "overview-empty", false);
  EmptyState::new(
    "overview-empty",
    Icon::ListPlus,
    ctx.t("overview.empty.title"),
    ctx.t("overview.empty.detail"),
  )
  .measure(EMPTY_MEASURE)
  .action(actions)
  .child(super::ready::ready(ctx, project))
  .into()
}

/// Import resources, locked with why under it, and New task, under the element ids `<id>-*`; New task
/// last in the heading, first in the empty project's card.
fn actions(ctx: &mut Ctx, project: ProjectId, id: &str, heading: bool) -> Column {
  let navigation = Navigation::of(ctx);
  let reason = ctx.t("overview.importReason");
  let import = ActionButton::new(
    format!("{id}-import"),
    ButtonKind::Secondary,
    ctx.t("overview.importResources"),
  )
  .icon(Icon::Download)
  .unavailable(reason.clone());
  let new_task = ActionButton::new(format!("{id}-new-task"), ButtonKind::Primary, ctx.t("overview.newTask"))
    .icon(Icon::Plus)
    .on_click(move || navigation.go(Route::Project(project, ProjectPage::NewTask)));
  let row = Row::new().spacing(SpacingSize::Sm).align_items(Alignment::Center);
  let row = if heading {
    row.child(import).child(new_task)
  } else {
    row.child(new_task).child(import)
  };
  with_reason(row, &format!("{id}-import-reason"), &reason)
}
