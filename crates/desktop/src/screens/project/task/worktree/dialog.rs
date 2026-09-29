//! The confirmation of *Remove worktree* (02.Q `LMrbK`, 02.Q2 `g1HS8`), in the frame of the app's
//! confirmations (`jYL1V`, `M4vhEv`): a title and what removing does, then each worktree (`l3hyb`
//! rows) with its uncommitted files named under its title, each branch with whether it is deleted
//! (merged) or kept, the switch that agrees to discard the uncommitted changes when there are any
//! (`K8ItM` in `tpwQD`), and Cancel and Remove.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::integration::{LeftBranch, LeftWorktree, WorktreeOwner};

use super::RemoveWorktreeProps;
use crate::{
  messages::Message,
  tasks::{detail::RunView, integration::file_list},
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    backdrop::backdrop,
    button::ActionButton,
    check_row::check_row,
    field::wrapping_field_error,
    form::{Submit, form},
    icon::Icon,
    message_text,
    switch::Switch,
  },
};

/// Dialog width, action bar and button heights (`jYL1V`, `M4vhEv`).
const DIALOG_WIDTH: f32 = 600.0;
const ACTIONS_HEIGHT: f32 = 76.0;
const BUTTON_HEIGHT: f32 = 36.0;

pub(super) struct Dialog<'a> {
  pub(super) props: &'a RemoveWorktreeProps,
  pub(super) discard: bool,
  pub(super) flip_discard: Arc<dyn Fn() + Send + Sync>,
  pub(super) pending: bool,
  pub(super) error: Option<Message>,
  /// Removes; `None` while it runs or until discarding is agreed to.
  pub(super) confirm: Submit,
  pub(super) cancel: Arc<dyn Fn() + Send + Sync>,
}

pub(super) fn dialog(ctx: &mut Ctx, dialog: &Dialog) -> Element {
  let run = dialog
    .props
    .detail
    .run
    .as_ref()
    .expect("worktrees are removed for the task's latest run");
  let leftovers = &dialog.props.leftovers;
  let number = run.run.number().get().to_string();
  let (title_key, explanation_key) = if leftovers.worktrees.len() == 1 {
    ("runs.worktree.title.one", "runs.worktree.explanation.one")
  } else {
    ("runs.worktree.title.other", "runs.worktree.explanation.other")
  };
  let header = Column::new()
    .width(Dimension::Pct(100.0))
    .padding_vertical(SpacingSize::Lg)
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(SpacingSize::Sm)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(Text::new(&ctx.t_args(title_key, [("run", number.as_str())])).variant(ExtraTypography::DialogTitle.role()))
    .child(Text::new(&ctx.t(explanation_key)).variant(ExtraTypography::Explanation.role()));
  let separator = ctx.t("common.listSeparator");
  let body = Column::new()
    .id("remove-worktree-list")
    .width(Dimension::Pct(100.0))
    .padding_top(ExtraSpacing::Step.role())
    .padding_horizontal(ExtraSpacing::Rail.role())
    .padding_bottom(SpacingSize::Lg)
    .spacing(SpacingSize::Sm)
    .with_children(
      leftovers
        .worktrees
        .iter()
        .map(|worktree| worktree_row(ctx, run, worktree, &separator)),
    )
    .with_children(leftovers.branches.iter().map(|branch| branch_row(ctx, branch)))
    .with_children(
      (leftovers.uncommitted() > 0)
        .then(|| {
          Switch::new(
            "remove-worktree-discard",
            ctx.t("runs.worktree.discard"),
            dialog.discard,
          )
        })
        .map(|switch| {
          let flip = dialog.flip_discard.clone();
          Column::new()
            .padding_top(SpacingSize::Sm)
            .child(switch.emphasized().render(move || flip()))
        }),
    );
  let content = Column::new()
    .id("remove-worktree-dialog")
    .width(Dimension::Px(DIALOG_WIDTH))
    .background(ExtraColor::Surface.role())
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .child(header)
    .child(body)
    .child(actions(ctx, dialog));
  backdrop(ctx, form(dialog.confirm.clone(), content))
}

/// "Run #3" or the step's name, then its state; the uncommitted files under its title (`gzztk`).
fn worktree_row(ctx: &Ctx, run: &RunView, worktree: &LeftWorktree, separator: &str) -> Element {
  let owner = match worktree.owner {
    WorktreeOwner::Run => ctx.t_args(
      "runs.worktree.runOwner",
      [("run", run.run.number().get().to_string().as_str())],
    ),
    WorktreeOwner::Step(execution) => {
      let step = run
        .step_of(execution)
        .map_or_else(String::new, |step| step.name.to_string());
      ctx.t_args("runs.worktree.stepOwner", [("step", step.as_str())])
    }
  };
  let files = worktree.uncommitted.len();
  let (glyph, state) = match (worktree.missing, files) {
    (true, _) => (Icon::FolderX, ctx.t("runs.worktree.state.missing")),
    (false, 0) => (Icon::Check, ctx.t("runs.worktree.state.clean")),
    (false, 1) => (Icon::FileDiff, ctx.t("runs.worktree.state.uncommitted.one")),
    (false, count) => (
      Icon::FileDiff,
      ctx.t_args(
        "runs.worktree.state.uncommitted.other",
        [("count", count.to_string().as_str())],
      ),
    ),
  };
  let listed = (files > 0).then(|| {
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding_left(ExtraSpacing::CheckIndent.role())
      .padding_bottom(SpacingSize::Xs)
      .child(
        Text::new(&file_list(&worktree.uncommitted, separator))
          .variant(ExtraTypography::MonoValue.role())
          .color(PaletteColor::TextSecondary)
          .width(Dimension::Pct(100.0)),
      )
  });
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .child(check_row(glyph, &owner, &state))
    .with_children(listed)
    .into()
}

/// A branch and what removing does to it: deleted (its commits are merged) or kept.
fn branch_row(ctx: &Ctx, branch: &LeftBranch) -> Element {
  let (glyph, state) = if branch.merged {
    (Icon::GitMerge, ctx.t("runs.worktree.branch.deleted"))
  } else {
    (Icon::Pin, ctx.t("runs.worktree.branch.kept"))
  };
  check_row(glyph, branch.name.as_str(), &state).into()
}

fn actions(ctx: &Ctx, dialog: &Dialog) -> Element {
  let count = dialog.props.leftovers.worktrees.len();
  let label = match (dialog.pending, count) {
    (true, _) => "runs.worktree.removing",
    (false, 1) => "runs.worktree.remove.one",
    (false, _) => "runs.worktree.remove.other",
  };
  // While git removes the worktrees the primary waits with a loader (`nf0IL` in 02.Q states `XIEn8`).
  let glyph = if dialog.pending { Icon::Loader } else { Icon::FolderX };
  let confirm = ActionButton::new(ctx.t(label))
    .primary()
    .icon(glyph)
    .id("remove-worktree-confirm")
    .in_form()
    .height(Dimension::Px(BUTTON_HEIGHT));
  let confirm = match dialog.confirm.clone() {
    Some(remove) => confirm.on_click(move || remove()),
    None => confirm.on_click(|| {}).enabled(false),
  };
  let cancel = dialog.cancel.clone();
  Row::new()
    .width(Dimension::Pct(100.0))
    // 76 high as drawn; a reason longer than two lines makes it taller rather than being cut off.
    .min_height(Dimension::Px(ACTIONS_HEIGHT))
    .padding_vertical(ExtraSpacing::Step.role())
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    // A failure takes the room left of the buttons and wraps there (`Xv3RZ`).
    .child(match &dialog.error {
      Some(error) => Column::new()
        .id("remove-worktree-error")
        .flex(1.0)
        .child(wrapping_field_error(&message_text(ctx, error)))
        .into(),
      None => Element::from(Spacer::new().flex(1.0)),
    })
    .child(
      ActionButton::new(ctx.t("common.cancel"))
        .id("remove-worktree-cancel")
        .in_form()
        .height(Dimension::Px(BUTTON_HEIGHT))
        .on_click(move || cancel())
        .enabled(!dialog.pending)
        .render(ctx),
    )
    .child(confirm.render(ctx))
    .into()
}
