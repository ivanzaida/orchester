//! The *Plan limits* card's menu (`Plan limits menu` `RvEnG`): a 28 square *…* whose one row opens Settings › General
//! at *Plan limits*, the warning threshold's one place (`Duplicates removed` in `o4nCPO`). The row says the threshold
//! the page drew its dashed line at, as Settings last saved it.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::Row,
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_core::usage::limits::WarningThreshold;
use orchester_workspace::usage::Dashboard;

use super::words;
use crate::{
  navigation::Navigation,
  routes::Route,
  ui::{
    icon::Icon,
    icon_button::IconButton,
    menu::{MenuItem, menu_at_end, menu_panel},
  },
};

/// The menu's width.
const MENU_WIDTH: f32 = 260.0;

/// The card's menu, saying `dashboard`'s threshold.
pub fn menu(ctx: &mut Ctx, dashboard: &Dashboard) -> Element {
  ctx.mount_keyed::<ThresholdMenu>("usage-threshold-menu", Threshold(dashboard.threshold))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Threshold(WarningThreshold);

impl DevtoolsInspectable for Threshold {}

pub struct ThresholdMenu {
  open: Signal<bool>,
  anchor: ElementRef,
}

impl Component for ThresholdMenu {
  type Props = Threshold;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      anchor: ElementRef::default(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Threshold(threshold) = *ctx.props::<Threshold>();
    let open = self.open.clone();
    let button = IconButton::new("usage-limits-menu", Icon::Ellipsis, ctx.t("usage.limits.menu"))
      .element(self.anchor.clone())
      .on_click(move || open.update(|open| *open = !*open));
    let shown = self.open.get();
    let button = if shown { button.active() } else { button }.render(ctx);
    let menu = shown.then(|| {
      let share = words::share(ctx, Some(u64::from(threshold.percent())));
      let item = MenuItem::opens(
        "usage-threshold-settings",
        ctx.t("usage.threshold.open"),
        Navigation::of(ctx).to(Route::PlanLimits),
      )
      .icon(Icon::Settings)
      .detail(ctx.t_args("usage.threshold.now", [("percent", &*share)]))
      .render(ctx, &self.open);
      let panel = menu_panel("usage-threshold-menu-panel")
        .width(Dimension::Px(MENU_WIDTH))
        .child(item);
      menu_at_end(self.anchor.clone(), self.open.clone(), panel)
    });
    Row::new().child(button).with_children(menu)
  }
}
