//! Over the page: the exact checkout identities on one raised strip, its three parts in equal thirds and each cut
//! to its room with its full value in a tooltip (`Exact checkout identities` `T3BGvO`), and the phases with the
//! one under way after them (`Current phase` `C68HJ`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::integration::local::LocalPhase;
use orchester_workspace::{display, tasks::local_integration::IntegrationView};

use super::Mode;
use crate::{
  theme::{ExtraSpacing, ExtraTypography, LocalText},
  ui::tooltip::Truncated,
};

/// The run's branch into the checkout's, the checkout's folder, and its branch at its commit.
pub(super) fn identities(ctx: &mut Ctx, source: &str, view: &IntegrationView) -> Element {
  let checkout = &view.checkout;
  let route = ctx.t_args(
    "local_integration.route",
    [("source", source), ("target", checkout.branch.as_str())],
  );
  let head = ctx.t_args(
    "local_integration.targetHead",
    [
      ("branch", checkout.branch.as_str()),
      ("head", display::short_revision(&checkout.head)),
    ],
  );
  let cells = [
    Truncated::new("local-identity-route", route, TypographyStyle::Mono)
      .mono()
      .render(ctx),
    Truncated::new(
      "local-identity-path",
      display::plain_path(&checkout.root),
      LocalText::Note.role(),
    )
    .mono()
    .render(ctx),
    Truncated::new("local-identity-head", head, LocalText::Note.role())
      .mono()
      .render(ctx),
  ];
  Row::new()
    .id("local-identities")
    .width(Dimension::Pct(100.0))
    .padding(ExtraSpacing::Loose.role())
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .rounded(RadiusSize::Md)
    .with_children(
      cells
        .into_iter()
        .map(|cell| Row::new().flex(1.0).min_width(Dimension::Px(0.0)).child(cell)),
    )
    .into()
}

/// The phases of local integration, then the one under way.
pub(super) fn trail(ctx: &Ctx, view: &IntegrationView, mode: &Mode) -> Element {
  let current = ctx.t(current(view, mode));
  Text::new(&ctx.t_args("local_integration.trail", [("current", &*current)]))
    .id("local-phase-trail")
    .variant(ExtraTypography::NoteStrong.role())
    .into()
}

fn current(view: &IntegrationView, mode: &Mode) -> &'static str {
  if view.external.is_some() {
    return "local_integration.now.blocked";
  }
  match mode {
    Mode::Commit { .. } => return "local_integration.now.commit",
    Mode::Preserve => return "local_integration.now.preserve",
    Mode::Abort => return "local_integration.now.abort",
    Mode::Cleanup => return "local_integration.now.cleanup",
    Mode::Cancel => return "local_integration.now.cancel",
    Mode::Review => {}
  }
  let Some(operation) = &view.operation else {
    return "local_integration.now.review";
  };
  match operation.phase() {
    LocalPhase::Review if operation.commit_tree().is_some() => "local_integration.now.remaining",
    LocalPhase::Review => "local_integration.now.review",
    LocalPhase::Committing | LocalPhase::Preserving => "local_integration.now.preparing",
    LocalPhase::Prepared | LocalPhase::Syncing | LocalPhase::Checking => "local_integration.now.check",
    LocalPhase::AwaitingApproval | LocalPhase::Merging => "local_integration.now.merge",
    LocalPhase::TaskConflicts => "local_integration.now.conflict",
    LocalPhase::MergedAwaitingRestore | LocalPhase::Restoring | LocalPhase::RestoreConflicts => {
      "local_integration.now.restore"
    }
    LocalPhase::RestorationVerified | LocalPhase::RemovingPreservation | LocalPhase::Complete => {
      "local_integration.now.verified"
    }
    LocalPhase::Paused | LocalPhase::NeedsReconciliation => "local_integration.now.recovery",
    LocalPhase::Canceling | LocalPhase::Canceled => "local_integration.now.cancel",
  }
}
