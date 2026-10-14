//! The interrupted runs' list in place (`JSRBO`): why they were interrupted, the runs, what is kept, what
//! could not be read, and the decision: *Open runner log* on the left, *Don't continue* and *Continue all N
//! from checkpoints* on the right. While the runs continue, the primary turns and nothing else can be
//! decided.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::recovery::{InterruptedRun, RecoveryState};

use super::{NOTICE_ID, RecoveryNotice, body_key, continue_all, notices::unread_lines, plural, table, title};
use crate::{
  project::ShownProject,
  services::Services,
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    pressable::pressable,
  },
};

impl RecoveryNotice {
  pub(super) fn list(
    &self,
    ctx: &mut Ctx,
    (shown, state): (&ShownProject, &RecoveryState),
    runs: &[InterruptedRun],
    continuing: bool,
  ) -> Element {
    let services = Services::of(ctx);
    let project = shown.open.project.id();
    let recovery = services.recovery.clone();
    let head = list_head(ctx, runs).child(hide(ctx, move || recovery.review(project, false)));
    let table = table::runs(ctx, project, runs);
    let unread = unread_lines(ctx, &state.unread_of(project));
    let note = Text::new(&ctx.t("lifecycle.recovery.note"))
      .variant(ExtraTypography::Prose.role())
      .width(Dimension::Pct(100.0));
    Column::new()
      .id(NOTICE_ID)
      .describe("state", if continuing { "continuing" } else { "open" })
      .width(Dimension::Pct(100.0))
      .padding(SpacingSize::Lg)
      .spacing(SpacingSize::Md)
      .background(PaletteColor::WarningMuted)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Warning))
      .rounded(RadiusSize::Lg)
      .child(head)
      .child(table)
      .with_children(unread.iter().enumerate().map(|(index, line)| {
        Text::new(line)
          .id(format!("recovery-unread-{index}"))
          .variant(ExtraTypography::Prose.role())
          .width(Dimension::Pct(100.0))
      }))
      .child(note)
      .child(decision(ctx, (&services, shown), runs.len(), continuing))
      .into()
  }
}

/// The list's title and why the runs were interrupted.
fn list_head(ctx: &Ctx, runs: &[InterruptedRun]) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Start)
    .child(icon(Icon::TriangleAlert, IconSize::Lg, PaletteColor::Warning))
    .child(
      Column::new()
        .flex(1.0)
        .spacing(ExtraSpacing::Tight.role())
        .child(
          Text::new(&title(ctx, runs.len()))
            .id(format!("{NOTICE_ID}-title"))
            .variant(ExtraTypography::Strong.role()),
        )
        .child(Text::new(&ctx.t(body_key(runs))).variant(ExtraTypography::Prose.role())),
    )
}

/// *Hide* with its chevron: closes the list back to the compact notice.
fn hide(ctx: &Ctx, close: impl Fn() + Send + Sync + 'static) -> Element {
  pressable(close)
    .id("recovery-hide")
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .child(icon(Icon::ChevronUp, IconSize::Sm, PaletteColor::TextSecondary))
    .child(
      Text::new(&ctx.t("lifecycle.recovery.hide"))
        .variant(ExtraTypography::Prose.role())
        .nowrap(),
    )
    .into()
}

fn decision(ctx: &Ctx, (services, shown): (&Services, &ShownProject), count: usize, continuing: bool) -> Row {
  let project = shown.open.project.id();
  let root = shown.open.repo_root.clone();
  let runners = services.runners.clone();
  let row = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      ActionButton::new(
        "recovery-open-log",
        ButtonKind::Secondary,
        ctx.t("lifecycle.recovery.openLog"),
      )
      .icon(Icon::FileText)
      .on_click(move || runners.open_log_of(&root)),
    )
    .child(Spacer::new().flex(1.0));
  let dont = ActionButton::new(
    "recovery-dont-continue",
    ButtonKind::Secondary,
    ctx.t("lifecycle.recovery.dontContinue"),
  );
  let words = |one, other| ctx.t_args(plural(count, one, other), [("count", count.to_string())]);
  if continuing {
    let label = words(
      "lifecycle.recovery.continuing.one",
      "lifecycle.recovery.continuing.other",
    );
    return row
      .child(dont.disabled())
      .child(ActionButton::new("recovery-continue", ButtonKind::Secondary, label).loading());
  }
  let recovery = services.recovery.clone();
  let label = words(
    "lifecycle.recovery.continueFromCheckpoints.one",
    "lifecycle.recovery.continueFromCheckpoints.other",
  );
  row.child(dont.on_click(move || recovery.leave(project))).child(
    ActionButton::new("recovery-continue", ButtonKind::Primary, label)
      .icon(Icon::Play)
      .on_click(continue_all(services, project)),
  )
}
