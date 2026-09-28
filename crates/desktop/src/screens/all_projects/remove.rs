//! Removing a project from the list (01.06 A `EtMXj`), in the frame of the app's confirmations
//! (`P8IN7Z` after the Remove worktree dialog): the title, that nothing is deleted, the folder
//! and its `.orchester` data as staying on disk (`l3hyb` rows), and Cancel and *Remove from list*.
//! Only the application database forgets the project; a runner it has in this session stops at
//! checkpoints. While agents are at work in the project it cannot be removed: the dialog says so
//! at the left of the actions and *Remove from list* is unavailable (01.06 F `glurU`). Escape or
//! Cancel closes it.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Modal, ModalTarget, Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::id::ProjectId;

use crate::{
  background,
  messages::{self, Message},
  navigation::Navigation,
  runner::Runners,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    backdrop::backdrop,
    button::ActionButton,
    check_row::check_row,
    field::wrapping_field_error,
    form::{Submit, form},
    icon::Icon,
    message_text,
  },
  workspace::ProjectEntry,
};

/// Dialog width, action bar and button heights (`P8IN7Z`, `cDtbR`).
const DIALOG_WIDTH: f32 = 600.0;
const ACTIONS_HEIGHT: f32 = 76.0;
const BUTTON_HEIGHT: f32 = 36.0;

#[derive(Clone)]
pub(super) struct RemoveProjectProps {
  pub(super) entry: ProjectEntry,
  /// Agents at work in the project, as its runner last reported.
  pub(super) agents_working: usize,
  /// Whether the dialog is open; Escape, Cancel and a removal close it.
  pub(super) open: Signal<bool>,
  /// Counts removals; the list reads the projects again.
  pub(super) revision: Signal<u64>,
}

impl PartialEq for RemoveProjectProps {
  fn eq(&self, other: &Self) -> bool {
    self.entry == other.entry && self.agents_working == other.agents_working
  }
}

impl lurq::app::component::DevtoolsInspectable for RemoveProjectProps {}

/// The project to forget, and the one forgotten.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Removal(ProjectId);

impl lurq::app::component::DevtoolsInspectable for Removal {}

type RemoveAction = FutureAction<Removal, Removal, Message>;

pub(super) struct RemoveProject;

impl Component for RemoveProject {
  type Props = RemoveProjectProps;

  fn create(ctx: &mut Ctx) -> Self {
    let action = remove_action(ctx);
    let props = ctx.props::<RemoveProjectProps>().clone();
    let navigation = ctx
      .use_context::<Navigation>()
      .expect("the root provides Navigation before mounting screens");
    let runners = ctx
      .use_context::<Runners>()
      .expect("the root provides Runners before mounting screens");
    let repo_root = props.entry.project.repo_root.clone();
    // Forgotten: the project's runner stops, its screens leave history and the list is read again.
    ctx.watch(&action.state(), move |state| {
      let Some(Removal(project)) = state.data.filter(|_| !state.is_pending()) else {
        return;
      };
      runners.forget(&repo_root);
      navigation.forget_project(project);
      props.open.set(false);
      props.revision.update(|revision| *revision += 1);
    });
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RemoveProjectProps>().clone();
    let action = remove_action(ctx);
    let state = action.state().get();
    let pending = state.is_pending();
    let confirm: Submit = (!pending && props.agents_working == 0).then(|| {
      let (action, project) = (action.clone(), props.entry.project.id);
      Arc::new(move || action.run(Removal(project))) as Arc<dyn Fn() + Send + Sync>
    });
    let problem = if props.agents_working > 0 {
      Some(Message::new("projects.remove.agentsWorking").with("name", props.entry.project.name.to_string()))
    } else {
      state.error.filter(|_| !pending)
    };
    let content = Column::new()
      .id("remove-project-dialog")
      .width(Dimension::Px(DIALOG_WIDTH))
      .background(ExtraColor::Surface.role())
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Lg)
      .child(header(ctx, &props))
      .child(kept(ctx, &props))
      .child(actions(ctx, &props, confirm.clone(), problem.as_ref(), pending));
    Modal::new(backdrop(ctx, form(confirm, content)))
      .target(ModalTarget::Root)
      .open(props.open.clone())
  }
}

fn header(ctx: &Ctx, props: &RemoveProjectProps) -> Element {
  let name = props.entry.project.name.as_str();
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_vertical(SpacingSize::Lg)
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(SpacingSize::Sm)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(
      Text::new(&ctx.t_args("projects.remove.title", [("name", name)])).variant(ExtraTypography::DialogTitle.role()),
    )
    .child(Text::new(&ctx.t("projects.remove.explanation")).variant(ExtraTypography::Explanation.role()))
    .into()
}

/// The folder and the project's data, both staying on disk (`Y5ojN`).
fn kept(ctx: &Ctx, props: &RemoveProjectProps) -> Element {
  let stays = ctx.t("projects.remove.staysOnDisk");
  Column::new()
    .id("remove-project-kept")
    .width(Dimension::Pct(100.0))
    .padding_top(ExtraSpacing::Step.role())
    .padding_horizontal(ExtraSpacing::Rail.role())
    .padding_bottom(SpacingSize::Lg)
    .spacing(SpacingSize::Sm)
    .child(check_row(
      Icon::Folder,
      &props.entry.project.repo_root.display().to_string(),
      &stays,
    ))
    .child(check_row(
      Icon::Database,
      &ctx.t("projects.remove.data"),
      &ctx.t("projects.remove.dataStays"),
    ))
    .into()
}

fn actions(
  ctx: &Ctx,
  props: &RemoveProjectProps,
  confirm: Submit,
  problem: Option<&Message>,
  pending: bool,
) -> Element {
  let label = if pending {
    "projects.remove.removing"
  } else {
    "projects.remove.confirm"
  };
  let remove = ActionButton::new(ctx.t(label))
    .primary()
    .icon(Icon::X)
    .id("remove-project-confirm")
    .in_form()
    .height(Dimension::Px(BUTTON_HEIGHT));
  let remove = match confirm {
    Some(confirm) => remove.on_click(move || confirm()),
    None => remove.on_click(|| {}).enabled(false),
  };
  let cancel = props.open.clone();
  Row::new()
    .width(Dimension::Pct(100.0))
    // 76 high as drawn; a reason longer than two lines makes it taller rather than being cut off.
    .min_height(Dimension::Px(ACTIONS_HEIGHT))
    .padding_vertical(ExtraSpacing::Step.role())
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    // The reason takes the room left of the buttons and wraps there (`flqtP`).
    .child(match problem {
      Some(problem) => Column::new()
        .id("remove-project-problem")
        .flex(1.0)
        .child(wrapping_field_error(&message_text(ctx, problem)))
        .into(),
      None => Element::from(Spacer::new().flex(1.0)),
    })
    .child(
      ActionButton::new(ctx.t("common.cancel"))
        .id("remove-project-cancel")
        .in_form()
        .height(Dimension::Px(BUTTON_HEIGHT))
        .on_click(move || cancel.set(false))
        .render(ctx),
    )
    .child(remove.render(ctx))
    .into()
}

fn remove_action(ctx: &mut Ctx) -> RemoveAction {
  let workspace = ctx
    .use_context::<Navigation>()
    .expect("the root provides Navigation before mounting screens")
    .workspace()
    .clone();
  ctx.future_action(move |Removal(project): Removal| {
    let workspace = workspace.clone();
    async move {
      background::run(move || workspace.forget(project))
        .await?
        .map(|()| Removal(project))
        .map_err(|error| {
          tracing::error!(%error, "cannot remove a project from the list");
          messages::store_error(&error)
        })
    }
  })
}
