//! Next start after Quit now or a crash: "The last session ended before agents could save"
//! (02.E `o8DCd`, dialog `BovYZ`, table `KzyXA`). All or nothing: *Continue all N from
//! checkpoints* or *Don't continue*; rows have no action of their own, and runs not continued
//! stay interrupted until each is continued from its task page. [`StartupRecovery`] finds the
//! runs and continues them; [`RecoveryDialog`] renders the dialog from plain props.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::run::InterruptCause;
use orchester_ipc::SessionEnd;

use crate::{
  display,
  recovery::{InterruptedRun, WorktreeState},
  screens::project::connections::status::short_key as provider_short_key,
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    backdrop::backdrop,
    button::ActionButton,
    field::field_error,
    icon::{Icon, IconSize, icon},
  },
};

/// Dialog width (`BovYZ`), header icon tile (`Ivo7b`), header row and action bar heights
/// (`EkIqH`, `bJD3f`) and the fixed table columns (`ZvJv8`, `LQdz1`, `vAi6i`, `uM7WA`).
const DIALOG_WIDTH: f32 = 1120.0;
const ICON_TILE: f32 = 36.0;
const HEADER_ROW_HEIGHT: f32 = 36.0;
const ACTIONS_HEIGHT: f32 = 76.0;
const DURING_WIDTH: f32 = 150.0;
const REASON_WIDTH: f32 = 180.0;
const CHECKPOINT_WIDTH: f32 = 120.0;
const WORKTREE_WIDTH: f32 = 190.0;

type Handler = Arc<dyn Fn() + Send + Sync>;

/// What the dialog's buttons do.
#[derive(Clone)]
pub struct RecoveryActions {
  pub open_log: Handler,
  pub dont_continue: Handler,
  pub continue_all: Handler,
}

#[derive(Clone)]
pub struct RecoveryDialogProps {
  /// How the previous session ended: Quit now or a crash.
  pub previous: SessionEnd,
  pub runs: Arc<[InterruptedRun]>,
  /// Continue all is under way.
  pub continuing: bool,
  /// Why not every run continued, once that is known.
  pub error: Option<Arc<str>>,
  pub actions: RecoveryActions,
}

impl PartialEq for RecoveryDialogProps {
  fn eq(&self, other: &Self) -> bool {
    self.previous == other.previous
      && self.runs == other.runs
      && self.continuing == other.continuing
      && self.error == other.error
      && Arc::ptr_eq(&self.actions.open_log, &other.actions.open_log)
      && Arc::ptr_eq(&self.actions.dont_continue, &other.actions.dont_continue)
      && Arc::ptr_eq(&self.actions.continue_all, &other.actions.continue_all)
  }
}

impl lurq::app::component::DevtoolsInspectable for RecoveryDialogProps {}

/// The dialog on its backdrop (`Q7Wxwk`).
pub struct RecoveryDialog;

impl Component for RecoveryDialog {
  type Props = RecoveryDialogProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RecoveryDialogProps>().clone();
    let dialog = Column::new()
      .id("recovery-dialog")
      .describe("runs", props.runs.len().to_string())
      .width(Dimension::Px(DIALOG_WIDTH))
      .background(ExtraColor::Surface.role())
      .border_inside(BorderSize::Sm, PaletteColor::Border)
      .rounded(RadiusSize::Lg)
      .child(header(ctx, props.previous, props.runs.len()))
      .child(body(ctx, &props.runs, props.error.as_deref()))
      .child(actions(ctx, &props));
    backdrop(dialog)
  }
}

/// Icon tile, title and explanation (`C0LXL`).
fn header(ctx: &Ctx, previous: SessionEnd, count: usize) -> Element {
  let key = match (previous, count) {
    (SessionEnd::QuitNow, 1) => "lifecycle.recovery.quitNow.one",
    (SessionEnd::QuitNow, _) => "lifecycle.recovery.quitNow.other",
    (_, 1) => "lifecycle.recovery.crashed.one",
    _ => "lifecycle.recovery.crashed.other",
  };
  let count = count.to_string();
  let tile = Row::new()
    .width(Dimension::Px(ICON_TILE))
    .height(Dimension::Px(ICON_TILE))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .background(PaletteColor::SurfaceRaised)
    .rounded(ExtraRadius::Notice.role())
    .child(icon(Icon::TriangleAlert, IconSize::Notice, PaletteColor::TextPrimary));
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding_vertical(SpacingSize::Lg)
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(SpacingSize::Md)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(tile)
    .child(
      Column::new()
        .flex(1.0)
        .spacing(SpacingSize::Sm)
        .child(Text::new(&ctx.t("lifecycle.recovery.title")).variant(ExtraTypography::DialogTitle.role()))
        .child(Text::new(&ctx.t_args(key, [("count", count.as_str())])).variant(ExtraTypography::Explanation.role())),
    )
    .into()
}

/// The table and the note about uncommitted changes (`n8P5P`).
fn body(ctx: &Ctx, runs: &[InterruptedRun], error: Option<&str>) -> Element {
  let table = Column::new()
    .id("recovery-runs")
    .width(Dimension::Pct(100.0))
    .child(heading_row(ctx))
    .with_children(runs.iter().map(|run| run_row(ctx, run)));
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_top(SpacingSize::Md)
    .padding_horizontal(ExtraSpacing::Rail.role())
    .padding_bottom(SpacingSize::Lg)
    .spacing(SpacingSize::Md)
    .child(table)
    .child(
      Text::new(&ctx.t("lifecycle.recovery.note"))
        .variant(ExtraTypography::Detail.role())
        .color(PaletteColor::TextMuted),
    )
    .with_children(error.map(field_error))
    .into()
}

fn cells(content: Row) -> Row {
  content
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
}

/// Column headings (`EkIqH`).
fn heading_row(ctx: &Ctx) -> Element {
  let heading = |key: &str, width: Option<f32>| {
    let text = Text::new(&ctx.t(key))
      .variant(ExtraTypography::FileListHeading.role())
      .color(PaletteColor::TextMuted)
      .nowrap();
    let cell = Row::new().align_items(Alignment::Center).child(text);
    match width {
      Some(width) => cell.width(Dimension::Px(width)),
      None => cell.flex(1.0),
    }
  };
  cells(Row::new())
    .height(Dimension::Px(HEADER_ROW_HEIGHT))
    .align_items(Alignment::Center)
    .child(heading("lifecycle.recovery.columns.task", None))
    .child(heading("lifecycle.recovery.columns.during", Some(DURING_WIDTH)))
    .child(heading("lifecycle.recovery.columns.reason", Some(REASON_WIDTH)))
    .child(heading("lifecycle.recovery.columns.checkpoint", Some(CHECKPOINT_WIDTH)))
    .child(heading("lifecycle.recovery.columns.worktree", Some(WORKTREE_WIDTH)))
    .into()
}

fn stacked(width: Option<f32>, top: impl Into<Element>, bottom: Option<Element>) -> Element {
  let column = Column::new().spacing(SpacingSize::Xs).child(top).with_children(bottom);
  match width {
    Some(width) => column.width(Dimension::Px(width)),
    None => column.flex(1.0),
  }
  .into()
}

fn caption(text: &str) -> Element {
  Text::new(text).variant(TypographyStyle::Caption).into()
}

/// One interrupted run (`ZZBQa`): task, where it was interrupted, why, its last checkpoint and
/// what its worktree holds.
fn run_row(ctx: &Ctx, run: &InterruptedRun) -> Element {
  let number = run.number.to_string();
  let project_run = ctx.t_args(
    "lifecycle.recovery.projectRun",
    [("project", &*run.project), ("number", number.as_str())],
  );
  let title = Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(&run.task_key)
        .variant(TypographyStyle::Mono)
        .color(PaletteColor::TextSecondary),
    )
    .child(
      Text::new(&run.task_title)
        .variant(ExtraTypography::NoticeHeading.role())
        .flex(1.0),
    );
  cells(Row::new())
    .id(format!("recovery-run-{}", run.run))
    .padding_vertical(ExtraSpacing::Group.role())
    .child(stacked(None, title, Some(caption(&project_run))))
    .child(during(ctx, run))
    .child(stacked(
      Some(REASON_WIDTH),
      Text::new(&ctx.t(reason_key(run.cause))).variant(ExtraTypography::Detail.role()),
      None,
    ))
    .child(checkpoint(ctx, run))
    .child(worktree(ctx, run.worktree))
    .into()
}

/// Step, then agent role · provider · model.
fn during(ctx: &Ctx, run: &InterruptedRun) -> Element {
  let Some(step) = &run.step else {
    return stacked(Some(DURING_WIDTH), caption(&ctx.t("lifecycle.recovery.noStep")), None);
  };
  let agent = match (&run.agent, run.provider, &run.model) {
    (Some(agent), Some(provider), Some(model)) => Some(caption(&ctx.t_args(
      "lifecycle.recovery.agent",
      [
        ("agent", &**agent),
        ("provider", &*ctx.t(provider_short_key(provider))),
        ("model", &**model),
      ],
    ))),
    _ => None,
  };
  stacked(
    Some(DURING_WIDTH),
    Text::new(step)
      .variant(TypographyStyle::FieldLabel)
      .color(PaletteColor::TextPrimary),
    agent,
  )
}

fn checkpoint(ctx: &Ctx, run: &InterruptedRun) -> Element {
  match &run.checkpoint {
    Some((revision, at)) => stacked(
      Some(CHECKPOINT_WIDTH),
      Text::new(revision).variant(ExtraTypography::MonoValue.role()),
      Some(caption(&ctx.t_args(
        "lifecycle.recovery.checkpointSaved",
        [("time", display::clock(*at).as_str())],
      ))),
    ),
    None => stacked(
      Some(CHECKPOINT_WIDTH),
      caption(&ctx.t("lifecycle.recovery.noCheckpoint")),
      None,
    ),
  }
}

/// "3 files not committed" or "No uncommitted changes" (`DP204`, `Bph2U`). The design's *View
/// changes* link opens the Changes tab, which does not exist yet, so it is not shown.
fn worktree(ctx: &Ctx, state: WorktreeState) -> Element {
  let (glyph, text, color) = match state {
    WorktreeState::Clean => (
      Icon::Check,
      ctx.t("lifecycle.recovery.worktree.clean"),
      PaletteColor::TextSecondary,
    ),
    WorktreeState::Uncommitted(files) => {
      let key = if files == 1 {
        "lifecycle.recovery.worktree.uncommitted.one"
      } else {
        "lifecycle.recovery.worktree.uncommitted.other"
      };
      let files = files.to_string();
      (
        Icon::FileDiff,
        ctx.t_args(key, [("count", files.as_str())]),
        PaletteColor::TextPrimary,
      )
    }
    WorktreeState::Unknown => (
      Icon::AlertCircle,
      ctx.t("lifecycle.recovery.worktree.unknown"),
      PaletteColor::TextSecondary,
    ),
  };
  let state = Row::new()
    .spacing(ExtraSpacing::Tight.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Small, color.clone()))
    .child(Text::new(&text).variant(TypographyStyle::Caption).color(color));
  stacked(Some(WORKTREE_WIDTH), state, None)
}

const fn reason_key(cause: InterruptCause) -> &'static str {
  match cause {
    InterruptCause::QuitNow => "lifecycle.recovery.reason.quitNow",
    InterruptCause::Crashed => "lifecycle.recovery.reason.crashed",
    InterruptCause::RunnerLost => "lifecycle.recovery.reason.runnerLost",
  }
}

/// Open runner log on the left; Don't continue and Continue all on the right (`bJD3f`).
fn actions(ctx: &Ctx, props: &RecoveryDialogProps) -> Element {
  let (log, skip, resume) = (
    props.actions.open_log.clone(),
    props.actions.dont_continue.clone(),
    props.actions.continue_all.clone(),
  );
  let count = props.runs.len().to_string();
  let continue_label = if props.continuing {
    ctx.t("lifecycle.recovery.continuing")
  } else if props.runs.len() == 1 {
    ctx.t("lifecycle.recovery.continueAll.one")
  } else {
    ctx.t_args("lifecycle.recovery.continueAll.other", [("count", count.as_str())])
  };
  let continue_all = ActionButton::new(continue_label)
    .primary()
    .icon(if props.continuing { Icon::Loader } else { Icon::Play })
    .id("recovery-continue-all")
    .in_form();
  let continue_all = if props.continuing {
    continue_all.disabled()
  } else {
    continue_all.on_click(move || resume())
  };
  let dont_continue = ActionButton::new(ctx.t("lifecycle.recovery.dontContinue"))
    .icon(Icon::X)
    .id("recovery-dont-continue")
    .in_form();
  // Once continuing has begun, the answer is given.
  let dont_continue = if props.continuing {
    dont_continue.disabled()
  } else {
    dont_continue.on_click(move || skip())
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ACTIONS_HEIGHT))
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(
      ActionButton::new(ctx.t("runner.connection.openLog"))
        .icon(Icon::FileText)
        .id("recovery-open-log")
        .in_form()
        .on_click(move || log())
        .render(ctx),
    )
    .child(Spacer::new().flex(1.0))
    .child(dont_continue.render(ctx))
    .child(continue_all.render(ctx))
    .into()
}
