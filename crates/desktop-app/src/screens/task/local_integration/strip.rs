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
  theme::{ExtraSpacing, ExtraTypography, LocalText, mono_space},
  ui::tooltip::Truncated,
};

/// The strip's thirds, the route's first.
const CELLS: [&str; 3] = [
  "local-identity-route-cell",
  "local-identity-path-cell",
  "local-identity-head-cell",
];
/// The widest the route's third grows to show it whole; a longer route is cut too.
const ROUTE_MOST: f32 = 360.0;

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
  // Geist Mono advances every glyph alike, so the route's width is its length.
  let route_width = (route.chars().count() as f32 * mono_space(ctx.theme(), TypographyStyle::Mono)).min(ROUTE_MOST);
  let [route_cell, path, head] = [
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
  let gap = ctx.theme().spacing_value(SpacingSize::Lg).to_px();
  // A third each, the gap after the first two inside them. The route's third widens to show it whole, up to
  // `ROUTE_MOST`, and the folder and the head share what is left (`s4gj9`).
  let route_room = Row::new()
    .id(CELLS[0])
    .width(Dimension::Pct(100.0 / 3.0))
    .min_width(Dimension::Px(route_width + gap))
    .padding_right(SpacingSize::Lg)
    .child(route_cell);
  let rest = |id: &str, cell: Element| Row::new().id(id).flex(1.0).min_width(Dimension::Px(0.0)).child(cell);
  Row::new()
    .id("local-identities")
    .width(Dimension::Pct(100.0))
    .padding(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .rounded(RadiusSize::Md)
    .child(route_room)
    .child(rest(CELLS[1], path).padding_right(SpacingSize::Lg))
    .child(rest(CELLS[2], head))
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
