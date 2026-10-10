//! Next start after Quit now or a crash: "The last session ended before agents could save"
//! (02.E `o8DCd`, dialog `BovYZ`, table `KzyXA`). All or nothing: *Continue all N from
//! checkpoints* or *Don't continue*; rows have no action of their own, and runs not continued
//! stay interrupted until each is continued from its task page. A run with uncommitted changes
//! links to them (*View changes*). [`StartupRecovery`] finds the runs and continues them;
//! [`RecoveryDialog`] renders the dialog from plain props.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text, TextOverflow},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, Style, border::Border, dimension::Dimension},
};
use orchester_core::{id::ProjectId, run::InterruptCause};
use orchester_ipc::SessionEnd;

use crate::{
  display,
  recovery::{InterruptedRun, WorktreeState},
  screens::project::connections::status::short_key as provider_short_key,
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography, LIFECYCLE_DIALOG_WIDTH},
  ui::{
    backdrop::backdrop,
    button::ActionButton,
    field::field_error,
    icon::{Icon, IconSize, icon},
    pressable::{pressable, surface},
  },
};

/// Header icon tile (`Ivo7b`), header row and action bar heights (`EkIqH`, `bJD3f`) and the
/// fixed table columns (`ZvJv8`, `LQdz1`, `vAi6i`, `uM7WA`); the task column takes the rest.
/// The worktree state's icon sits in a slot as high as the first line of its label (`Ar3RO`).
const ICON_TILE: f32 = 36.0;
const STATE_ICON_SLOT: f32 = 16.0;
const HEADER_ROW_HEIGHT: f32 = 36.0;
const ACTIONS_HEIGHT: f32 = 76.0;
const DURING_WIDTH: f32 = 140.0;
const REASON_WIDTH: f32 = 150.0;
const CHECKPOINT_WIDTH: f32 = 88.0;
const WORKTREE_WIDTH: f32 = 160.0;

type Handler = Arc<dyn Fn() + Send + Sync>;

/// What the dialog's buttons and links do.
#[derive(Clone)]
pub struct RecoveryActions {
  pub open_log: Handler,
  pub dont_continue: Handler,
  pub continue_all: Handler,
  /// Opens the run's changes on its task page.
  pub view_changes: Arc<dyn Fn(&InterruptedRun) + Send + Sync>,
}

#[derive(Clone)]
pub struct RecoveryDialogProps {
  /// How the previous session ended: Quit now or a crash.
  pub previous: SessionEnd,
  pub runs: Arc<[InterruptedRun]>,
  /// The project the window shows, once known. *View changes* is offered for its runs only:
  /// there is no switching to another project yet.
  pub shown_project: Option<ProjectId>,
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
      && self.shown_project == other.shown_project
      && self.continuing == other.continuing
      && self.error == other.error
      && Arc::ptr_eq(&self.actions.open_log, &other.actions.open_log)
      && Arc::ptr_eq(&self.actions.dont_continue, &other.actions.dont_continue)
      && Arc::ptr_eq(&self.actions.continue_all, &other.actions.continue_all)
      && Arc::ptr_eq(&self.actions.view_changes, &other.actions.view_changes)
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
      .width(Dimension::Pct(100.0))
      .max_width(Dimension::Px(LIFECYCLE_DIALOG_WIDTH))
      .background(ExtraColor::Surface.role())
      .border_inside(BorderSize::Sm, PaletteColor::Border)
      .rounded(RadiusSize::Lg)
      .child(header(ctx, props.previous, props.runs.len()))
      .child(body(ctx, &props))
      .child(actions(ctx, &props));
    backdrop(ctx, dialog)
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
fn body(ctx: &Ctx, props: &RecoveryDialogProps) -> Element {
  let table = Column::new()
    .id("recovery-runs")
    .width(Dimension::Pct(100.0))
    .child(heading_row(ctx))
    .with_children(props.runs.iter().map(|run| run_row(ctx, run, props)));
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
    .with_children(props.error.as_deref().map(field_error))
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
fn run_row(ctx: &Ctx, run: &InterruptedRun, props: &RecoveryDialogProps) -> Element {
  let number = run.number.to_string();
  let project_run = ctx.t_args(
    "lifecycle.recovery.projectRun",
    [("project", &*run.project), ("number", number.as_str())],
  );
  let title = Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(&run.task_key.to_string())
        .variant(TypographyStyle::Mono)
        .color(PaletteColor::TextSecondary),
    )
    .child(
      Text::new(&run.task_title)
        .variant(ExtraTypography::NoticeHeading.role())
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex(1.0),
    );
  cells(Row::new())
    .id(format!("recovery-run-{}", run.run))
    .padding_vertical(ExtraSpacing::Group.role())
    .child(stacked(
      None,
      title,
      Some(
        Text::new(&project_run)
          .variant(TypographyStyle::Caption)
          .nowrap()
          .text_overflow(TextOverflow::Elipsis)
          .into(),
      ),
    ))
    .child(during(ctx, run))
    .child(stacked(
      Some(REASON_WIDTH),
      Text::new(&ctx.t(reason_key(run.cause))).variant(ExtraTypography::Detail.role()),
      None,
    ))
    .child(checkpoint(ctx, run))
    .child(worktree(ctx, run, view_changes(ctx, run, props)))
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

/// "3 files not committed" with *View changes* under it, "No uncommitted changes" or "Could not
/// read the worktree" (`BwjS0`, `Bv9Wc`, `Py65X`).
fn worktree(ctx: &Ctx, run: &InterruptedRun, link: Option<Element>) -> Element {
  let (glyph, text, color) = match run.worktree {
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
  // A label longer than the column wraps beside the icon instead of being cut; the icon stays
  // with its first line.
  let slot = Row::new()
    .id(format!("recovery-worktree-icon-{}", run.run))
    .height(Dimension::Px(STATE_ICON_SLOT))
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Small, color.clone()));
  let state = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Tight.role())
    .align_items(Alignment::Start)
    .child(slot)
    .child(
      Text::new(&text)
        .id(format!("recovery-worktree-label-{}", run.run))
        .variant(ExtraTypography::CellLabel.role())
        .color(color)
        .flex(1.0),
    );
  stacked(Some(WORKTREE_WIDTH), state, link)
}

/// *View changes →* (`O5D84`): the run's task page on its Changes tab, which shows what the
/// worktree holds beyond the checkpoint. Only for a run of the project the window shows.
fn view_changes(ctx: &Ctx, run: &InterruptedRun, props: &RecoveryDialogProps) -> Option<Element> {
  let uncommitted = matches!(run.worktree, WorktreeState::Uncommitted(_));
  if !uncommitted || props.shown_project != Some(run.project_id) {
    return None;
  }
  let (view, target) = (props.actions.view_changes.clone(), run.clone());
  let link = pressable(surface().id(format!("recovery-view-changes-{}", run.run)), move || {
    view(&target);
  })
  .rounded(RadiusSize::Sm)
  .hovered_style(Style::new().background(PaletteColor::AccentMuted))
  .child(Text::new(&ctx.t("lifecycle.recovery.viewChanges")).variant(TypographyStyle::Caption));
  Some(link.into())
}

const fn reason_key(cause: InterruptCause) -> &'static str {
  match cause {
    InterruptCause::QuitNow => "lifecycle.recovery.reason.quitNow",
    InterruptCause::Crashed => "lifecycle.recovery.reason.crashed",
    InterruptCause::RunnerLost => "lifecycle.recovery.reason.runnerLost",
    InterruptCause::RunnerFailed => "lifecycle.recovery.reason.runnerFailed",
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
