//! One Leads chip (`Watch · Webhooks v2` `VFXga` and `Watch · KON-45` `rgeO4` in `X93D8T` b; under the pointer
//! `dRw23` in d): a 22 high round pill on `surface_raised` in a `border` stroke, 8 at its sides and 5 between its
//! parts, an 11 icon in `text_secondary` (a module's `boxes`, a task's `square-check`), the target's name, a task by
//! its key, in 11.5 `text_secondary`, and always an 11 `x` in `text_muted` ([`super::stop`]), so the pointer never
//! changes its width. A press opens the module or the task; a press on the `x` stops that subscription instead. Under
//! the pointer it is on `hover` in `border_strong`, its name in `text_primary` and its `x` in `text_secondary`.
//!
//! Once the pointer rested on it as long as a tooltip waits, its tooltip hangs 8 under the header, its start at the
//! chip's start (`Tooltip · subscription` `nzz9r` in `X93D8T` d): `text_primary`, radius 4, 6 over and under and 8 at
//! the sides, its lines 2 apart in `text_inverse` 11: the name, kind and since when at 600, the order in a 320 wide
//! column at 1.45, and the hint at 70 % — or why the last stop was not applied.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, Timeout},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Button, Column, Placement, Popup, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, HitTestBehavior, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  orchestrator::{WatchTarget, watch::NamedTarget},
  time::Timestamp,
};
use orchester_workspace::{
  display,
  messages::Message,
  orchestrator::watching::{LeadChip, WatchOrder},
};

use super::{CHIP_HEIGHT, Fit, stop::stop_button};
use crate::{
  navigation::Navigation,
  routes::Route,
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, IconSize, LEADS_HINT_OPACITY, OrchestratorText},
  ui::{
    icon::{Icon, icon},
    message::message_text,
    pressable::Press,
    tooltip::TOOLTIP_DELAY,
  },
};

/// The order's column in the tooltip (`X48YP` in `nzz9r`).
const ORDER_WIDTH: f32 = 320.0;

/// What a chip shares with the others: the header its tooltip hangs under, the fit it reports its width to, and what
/// stops a subscription.
#[derive(Clone)]
pub(super) struct Hold {
  pub(super) head: ElementRef,
  pub(super) fit: Fit,
  pub(super) stop: Arc<dyn Fn(NamedTarget) + Send + Sync>,
}

/// The same for every chip of the header, for as long as it shows: two compare equal.
impl PartialEq for Hold {
  fn eq(&self, _: &Self) -> bool {
    true
  }
}

#[derive(Clone, PartialEq)]
pub(super) struct ChipProps {
  pub(super) id: String,
  pub(super) lead: LeadChip,
  /// Why the last stop of it was not applied.
  pub(super) failed: Option<Message>,
  pub(super) hold: Hold,
}

impl DevtoolsInspectable for ChipProps {}

pub(super) struct LeadChipView {
  hovered: Signal<bool>,
  /// The tooltip shows.
  tip: Signal<bool>,
  timer: Timeout,
  pill: ElementRef,
}

impl Component for LeadChipView {
  type Props = ChipProps;

  fn create(ctx: &mut Ctx) -> Self {
    let tip = ctx.signal(false);
    let show = tip.clone();
    Self {
      hovered: ctx.signal(false),
      tip,
      timer: ctx.create_timeout(TOOLTIP_DELAY, move || show.set(true)),
      pill: ctx.element_ref(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ChipProps>().clone();
    let target = props.lead.target.target;
    let hovered = self.hovered.get();
    self.measure(&props);
    let name = message_text(ctx, &props.lead.target.name);
    let label = Text::new(&name)
      .id(format!("{}-name", props.id))
      .variant(OrchestratorText::LeadsChip.role())
      .nowrap();
    let (label, fill, stroke) = if hovered {
      (
        label.color(PaletteColor::TextPrimary),
        ExtraColor::Hover.role(),
        PaletteColor::BorderStrong,
      )
    } else {
      (label, PaletteColor::SurfaceRaised, PaletteColor::Border)
    };
    let base = match route(&props.lead) {
      Some(route) => Press::Open(Navigation::of(ctx).to(route)).button(),
      None => Button::empty().align_items(Alignment::Center).focusable(false),
    };
    let named = NamedTarget {
      target,
      name: Some(name.to_string()),
    };
    let stop = stop_button(ctx, &props.id, (&props.hold, named), (IconSize::Xs, hovered));
    let tip = self.tip.get().then(|| tooltip(ctx, &props, &self.pill));
    let (enter, leave) = (self.on_hover(), self.on_leave());
    base
      .id(props.id)
      .ref_element(self.pill.clone())
      .height(Dimension::Px(CHIP_HEIGHT))
      .flex_shrink(0.0)
      .padding(Padding::new().left(SpacingSize::Sm).right(SpacingSize::Sm))
      .spacing(ExtraSpacing::Pair.role())
      .align_items(Alignment::Center)
      .background(fill)
      .border(Border::inside(BorderSize::Sm, stroke))
      .rounded(ExtraRadius::Full.role())
      .on_mouse_enter(enter)
      .on_mouse_leave(leave)
      .child(icon(target_icon(target), IconSize::Xs, PaletteColor::TextSecondary))
      .child(label)
      .child(stop)
      .with_children(tip)
  }
}

impl LeadChipView {
  /// Reports the chip's width to the fit: its × always shows, so the pointer never changes it.
  fn measure(&self, props: &ChipProps) {
    let (fit, target) = (props.hold.fit.clone(), props.lead.target.target);
    self.pill.observe_rect(move |rect| {
      if rect.width > 0.0 {
        fit.record(target, rect.width);
      }
    });
  }

  fn on_hover(&self) -> impl Fn() + Send + Sync + 'static {
    let (hovered, timer) = (self.hovered.clone(), self.timer.clone());
    move || {
      hovered.set(true);
      timer.restart();
    }
  }

  fn on_leave(&self) -> impl Fn() + Send + Sync + 'static {
    let (hovered, timer, tip) = (self.hovered.clone(), self.timer.clone(), self.tip.clone());
    move || {
      timer.cancel();
      tip.set(false);
      hovered.set(false);
    }
  }
}

/// Where a press on `lead`'s chip, or on its row in *+N*'s popover, takes the person; `None` for a target with no
/// page.
pub(super) fn route(lead: &LeadChip) -> Option<Route> {
  lead.target.link.as_deref().and_then(|link| Route::parse(link).ok())
}

/// How far down a popup under `anchor` moves to hang `gap` under the header `head` instead, as both were last laid
/// out.
pub(super) fn under_head(head: &ElementRef, anchor: &ElementRef, gap: f32) -> f32 {
  (head.y() + head.height()) - (anchor.y() + anchor.height()) + gap
}

/// The tooltip of the chip of `props`, laid out as `pill`: 8 under the header, its start at the chip's.
fn tooltip(ctx: &mut Ctx, props: &ChipProps, pill: &ElementRef) -> Popup {
  let lead = &props.lead;
  let order = match &lead.order {
    WatchOrder::Own(order) => ctx.t_args("orchestrator.leads.order.own", [("order", &**order)]),
    WatchOrder::ProjectDefault(order) => ctx.t_args("orchestrator.leads.order.projectDefault", [("order", &**order)]),
    WatchOrder::None => ctx.t("orchestrator.leads.order.none"),
  };
  let hint = match &props.failed {
    Some(reason) => {
      let reason = message_text(ctx, reason);
      ctx.t_args("orchestrator.leads.stopFailed", [("reason", &*reason)])
    }
    None => ctx.t("orchestrator.leads.hint"),
  };
  let id = format!("{}-tooltip", props.id);
  let panel = Column::new()
    .id(id.clone())
    .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Snug.role()))
    .spacing(ExtraSpacing::Tight.role())
    .background(PaletteColor::TextPrimary)
    .rounded(RadiusSize::Sm)
    .child(
      Text::new(&since(ctx, lead))
        .id(format!("{id}-title"))
        .variant(OrchestratorText::LeadsTipTitle.role())
        .nowrap(),
    )
    .child(
      Text::new(&order)
        .id(format!("{id}-order"))
        .variant(OrchestratorText::LeadsTipOrder.role())
        .width(Dimension::Px(ORDER_WIDTH)),
    )
    .child(
      Text::new(&hint)
        .id(format!("{id}-hint"))
        .variant(OrchestratorText::LeadsTipHint.role())
        .opacity(LEADS_HINT_OPACITY)
        .max_width(Dimension::Px(ORDER_WIDTH)),
    );
  let away = ctx.theme().spacing_value(SpacingSize::Sm).to_px();
  Popup::new(pill.clone(), panel)
    .open_when(true)
    .placement(Placement::BottomStart)
    .offset(0.0, under_head(&props.hold.head, pill, away))
    .hit_test(HitTestBehavior::None)
    .dismiss_on_outside_click(false)
    .dismiss_on_escape(false)
}

/// *Webhooks v2 · module · since 09:14*, or *· since Oct 8* for an earlier day.
fn since(ctx: &Ctx, lead: &LeadChip) -> Arc<str> {
  let name = message_text(ctx, &lead.target.name);
  let kind = message_text(ctx, &lead.target.kind);
  if today(lead.since) {
    let time = display::clock(lead.since);
    ctx.t_args(
      "orchestrator.leads.since.today",
      [("name", &*name), ("kind", &*kind), ("time", time.as_str())],
    )
  } else {
    let date = super::super::date(ctx, lead.since);
    ctx.t_args(
      "orchestrator.leads.since.on",
      [("name", &*name), ("kind", &*kind), ("date", &*date)],
    )
  }
}

/// Whether `at` is on today's local day: since when a target is led says its time then, else its day.
pub(super) fn today(at: Timestamp) -> bool {
  at.local_day_start() == Timestamp::now().local_day_start()
}

/// A module's `boxes`, a task's `square-check`; the tasks without a module and the whole project, which the design
/// draws no chip of, a list of tasks and the Overview's icon.
pub(super) const fn target_icon(target: WatchTarget) -> Icon {
  match target {
    WatchTarget::Module(_) => Icon::Boxes,
    WatchTarget::Task(_) => Icon::SquareCheck,
    WatchTarget::WithoutModule => Icon::ListChecks,
    WatchTarget::Project => Icon::LayoutDashboard,
  }
}
