//! The info line of Review › Merges (`Info` `il5fP` in 09 `Os7b3`), 28 high: what the tab lists, and at its end the
//! base branch (`Base` `RNiQG`: a 13 git-branch glyph, *Base*, the branch in mono) and how the checkout stands
//! (`Checkout` `V1b1XV`: a 6 dot, *Checkout*, the branch it has and whether it is clean), 16 apart. While the merges
//! are read (09d `Zlk49`) the lead, naming the checkout's branch, and the names stay; the base, the checkout's dot
//! and its words are bones.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Row, Text},
  layout::Alignment,
  node::dimension::Dimension,
};
use orchester_workspace::merge_queue::BaseState;

use crate::{
  screens::merge_queue::header::{DOT, checkout_words, clean, dot},
  theme::{DeliveryText, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    skeleton::{self, Skeleton, round_bone},
  },
};

/// The info line's height (`il5fP`).
const INFO_HEIGHT: f32 = 28.0;
/// The bones of the base branch and of the checkout's words while they are read (`Zlk49`).
const BASE_BONE: f32 = 48.0;
const CHECKOUT_BONE: f32 = 90.0;

/// The info line with `base`, the branch the checkout has; `None` when it could not be told.
pub(super) fn info(ctx: &Ctx, base: Option<&BaseState>) -> Row {
  let Some(base) = base else {
    return line(&ctx.t("merges.leadNoBranch"));
  };
  line(&ctx.t_args("merges.lead", [("branch", base.branch.as_str())]))
    .child(
      base_part(ctx).child(
        Text::new(base.branch.as_str())
          .id("merges-base")
          .variant(ExtraTypography::PropertyMono.role())
          .color(PaletteColor::TextSecondary)
          .nowrap(),
      ),
    )
    .child(checkout(ctx, base))
}

/// The info line while the merges are read: the lead naming `branch`, the checkout's once git named it, the names,
/// and the values' bones, which pulse after the skeleton's delay as the list's do.
pub(super) fn reading(ctx: &mut Ctx, branch: Option<&str>) -> Row {
  let lead = match branch {
    Some(branch) => ctx.t_args("merges.lead", [("branch", branch)]),
    None => ctx.t("merges.leadNoBranch"),
  };
  let label = ctx.t("merges.reading");
  let (base, state) = (
    skeleton::line(ctx, ExtraTypography::PropertyMono.role(), BASE_BONE),
    skeleton::line(ctx, ExtraTypography::Property.role(), CHECKOUT_BONE),
  );
  let base = Skeleton::new("merges-base-reading", label.clone())
    .inline()
    .show(ctx, base);
  let dot = Skeleton::new("merges-checkout-dot-reading", label.clone())
    .inline()
    .show(ctx, round_bone(DOT));
  let state = Skeleton::new("merges-checkout-reading", label)
    .inline()
    .show(ctx, state);
  line(&lead).child(base_part(ctx).child(base)).child(
    Row::new()
      .flex_shrink(0.0)
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .child(dot)
      .child(name(ctx, "delivery.queue.checkout.key"))
      .child(state),
  )
}

/// The line with the lead, which takes the room the base and the checkout leave.
fn line(lead: &str) -> Row {
  Row::new()
    .id("merges-info")
    .width(Dimension::Pct(100.0))
    .min_height(Dimension::Px(INFO_HEIGHT))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(
      Text::new(lead)
        .id("merges-lead")
        .variant(DeliveryText::Lead.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
}

/// The base's glyph and name, before its branch.
fn base_part(ctx: &Ctx) -> Row {
  Row::new()
    .flex_shrink(0.0)
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::GitBranch, IconSize::Base, PaletteColor::TextMuted))
    .child(name(ctx, "merges.base"))
}

fn name(ctx: &Ctx, key: &str) -> Text {
  Text::new(&ctx.t(key))
    .variant(ExtraTypography::Property.role())
    .color(PaletteColor::TextMuted)
    .nowrap()
}

/// How the checkout stands: its dot, `success` while it keeps nothing out, and its words.
fn checkout(ctx: &Ctx, base: &BaseState) -> Row {
  let clean = clean(base);
  let (state, color) = checkout_words(ctx, base);
  Row::new()
    .id("merges-checkout")
    .flex_shrink(0.0)
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .describe("checkout", if clean { "clean" } else { "not_clean" })
    .child(dot(if clean {
      PaletteColor::Success
    } else {
      PaletteColor::Warning
    }))
    .child(name(ctx, "delivery.queue.checkout.key"))
    .child(
      Text::new(&state)
        .id("merges-checkout-state")
        .variant(ExtraTypography::Property.role())
        .color(color)
        .nowrap(),
    )
}
