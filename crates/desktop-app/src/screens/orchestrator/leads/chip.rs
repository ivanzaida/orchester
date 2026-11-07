//! One Leads chip (`Watch · Webhooks v2` `VFXga` and `Watch · KON-45` `rgeO4` in `X93D8T` b; under the pointer
//! `dRw23` in d): a 22 high round pill on `surface_raised` in a `border` stroke, 8 at its sides and 5 between its
//! parts, an 11 icon in `text_secondary` (a module's `boxes`, a task's `square-check`) and the target's name, a task by
//! its key, in 11.5 `text_secondary`. A press opens the module or the task. Under the pointer it is on `hover` in
//! `border_strong`, its name in `text_primary`, and an 11 `x` in `text_secondary` ends it: a press on the `x` stops
//! that subscription at once, with no card (`orchester_workspace::orchestrator::watch::stop_in`, ORC-760).
//!
//! Once the pointer rested on it as long as a tooltip waits, its tooltip hangs 8 under the header, its end at the
//! header's end (`Tooltip · subscription` `nzz9r`): `text_primary`, radius 4, 6 over and under and 8 at the sides, its
//! lines 2 apart in `text_inverse` 11: the name, kind and since when at 600, the order in a 320 wide column at 1.45,
//! and the hint at 70 % — or why the last stop was not applied.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, Timeout},
    events::mouse_event::MouseEvent,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Button, Column, Placement, Popup, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{CursorIcon, Element, HitTestBehavior, border::Border, dimension::Dimension, padding::Padding},
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

use super::{CHIP_HEIGHT, Fit};
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
    let route = props
      .lead
      .target
      .link
      .as_deref()
      .and_then(|link| Route::parse(link).ok());
    let base = match route {
      Some(route) => Press::Open(Navigation::of(ctx).to(route)).button(),
      None => Button::empty().align_items(Alignment::Center).focusable(false),
    };
    let stop = hovered.then(|| stop_button(ctx, &props, &name));
    let tip = self.tip.get().then(|| tooltip(ctx, &props));
    let (enter, leave) = (
      self.on_hover(&props.hold.fit, target),
      self.on_leave(&props.hold.fit, target),
    );
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
      .with_children(stop)
      .with_children(tip)
  }
}

impl LeadChipView {
  /// Reports the chip's width without its × to the fit.
  fn measure(&self, props: &ChipProps) {
    let (hovered, fit, target) = (self.hovered.clone(), props.hold.fit.clone(), props.lead.target.target);
    self.pill.observe_rect(move |rect| {
      if !hovered.get_untracked() && rect.width > 0.0 {
        fit.record(target, rect.width);
      }
    });
  }

  fn on_hover(&self, fit: &Fit, target: WatchTarget) -> impl Fn() + Send + Sync + 'static {
    let (hovered, timer, fit) = (self.hovered.clone(), self.timer.clone(), fit.clone());
    move || {
      fit.rest(target, true);
      hovered.set(true);
      timer.restart();
    }
  }

  fn on_leave(&self, fit: &Fit, target: WatchTarget) -> impl Fn() + Send + Sync + 'static {
    let (hovered, timer, tip, fit) = (self.hovered.clone(), self.timer.clone(), self.tip.clone(), fit.clone());
    move || {
      timer.cancel();
      tip.set(false);
      hovered.set(false);
      fit.rest(target, false);
    }
  }
}

/// The × of the chip of `props`, named `name`, which stops its subscription.
fn stop_button(ctx: &Ctx, props: &ChipProps, name: &str) -> Element {
  let (stop, target, named) = (props.hold.stop.clone(), props.lead.target.target, name.to_owned());
  Button::empty()
    .id(format!("{}-stop", props.id))
    .align_items(Alignment::Center)
    .focusable(false)
    .cursor(CursorIcon::Pointer)
    .describe("a11y_name", ctx.t_args("orchestrator.leads.stop", [("name", name)]))
    .on_click(move |event: MouseEvent| {
      event.stop_propagation();
      stop(NamedTarget {
        target,
        name: Some(named.clone()),
      });
    })
    .child(icon(Icon::X, IconSize::Xs, PaletteColor::TextSecondary))
    .into()
}

/// The tooltip of the chip of `props`, hung under the header.
fn tooltip(ctx: &mut Ctx, props: &ChipProps) -> Popup {
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
  Popup::new(props.hold.head.clone(), panel)
    .open_when(true)
    .placement(Placement::BottomEnd)
    .offset(0.0, away)
    .hit_test(HitTestBehavior::None)
    .dismiss_on_outside_click(false)
    .dismiss_on_escape(false)
}

/// *Webhooks v2 · module · since 09:14*, or *· since Oct 8* for an earlier day.
fn since(ctx: &Ctx, lead: &LeadChip) -> Arc<str> {
  let name = message_text(ctx, &lead.target.name);
  let kind = message_text(ctx, &lead.target.kind);
  if lead.since.local_day_start() == Timestamp::now().local_day_start() {
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

/// A module's `boxes`, a task's `square-check`; the tasks without a module and the whole project, which the design
/// draws no chip of, a list of tasks and the Overview's icon.
const fn target_icon(target: WatchTarget) -> Icon {
  match target {
    WatchTarget::Module(_) => Icon::Boxes,
    WatchTarget::Task(_) => Icon::SquareCheck,
    WatchTarget::WithoutModule => Icon::ListChecks,
    WatchTarget::Project => Icon::LayoutDashboard,
  }
}
