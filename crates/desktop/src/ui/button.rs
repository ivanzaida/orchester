//! Button (`u1XBP`): an optional icon and a label, as the primary or secondary action.
//! Sizes and colors come from lurq's form button roles, filled from the design in `theme::form`.
//! A button is pointer-only unless it is marked [`ActionButton::in_form`] (see `ui::pressable`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    events::mouse_event::MouseEvent,
    theme::{BorderSize, FormButtonRole, SpacingSize, TypographyStyle},
  },
  components::{Button, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension},
};

use super::{
  icon::{Icon, IconSize, icon},
  pressable::{Ring, TAB_STOP},
};
use crate::theme::DISABLED_OPACITY;

type Handler = Arc<dyn Fn() + Send + Sync>;

/// lurq's tab index for a control that Tab passes over.
pub const SKIPPED_BY_TAB: i32 = -1;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
  Primary,
  Secondary,
}

enum State {
  Enabled(Handler),
  /// Temporarily off, e.g. while the input is incomplete or work is running.
  Disabled,
  /// The feature does not exist yet; marked with a lock like other unavailable controls.
  Unavailable,
}

pub struct ActionButton {
  label: Arc<str>,
  icon: Option<Icon>,
  kind: Kind,
  state: State,
  width: Option<Dimension>,
  height: Option<Dimension>,
  roomy: bool,
  compact: bool,
  in_form: bool,
  /// The icon and label start at the left instead of the middle.
  start: bool,
  id: Option<&'static str>,
}

impl ActionButton {
  /// A secondary button. Without `on_click` it renders as unavailable.
  pub fn new(label: Arc<str>) -> Self {
    Self {
      label,
      icon: None,
      kind: Kind::Secondary,
      state: State::Unavailable,
      width: None,
      height: None,
      roomy: false,
      compact: false,
      in_form: false,
      start: false,
      id: None,
    }
  }

  pub fn primary(mut self) -> Self {
    self.kind = Kind::Primary;
    self
  }

  pub fn icon(mut self, icon: Icon) -> Self {
    self.icon = Some(icon);
    self
  }

  pub fn on_click(mut self, handler: impl Fn() + Send + Sync + 'static) -> Self {
    self.state = State::Enabled(Arc::new(handler));
    self
  }

  /// A button that cannot be pressed now, e.g. while the request it makes is under way.
  pub fn disabled(mut self) -> Self {
    self.state = State::Disabled;
    self
  }

  /// When not `enabled`, an available button renders disabled.
  pub fn enabled(mut self, enabled: bool) -> Self {
    if !enabled && matches!(self.state, State::Enabled(_)) {
      self.state = State::Disabled;
    }
    self
  }

  pub fn size(mut self, width: impl Into<Dimension>, height: impl Into<Dimension>) -> Self {
    self.width = Some(width.into());
    self.height = Some(height.into());
    self
  }

  pub fn width(mut self, width: impl Into<Dimension>) -> Self {
    self.width = Some(width.into());
    self
  }

  pub fn height(mut self, height: impl Into<Dimension>) -> Self {
    self.height = Some(height.into());
    self
  }

  /// Wider horizontal padding, as on page header actions (`nMeVq`).
  pub fn roomy(mut self) -> Self {
    self.roomy = true;
    self
  }

  /// Smaller label and icon, as the Copy button of a command row (`eFsWL` in `LBSz7`).
  pub fn compact(mut self) -> Self {
    self.compact = true;
    self
  }

  /// A control of a form or a dialog: a Tab stop with the outer focus ring while focused.
  pub fn in_form(mut self) -> Self {
    self.in_form = true;
    self
  }

  /// The icon and label at the left, as the full-width choices of a side panel (`oeTlM`, `BBL80`).
  pub fn start(mut self) -> Self {
    self.start = true;
    self
  }

  pub fn fill_width(mut self) -> Self {
    self.width = Some(Dimension::Pct(100.0));
    self
  }

  /// Element id for tooling (`lurq_read_tree`, `lurq_interact`).
  pub fn id(mut self, id: &'static str) -> Self {
    self.id = Some(id);
    self
  }

  pub fn render(self, ctx: &Ctx) -> Element {
    let form = ctx.theme().form();
    let role = match self.kind {
      Kind::Primary => form.button.primary.clone(),
      Kind::Secondary => form.button.secondary.clone(),
    };
    drop(form);
    let text = role.text.color.clone();

    let (icon_size, typography) = if self.compact {
      (IconSize::Marker, TypographyStyle::Link)
    } else {
      (IconSize::Button, role.text.typography)
    };
    let mut content = Row::new().spacing(SpacingSize::Sm).align_items(Alignment::Center);
    if let Some(glyph) = self.icon {
      content = content.child(icon(glyph, icon_size, text.clone()));
    }
    content = content.child(Text::new(&self.label).variant(typography).color(text.clone()).nowrap());
    if matches!(self.state, State::Unavailable) {
      content = content.child(icon(Icon::Lock, IconSize::Small, text));
    }

    let mut button = Button::empty()
      .button()
      .height(self.height.unwrap_or(role.height))
      .padding_custom(role.padding)
      .background(role.background.clone())
      .border(Border::inside(BorderSize::Sm, role.border.clone()))
      .rounded(role.radius)
      .align_items(Alignment::Center)
      .justify(if self.start { Justify::Start } else { Justify::Center })
      .child(content);
    if self.roomy {
      button = button.padding_horizontal(SpacingSize::Md);
    }
    if let Some(width) = self.width {
      button = button.width(width);
    }
    if let Some(id) = self.id {
      button = button.id(id);
    }
    let in_form = self.in_form;
    let focus = |button: Button, tab_index| {
      if in_form {
        button.tab_index(tab_index)
      } else {
        button.focusable(false)
      }
    };
    match self.state {
      // A press is the button's own: it does not also press a clickable row around it.
      State::Enabled(handler) => with_interaction(focus(button, TAB_STOP), &role, in_form)
        .describe("state", "enabled")
        .on_click(move |event: MouseEvent| {
          event.stop_propagation();
          handler();
        }),
      // Tab skips a button that cannot be pressed.
      State::Disabled => not_pressable(focus(button, SKIPPED_BY_TAB)).describe("state", "disabled"),
      State::Unavailable => not_pressable(focus(button, SKIPPED_BY_TAB)).describe("state", "unavailable"),
    }
    .into()
  }
}

/// A button that cannot be pressed: its variant's own colors, faded as a whole (`kLJJb`).
fn not_pressable(button: Button) -> Button {
  button.opacity(DISABLED_OPACITY).cursor(CursorIcon::NotAllowed)
}

/// Pointer, hover and press fills; in a form also the outer focus ring while focused. Hover and
/// press set no shadow, so the ring stays.
fn with_interaction(button: Button, role: &FormButtonRole, in_form: bool) -> Button {
  let state = |background, border| {
    Style::new()
      .background(background)
      .border(Border::inside(BorderSize::Sm, border))
  };
  let button = if in_form {
    button.focused_style(Ring::Outer.focused())
  } else {
    button
  };
  button
    .cursor(CursorIcon::Pointer)
    .hovered_style(state(role.background_hover.clone(), role.border_hover.clone()))
    .active_style(state(role.background_active.clone(), role.border_active.clone()))
}
