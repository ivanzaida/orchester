//! The `x` that stops a subscription the conversation leads: at the end of every Leads chip (an 11 `x` in
//! `text_muted`, `text_secondary` while the pointer is on the chip or on the `x`, `X93D8T` b–d) and of every row of
//! *+N*'s list (a 12 `x` in `text_muted`, `text_secondary` under the pointer, `Jl9jP`). A press stops that subscription
//! at once, with no card (`orchester_workspace::orchestrator::watch::stop_in`, ORC-760), and opens nothing: it is a
//! press target of its own inside the chip or row that opens the target.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::mouse_event::MouseEvent,
    theme::PaletteColor,
  },
  components::Button,
  core::Signal,
  layout::Alignment,
  node::{CursorIcon, Element},
};
use orchester_core::orchestrator::watch::NamedTarget;

use super::chip::Hold;
use crate::{
  theme::IconSize,
  ui::icon::{Icon, icon},
};

#[derive(Clone, PartialEq)]
struct StopProps {
  id: String,
  named: NamedTarget,
  hold: Hold,
  size: IconSize,
  /// The glyph in `text_secondary` without the pointer on the `x` itself: it is on the chip around it.
  lit: bool,
}

impl DevtoolsInspectable for StopProps {}

struct StopView {
  hovered: Signal<bool>,
}

/// The `x` of the control `id` (a chip, or a row of *+N*'s list), its glyph `size` big, which stops the subscription
/// of `named` through `hold`; `lit` while the pointer is on the control around it.
pub(super) fn stop_button(
  ctx: &mut Ctx,
  id: &str,
  (hold, named): (&Hold, NamedTarget),
  (size, lit): (IconSize, bool),
) -> Element {
  let id = format!("{id}-stop");
  ctx.mount_keyed::<StopView>(
    &id.clone(),
    StopProps {
      id,
      named,
      hold: hold.clone(),
      size,
      lit,
    },
  )
}

impl Component for StopView {
  type Props = StopProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      hovered: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<StopProps>().clone();
    let name = props.named.name.clone().unwrap_or_default();
    let (stop, named) = (props.hold.stop.clone(), props.named.clone());
    let color = if props.lit || self.hovered.get() {
      PaletteColor::TextSecondary
    } else {
      PaletteColor::TextMuted
    };
    let (enter, leave) = (self.hovered.clone(), self.hovered.clone());
    Button::empty()
      .id(props.id)
      .align_items(Alignment::Center)
      .focusable(false)
      .cursor(CursorIcon::Pointer)
      .describe(
        "a11y_name",
        ctx.t_args("orchestrator.leads.stop", [("name", name.as_str())]),
      )
      .on_mouse_enter(move || enter.set(true))
      .on_mouse_leave(move || leave.set(false))
      .on_click(move |event: MouseEvent| {
        event.stop_propagation();
        stop(named.clone());
      })
      .child(icon(Icon::X, props.size, color))
  }
}
