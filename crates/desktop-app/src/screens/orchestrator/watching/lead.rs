//! How a subscription takes its target's work, on its head row in the `/watching` answer (`Mode · Lead to the end`
//! `ccoKs` in `L02` `HpcQJ`; its menu `Mode menu` `ZHAjU` in `Parts · Ceilings` d, board `Orchestrator autonomy ·
//! frames to approve`; `docs/plan/orchestrator-autonomy/OA-10-…`).
//!
//! A subscription that leads its target to the end shows, before *Stop leading*, a control 22 high in a `border`
//! stroke, radius sm, 7 at its sides, its parts 5 apart: an 11 `flag` in `text_secondary`, *Lead to the end* in 11.5
//! `text_secondary` and an 11 `chevron-down` in `text_muted`; the `hover` fill under the pointer and while its menu is
//! open. The menu hangs 6 under it from its end, 440 wide, as a menu is drawn (`ui::menu`): two rows, 8 inside and
//! their parts 10 apart, a 13 icon, the way's name in 12.5/500 `text_primary` over what it means in 11.5 `text_muted`
//! (3 apart), the current one on `hover` with its icon in `text_primary` and a 13 `check` in `text_secondary`.
//! *Follow* has the conversation only follow the target from now on: what runs goes on, what is parked stays parked
//! and nothing new is started. A subscription that only follows has no such control: leading it is the /watch
//! command's, which starts its ceilings and its budget anew.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::orchestrator::WatchTarget;

use super::LeadChange;
use crate::{
  theme::{
    CEILINGS_FORM_WIDTH, ControlHeight, DISABLED_OPACITY, ExtraColor, ExtraSpacing, ExtraTypography, IconSize,
    LeadingSpacing, LeadingText,
  },
  ui::{
    icon::{Icon, icon},
    menu::{MenuPanel, menu_at_end, menu_panel},
    pressable::{Focus, Press},
  },
};

/// The control of one subscription that leads its target to the end.
#[derive(Clone)]
pub(super) struct ModeProps {
  /// The control's element id; its menu is `{id}-menu`.
  pub(super) id: String,
  pub(super) target: WatchTarget,
  /// The target as people read it, which names the control for tools.
  pub(super) name: Arc<str>,
  /// A change of how a target is taken is under way: the control waits for it.
  pub(super) busy: bool,
  pub(super) change: Arc<dyn Fn(WatchTarget, LeadChange) -> bool + Send + Sync>,
}

/// Compared by what it shows; the change is the screen's.
impl PartialEq for ModeProps {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id && self.target == other.target && self.name == other.name && self.busy == other.busy
  }
}

impl DevtoolsInspectable for ModeProps {}

/// The mode control named by `props`, mounted by its id so its menu keeps whether it is open.
pub(super) fn control(ctx: &mut Ctx, props: ModeProps) -> Element {
  ctx.mount_keyed::<ModeControl>(&props.id.clone(), props)
}

struct ModeControl {
  open: Signal<bool>,
  anchor: ElementRef,
}

impl Component for ModeControl {
  type Props = ModeProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      anchor: ctx.element_ref(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ModeProps>().clone();
    let open = self.open.get();
    let words = ctx.t("orchestrator.lead.toTheEnd");
    let toggle = self.open.clone();
    let control = Focus::Keyboard
      .apply(Press::run(move || toggle.update(|open| *open = !*open)).button())
      .id(props.id.clone())
      .ref_element(self.anchor.clone())
      .describe(
        "a11y_name",
        ctx.t_args(
          "orchestrator.lead.menu.name",
          [("name", &*props.name), ("mode", &*words)],
        ),
      )
      .describe("mode", "lead")
      .describe("expanded", open.to_string())
      .flex_shrink(0.0)
      .height(Dimension::Px(ControlHeight::Helper.value()))
      .padding(Padding::horizontal(LeadingSpacing::ControlSide.role()))
      .spacing(ExtraSpacing::Pair.role())
      .rounded(RadiusSize::Sm)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .hovered_style(Style::new().background(ExtraColor::Hover.role()))
      .child(icon(Icon::Flag, IconSize::Xs, ExtraColor::TextSecondary.role()))
      .child(Text::new(&words).variant(LeadingText::Way.role()).nowrap())
      .child(icon(Icon::ChevronDown, IconSize::Xs, PaletteColor::MutedForeground));
    let control = if open {
      control.background(ExtraColor::Hover.role())
    } else {
      control
    };
    let menu = open.then(|| menu_at_end(self.anchor.clone(), self.open.clone(), self.menu(ctx, &props)));
    let row = Row::new()
      .flex_shrink(0.0)
      .align_items(Alignment::Center)
      .child(control)
      .with_children(menu);
    if props.busy { row.opacity(DISABLED_OPACITY) } else { row }
  }
}

impl ModeControl {
  /// The menu: *Follow*, which has the conversation only follow the target, and *Lead to the end*, the way it is
  /// taken now.
  fn menu(&self, ctx: &Ctx, props: &ModeProps) -> MenuPanel {
    let id = format!("{}-menu", props.id);
    let (change, target) = (props.change.clone(), props.target);
    let follow = item(
      &format!("{id}-follow"),
      (
        Icon::Eye,
        &ctx.t("orchestrator.lead.follow"),
        &ctx.t("orchestrator.lead.menu.follow"),
      ),
      false,
      self.closing(move || {
        // A change that did not start (it is a person's alone) leaves the subscription as the row shows it.
        change(target, LeadChange::Follow);
      }),
    );
    let lead = item(
      &format!("{id}-lead"),
      (
        Icon::Flag,
        &ctx.t("orchestrator.lead.toTheEnd"),
        &ctx.t("orchestrator.lead.menu.lead"),
      ),
      true,
      self.closing(|| {}),
    );
    menu_panel(&id).width(CEILINGS_FORM_WIDTH).child(follow).child(lead)
  }

  /// A press that closes the menu, then does `chosen`.
  fn closing(&self, chosen: impl Fn() + Send + Sync + 'static) -> Press {
    let open = self.open.clone();
    Press::run(move || {
      open.set(false);
      chosen();
    })
  }
}

/// A way's row in the menu, named `id`: its `glyph`, its `name` over what it `means`, and a check while it is the
/// `current` one.
fn item(id: &str, (glyph, name, means): (Icon, &str, &str), current: bool, press: Press) -> Element {
  let tone = if current {
    PaletteColor::Foreground
  } else {
    ExtraColor::TextSecondary.role()
  };
  let words = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Fine.role())
    .child(
      Text::new(name)
        .id(format!("{id}-name"))
        .variant(ExtraTypography::OptionTitle.role())
        .nowrap(),
    )
    .child(
      Text::new(means)
        .id(format!("{id}-means"))
        .variant(ExtraTypography::Note.role())
        .width(Dimension::Pct(100.0)),
    );
  let row = press
    .button()
    .id(id.to_owned())
    .describe("a11y_name", name.to_owned())
    .describe("chosen", current.to_string())
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Sm)
    .spacing(ExtraSpacing::Loose.role())
    .rounded(RadiusSize::Sm)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(icon(glyph, IconSize::Base, tone))
    .child(words)
    .with_children(current.then(|| icon(Icon::Check, IconSize::Base, ExtraColor::TextSecondary.role())));
  if current {
    row.background(ExtraColor::Hover.role()).into()
  } else {
    row.into()
  }
}
