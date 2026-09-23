//! Button (`u1XBP`): an optional icon and a label, as the primary or secondary action.
//! Sizes and colors come from lurq's form button roles, filled from the design in `theme::form`.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, FormButtonRole, PaletteColor, SpacingSize},
  },
  components::{Button, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension},
};

use super::icon::{Icon, IconSize, icon};

type Handler = Arc<dyn Fn() + Send + Sync>;

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

  pub fn fill_width(mut self) -> Self {
    self.width = Some(Dimension::Pct(100.0));
    self
  }

  /// Element id for tooling (`lurq_read_tree`, `lurq_interact`).
  pub fn id(mut self, id: &'static str) -> Self {
    self.id = Some(id);
    self
  }

  fn colors(&self, role: &FormButtonRole) -> (PaletteColor, PaletteColor, PaletteColor) {
    match (&self.state, self.kind) {
      (State::Enabled(_), _) => (role.background.clone(), role.border.clone(), role.text.color.clone()),
      (_, Kind::Primary) => (
        PaletteColor::TextMuted,
        PaletteColor::TextMuted,
        PaletteColor::TextInverse,
      ),
      (_, Kind::Secondary) => (
        PaletteColor::SurfaceRaised,
        PaletteColor::Border,
        PaletteColor::TextMuted,
      ),
    }
  }

  pub fn render(self, ctx: &Ctx) -> Element {
    let form = ctx.theme().form();
    let role = match self.kind {
      Kind::Primary => form.button.primary.clone(),
      Kind::Secondary => form.button.secondary.clone(),
    };
    drop(form);
    let (background, border, text) = self.colors(&role);

    let mut content = Row::new().spacing(SpacingSize::Sm).align_items(Alignment::Center);
    if let Some(glyph) = self.icon {
      content = content.child(icon(glyph, IconSize::Button, text.clone()));
    }
    content = content.child(
      Text::new(&self.label)
        .variant(role.text.typography)
        .color(text.clone())
        .nowrap(),
    );
    if matches!(self.state, State::Unavailable) {
      content = content.child(icon(Icon::Lock, IconSize::Small, text));
    }

    let mut button = Button::empty()
      .button()
      .height(self.height.unwrap_or(role.height))
      .padding_custom(role.padding)
      .background(background)
      .border(Border::inside(BorderSize::Sm, border))
      .rounded(role.radius)
      .align_items(Alignment::Center)
      .justify(Justify::Center)
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
    match self.state {
      State::Enabled(handler) => with_interaction(button, &role)
        .describe("state", "enabled")
        .on_click(move |_| handler()),
      State::Disabled => button.describe("state", "disabled").cursor(CursorIcon::NotAllowed),
      State::Unavailable => button.describe("state", "unavailable").cursor(CursorIcon::NotAllowed),
    }
    .into()
  }
}

fn with_interaction(button: Button, role: &FormButtonRole) -> Button {
  let state = |background, border| {
    Style::new()
      .background(background)
      .border(Border::inside(BorderSize::Sm, border))
  };
  button
    .cursor(CursorIcon::Pointer)
    .hovered_style(state(role.background_hover.clone(), role.border_hover.clone()))
    .active_style(state(role.background_active.clone(), role.border_active.clone()))
}
