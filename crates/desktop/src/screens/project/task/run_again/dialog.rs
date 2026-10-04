//! The confirmation of *Run again* (ORC-16 `R5y5x`), in the frame of the app's confirmations (02.Q2
//! `g1HS8`): a title and what starting does, what the new run uses (`tfGRt` rows), the switch
//! that removes the worktrees of the run before as *Remove worktree* does, naming the uncommitted
//! files it would discard (02.Q), and Cancel and Start.

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
use orchester_core::{
  integration::{Leftovers, WORKTREES_DIR},
  tokens::compact_tokens,
};

use crate::{
  budget::whole_minutes,
  display,
  messages::Message,
  tasks::{
    detail::{RunView, TaskDetail},
    integration::file_list,
    run_again::NextRun,
  },
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    backdrop::backdrop,
    button::ActionButton,
    field::wrapping_field_error,
    form::{Submit, form},
    icon::Icon,
    message_text,
    property::{ValueStyle, property, styled_property},
    switch::Switch,
  },
};

/// Dialog width, action bar and button heights (`C2CPbs`, `f0IZN2`).
const DIALOG_WIDTH: f32 = 600.0;
const ACTIONS_HEIGHT: f32 = 76.0;
const BUTTON_HEIGHT: f32 = 36.0;

pub(super) struct Dialog<'a> {
  pub(super) detail: &'a TaskDetail,
  /// The run before, which failed or was cancelled.
  pub(super) run: &'a RunView,
  /// The name of the task's pipeline.
  pub(super) pipeline: &'a str,
  pub(super) next: &'a NextRun,
  /// What the run before left, once read.
  pub(super) leftovers: Option<&'a Leftovers>,
  pub(super) remove: bool,
  pub(super) flip_remove: Arc<dyn Fn() + Send + Sync>,
  pub(super) pending: bool,
  /// Why the runner did not start the run.
  pub(super) errors: Vec<Message>,
  /// Starts; `None` while the request is under way.
  pub(super) confirm: Submit,
  pub(super) cancel: Arc<dyn Fn() + Send + Sync>,
}

pub(super) fn dialog(ctx: &mut Ctx, dialog: &Dialog) -> Element {
  let content = Column::new()
    .id("run-again-dialog")
    .width(Dimension::Px(DIALOG_WIDTH))
    .background(ExtraColor::Surface.role())
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .child(header(ctx, dialog))
    .child(properties(ctx, dialog))
    .with_children(option(ctx, dialog))
    .child(actions(ctx, dialog));
  backdrop(ctx, form(dialog.confirm.clone(), content))
}

fn header(ctx: &Ctx, dialog: &Dialog) -> Element {
  let number = dialog.next.number.get().to_string();
  let previous = dialog.run.run.number().get().to_string();
  let key = dialog.detail.task.key().to_string();
  let explanation = match dialog.next.base.as_ref().and_then(|base| base.branch.as_ref()) {
    Some(branch) => ctx.t_args(
      "runs.runAgain.explanation.branch",
      [
        ("run", number.as_str()),
        ("base", branch.as_str()),
        ("previous", previous.as_str()),
      ],
    ),
    None => ctx.t_args(
      "runs.runAgain.explanation.head",
      [("run", number.as_str()), ("previous", previous.as_str())],
    ),
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_vertical(SpacingSize::Lg)
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(SpacingSize::Sm)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(
      Text::new(&ctx.t_args(
        "runs.runAgain.title",
        [("run", number.as_str()), ("task", key.as_str())],
      ))
      .variant(ExtraTypography::DialogTitle.role()),
    )
    .child(Text::new(&explanation).variant(ExtraTypography::Explanation.role()))
    .into()
}

/// What the new run uses (`l7d9Ug`).
fn properties(ctx: &Ctx, dialog: &Dialog) -> Element {
  let (next, run) = (dialog.next, dialog.run);
  let version = next.version.to_string();
  let previous_version = run.version.to_string();
  let previous = run.run.number().get().to_string();
  let steps = next.steps.to_string();
  let (budget, minutes) = (
    compact_tokens(next.limits.budget().get()),
    whole_minutes(next.limits.max_duration()).to_string(),
  );
  let base = match &next.base {
    Some(base) => {
      let revision = display::short_revision(base.revision.as_str());
      let text = match &base.branch {
        Some(branch) => ctx.t_args(
          "runs.runAgain.property.baseValue",
          [("branch", branch.as_str()), ("revision", revision)],
        ),
        None => ctx.t_args("runs.runAgain.property.baseDetached", [("revision", revision)]),
      };
      styled_property(&ctx.t("runs.runAgain.property.base"), &text, ValueStyle::Mono)
    }
    None => styled_property(
      &ctx.t("runs.runAgain.property.base"),
      &ctx.t("runs.runAgain.property.baseUnknown"),
      ValueStyle::Pending,
    ),
  };
  Column::new()
    .id("run-again-properties")
    .width(Dimension::Pct(100.0))
    .padding_vertical(ExtraSpacing::Step.role())
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(ExtraSpacing::Stack.role())
    .child(property(
      &ctx.t("runs.runAgain.property.pipeline"),
      &ctx.t_args(
        "runs.runAgain.property.pipelineValue",
        [("pipeline", dialog.pipeline), ("version", version.as_str())],
      ),
    ))
    .child(property(
      &ctx.t_args("runs.runAgain.property.previous", [("run", previous.as_str())]),
      &ctx.t_args(
        "runs.runAgain.property.previousValue",
        [("pipeline", &*run.pipeline), ("version", previous_version.as_str())],
      ),
    ))
    .child(property(
      &ctx.t("runs.runAgain.property.startsFrom"),
      &ctx.t_args(
        "runs.runAgain.property.startsFromValue",
        [("step", &*next.first_step), ("count", steps.as_str())],
      ),
    ))
    .child(base)
    .child(styled_property(
      &ctx.t("runs.runAgain.property.branch"),
      next.branch.as_str(),
      ValueStyle::Mono,
    ))
    .child(property(
      &ctx.t("runs.runAgain.property.worktree"),
      &ctx.t_args("runs.runAgain.property.worktreeValue", [("folder", WORKTREES_DIR)]),
    ))
    .child(property(
      &ctx.t("runs.runAgain.property.limits"),
      &ctx.t_args(
        "runs.runAgain.property.limitsValue",
        [("budget", budget.as_str()), ("minutes", minutes.as_str())],
      ),
    ))
    .into()
}

/// *Remove the worktree of run #N* (`SmhER`), while the run before has worktrees.
fn option(ctx: &Ctx, dialog: &Dialog) -> Option<Element> {
  let leftovers = dialog.leftovers.filter(|left| !left.worktrees.is_empty())?;
  let run = &dialog.run.run;
  let number = run.number().get().to_string();
  let several = leftovers.worktrees.len() > 1;
  let pick = |one, other| if several { other } else { one };
  let title = ctx.t_args(
    pick("runs.runAgain.remove.title.one", "runs.runAgain.remove.title.other"),
    [("run", number.as_str())],
  );
  let files: Vec<String> = leftovers
    .worktrees
    .iter()
    .flat_map(|worktree| worktree.uncommitted.iter().cloned())
    .collect();
  let kept = leftovers
    .branches
    .iter()
    .any(|branch| branch.name == *run.branch() && !branch.merged);
  // A folder git forgot that holds more than it lists loses files even when none are listed.
  let cut = leftovers.listing_cut();
  let note = match files.len() {
    0 if !cut && kept => ctx.t_args(
      pick(
        "runs.runAgain.remove.cleanKept.one",
        "runs.runAgain.remove.cleanKept.other",
      ),
      [("branch", run.branch().as_str())],
    ),
    0 if !cut => ctx.t(pick(
      "runs.runAgain.remove.clean.one",
      "runs.runAgain.remove.clean.other",
    )),
    count => {
      let listed = file_list(&files, &ctx.t("common.listSeparator"));
      let key = match count {
        _ if cut => "runs.runAgain.remove.uncommitted.more",
        1 => "runs.runAgain.remove.uncommitted.one",
        _ => "runs.runAgain.remove.uncommitted.other",
      };
      ctx.t_args(key, [("count", count.to_string().as_str()), ("files", listed.as_str())])
    }
  };
  let flip = dialog.flip_remove.clone();
  Some(
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding_top(SpacingSize::Md)
      .padding_horizontal(ExtraSpacing::Rail.role())
      .padding_bottom(SpacingSize::Lg)
      .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(
        Switch::new("run-again-remove", title, dialog.remove)
          .note(note)
          .render(move || flip()),
      )
      .into(),
  )
}

fn actions(ctx: &Ctx, dialog: &Dialog) -> Element {
  let number = dialog.next.number.get().to_string();
  let (label, glyph) = if dialog.pending {
    (ctx.t("runs.runAgain.starting"), Icon::Loader)
  } else {
    (
      ctx.t_args("runs.runAgain.start", [("run", number.as_str())]),
      Icon::Play,
    )
  };
  let confirm = ActionButton::new(label)
    .primary()
    .icon(glyph)
    .id("run-again-confirm")
    .in_form()
    .height(Dimension::Px(BUTTON_HEIGHT));
  let confirm = match dialog.confirm.clone() {
    Some(start) => confirm.on_click(move || start()),
    None => confirm.on_click(|| {}).enabled(false),
  };
  let cancel = dialog.cancel.clone();
  let problems = (!dialog.errors.is_empty()).then(|| {
    Column::new()
      .id("run-again-error")
      .flex(1.0)
      .spacing(SpacingSize::Xs)
      .with_children(
        dialog
          .errors
          .iter()
          .map(|error| wrapping_field_error(&message_text(ctx, error))),
      )
      .into()
  });
  Row::new()
    .width(Dimension::Pct(100.0))
    // 76 high as drawn; a reason longer than two lines makes it taller rather than being cut off.
    .min_height(Dimension::Px(ACTIONS_HEIGHT))
    .padding_vertical(ExtraSpacing::Step.role())
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(problems.unwrap_or_else(|| Element::from(Spacer::new().flex(1.0))))
    .child(
      ActionButton::new(ctx.t("common.cancel"))
        .id("run-again-cancel")
        .in_form()
        .height(Dimension::Px(BUTTON_HEIGHT))
        .on_click(move || cancel())
        .enabled(!dialog.pending)
        .render(ctx),
    )
    .child(confirm.render(ctx))
    .into()
}
