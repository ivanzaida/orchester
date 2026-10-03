//! Button (`u1XBP`): an optional icon and a label, as the primary or secondary action, or as a quiet
//! action that is only text until the pointer or the focus reaches it.
//! Sizes and colors come from lurq's form button roles, filled from the design in `theme::form`.
//! A button is pointer-only unless it is marked [`ActionButton::in_form`] (see `ui::pressable`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    events::mouse_event::{MouseButton, MouseEvent},
    theme::{BorderSize, FormButtonRole, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Button, Row, Select, SelectOption, Text},
  core::{ElementRef, Signal, signal::SignalValue},
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, SelectPartStyle, Style, border::Border, dimension::Dimension},
};

use super::{
  field::select_style,
  icon::{Icon, IconSize, icon},
  link::{Link, Press},
  pressable::{Ring, TAB_STOP},
};
use crate::theme::{DISABLED_OPACITY, ExtraShadow, ExtraSpacing};

/// lurq's tab index for a control that Tab passes over.
pub const SKIPPED_BY_TAB: i32 = -1;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
  Primary,
  Secondary,
  /// No fill and no border, its icon and label muted; under the pointer the hover fill shows and
  /// they turn `text` (`feLwP`, `oriyY`).
  Quiet,
}

/// Whether the pointer is over a quiet button, kept by its owner, and who hears it change: lurq
/// does not color a button's text by the button's own hover.
struct Hover {
  hovered: bool,
  changed: Arc<dyn Fn(bool) + Send + Sync>,
}

enum State {
  Enabled(Press),
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
  min_width: Option<Dimension>,
  element_ref: Option<ElementRef>,
  height: Option<Dimension>,
  roomy: bool,
  compact: bool,
  in_form: bool,
  /// The icon and label start at the left instead of the middle.
  start: bool,
  hover: Option<Hover>,
  id: Option<Arc<str>>,
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
      min_width: None,
      element_ref: None,
      height: None,
      roomy: false,
      compact: false,
      in_form: false,
      start: false,
      hover: None,
      id: None,
    }
  }

  pub fn primary(mut self) -> Self {
    self.kind = Kind::Primary;
    self
  }

  /// A secondary action that should not draw the eye, e.g. Copy beside a value's label (`feLwP`):
  /// 6 at each side and between its icon and label. `hovered` is whether the pointer is over it;
  /// `changed` hears the pointer enter (`true`) and leave (`false`).
  pub fn quiet(mut self, hovered: bool, changed: impl Fn(bool) + Send + Sync + 'static) -> Self {
    self.kind = Kind::Quiet;
    self.hover = Some(Hover {
      hovered,
      changed: Arc::new(changed),
    });
    self
  }

  pub fn icon(mut self, icon: Icon) -> Self {
    self.icon = Some(icon);
    self
  }

  pub fn on_click(mut self, handler: impl Fn() + Send + Sync + 'static) -> Self {
    self.state = State::Enabled(Press::action(handler));
    self
  }

  /// Opens a place where the press asks (see `ui::link`).
  pub fn link(mut self, link: Link) -> Self {
    self.state = State::Enabled(Press::Link(link));
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

  pub fn min_width(mut self, width: impl Into<Dimension>) -> Self {
    self.min_width = Some(width.into());
    self
  }

  /// Observe this button's laid-out bounds from its owning component.
  pub fn ref_element(mut self, element_ref: ElementRef) -> Self {
    self.element_ref = Some(element_ref);
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
  pub fn id(mut self, id: impl Into<Arc<str>>) -> Self {
    self.id = Some(id.into());
    self
  }

  pub fn render(self, ctx: &Ctx) -> Element {
    let form = ctx.theme().form();
    let role = match self.kind {
      Kind::Primary => form.button.primary.clone(),
      Kind::Secondary | Kind::Quiet => form.button.secondary.clone(),
    };
    drop(form);
    let quiet = self.kind == Kind::Quiet;
    let content = self.content(&role);
    let mut button = Button::empty()
      .button()
      .height(self.height.unwrap_or(role.height))
      .padding_custom(role.padding)
      .rounded(role.radius)
      .align_items(Alignment::Center)
      .justify(if self.start { Justify::Start } else { Justify::Center })
      .child(content);
    match self.hover {
      Some(hover) => {
        let (enter, leave) = (hover.changed.clone(), hover.changed);
        button = button
          .padding_horizontal(ExtraSpacing::Tight.role())
          .on_mouse_enter(move || enter(true))
          .on_mouse_leave(move || leave(false));
      }
      None => {
        button = button
          .background(role.background.clone())
          .border(Border::inside(BorderSize::Sm, role.border.clone()));
      }
    }
    if let Some(element_ref) = self.element_ref {
      button = button.ref_element(element_ref);
    }
    if self.roomy {
      button = button.padding_horizontal(SpacingSize::Md);
    }
    if let Some(width) = self.width {
      button = button.width(width);
    }
    if let Some(width) = self.min_width {
      button = button.min_width(width);
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
      State::Enabled(press) => press.attach(
        with_interaction(focus(button, TAB_STOP), &role, in_form, quiet)
          .describe("state", "enabled")
          .on_click(|event: MouseEvent| event.stop_propagation())
          .on_mouse_click(MouseButton::Middle, |event: MouseEvent| event.stop_propagation()),
      ),
      // Tab skips a button that cannot be pressed.
      State::Disabled => not_pressable(focus(button, SKIPPED_BY_TAB)).describe("state", "disabled"),
      State::Unavailable => not_pressable(focus(button, SKIPPED_BY_TAB)).describe("state", "unavailable"),
    }
    .into()
  }
}

impl ActionButton {
  /// The icon, the label and, when unavailable, the lock, in the variant's text color: a quiet
  /// button's is `muted`, and `text` under the pointer.
  fn content(&self, role: &FormButtonRole) -> Row {
    let text = match &self.hover {
      Some(hover) if hover.hovered => PaletteColor::TextPrimary,
      Some(_) => PaletteColor::TextSecondary,
      None => role.text.color.clone(),
    };
    let (icon_size, typography) = if self.compact {
      (IconSize::Marker, TypographyStyle::Link)
    } else {
      (IconSize::Button, role.text.typography)
    };
    let gap = if self.kind == Kind::Quiet {
      ExtraSpacing::Tight.role()
    } else {
      SpacingSize::Sm
    };
    let mut content = Row::new().spacing(gap).align_items(Alignment::Center);
    if let Some(glyph) = self.icon {
      content = content.child(icon(glyph, icon_size, text.clone()));
    }
    content = content.child(Text::new(&self.label).variant(typography).color(text.clone()).nowrap());
    if matches!(self.state, State::Unavailable) {
      content = content.child(icon(Icon::Lock, IconSize::Small, text));
    }
    content
  }
}

/// A button that cannot be pressed: its variant's own colors, faded as a whole (`kLJJb`).
fn not_pressable(button: Button) -> Button {
  button.opacity(DISABLED_OPACITY).cursor(CursorIcon::NotAllowed)
}

/// Pointer, hover and press fills; in a form also the outer focus ring while focused. Hover and
/// press set no shadow, so the ring stays. A quiet button gets the secondary hover fill under the
/// pointer and while pressed, and no border.
fn with_interaction(button: Button, role: &FormButtonRole, in_form: bool, quiet: bool) -> Button {
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
  let button = button.cursor(CursorIcon::Pointer);
  if quiet {
    let fill = || Style::new().background(role.background_hover.clone());
    return button.hovered_style(fill()).active_style(fill());
  }
  button
    .hovered_style(state(role.background_hover.clone(), role.border_hover.clone()))
    .active_style(state(role.background_active.clone(), role.border_active.clone()))
}

/// A select drawn as a compact secondary button `height` high with `glyph` and `label`, e.g. *Add a
/// variable from a secret…* (`Pdcuk` in 04.E `yRb2k`: an 11/500 label beside a 12px icon). Its face
/// stays the same whatever is chosen, so the menu (`u0ByM`) marks no option as chosen. A Tab stop,
/// with the outer focus ring while focused, like a button in a form; its parents let the ring show
/// past them.
pub fn select_button<T>(
  ctx: &Ctx,
  value: Signal<T>,
  options: Vec<SelectOption<T>>,
  (glyph, label): (Icon, Arc<str>),
  height: f32,
  min_menu_width: f32,
) -> Select<T>
where
  T: Clone + PartialEq + Send + Sync + 'static + SignalValue,
  Vec<T>: SignalValue,
{
  let role = ctx.theme().form().button.secondary.clone();
  let text = role.text.color.clone();
  let face = move |_| {
    Row::new()
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(icon(glyph, IconSize::Small, text.clone()))
      .child(
        Text::new(&label)
          .variant(TypographyStyle::Link)
          .color(text.clone())
          .nowrap(),
      )
  };
  let state = |background, border| {
    SelectPartStyle::new()
      .background(background)
      .border_inside(BorderSize::Sm, border)
  };
  let style = select_style(ctx, height)
    .trigger(
      state(role.background, role.border)
        .rounded(role.radius)
        .padding(role.padding)
        .min_height(height),
    )
    .trigger_hovered(state(role.background_hover, role.border_hover))
    .trigger_open(state(role.background_active, role.border_active))
    .trigger_focused(SelectPartStyle::new().box_shadow(ExtraShadow::FocusRing.role()))
    .min_menu_width(min_menu_width)
    .single_checkmark(false);
  Select::new(value)
    .options(options)
    .trigger(face)
    .height(Dimension::Px(height))
    .style(style)
}
