//! An attachment's chip: an image's thumbnail, or a 32 tile with the kind's icon on `surface_input`; then the file's
//! name in 12 `text_primary`, cut to fit, over its size in 11 `text_muted`; in a composer, a cross that removes it.
//! The chip is `surface_raised` in a `border` stroke, radius 6, 4 inside and 8 at its end, 240 wide at most. A chip
//! that opens a preview or the file is pressed with the pointer, with the `hover` fill, and while its preview is open
//! its stroke is `border_focus`.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    events::mouse_event::MouseEvent,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Image, Row, Text, TextOverflow},
  images::{ImageKey, ImageSource},
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::attachment::MediaType;

use super::{TILE, TILE_PIXELS, kind_icon, size_words};
use crate::{
  theme::{ExtraColor, IconSize},
  ui::{icon::icon, pressable::pressable},
};

type Action = Arc<dyn Fn() + Send + Sync>;

pub struct AttachmentChip {
  id: Arc<str>,
  name: Arc<str>,
  media: MediaType,
  size: u64,
  thumbnail: Option<(ImageSource, ImageKey)>,
  open: Option<(Arc<str>, Action)>,
  remove: Option<(Arc<str>, Action)>,
  opened: bool,
}

impl AttachmentChip {
  /// The chip of a file named `name`, of `media`, `size` bytes; `id` is its element's.
  pub fn new(id: impl Into<Arc<str>>, (name, media, size): (&str, MediaType, u64)) -> Self {
    Self {
      id: id.into(),
      name: Arc::from(name),
      media,
      size,
      thumbnail: None,
      open: None,
      remove: None,
      opened: false,
    }
  }

  /// An image's thumbnail from `source`, cached under `key`, in place of the kind's icon.
  #[must_use]
  pub fn thumbnail(mut self, source: ImageSource, key: ImageKey) -> Self {
    self.thumbnail = Some((source, key));
    self
  }

  /// A press opens its preview, or the file; `label` names the press for tools.
  #[must_use]
  pub fn opens(mut self, label: Arc<str>, open: impl Fn() + Send + Sync + 'static) -> Self {
    self.open = Some((label, Arc::new(open)));
    self
  }

  /// Its cross removes it; `label` names the cross for tools.
  #[must_use]
  pub fn removable(mut self, label: Arc<str>, remove: impl Fn() + Send + Sync + 'static) -> Self {
    self.remove = Some((label, Arc::new(remove)));
    self
  }

  /// Its preview is open.
  #[must_use]
  pub const fn opened(mut self, opened: bool) -> Self {
    self.opened = opened;
    self
  }

  pub fn render(self, ctx: &Ctx) -> Element {
    let stroke = if self.opened {
      PaletteColor::BorderFocus
    } else {
      PaletteColor::Border
    };
    let words = Column::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .child(
        Text::new(&self.name)
          .id(format!("{}-name", self.id))
          .variant(TypographyStyle::FieldLabel)
          .color(PaletteColor::TextPrimary)
          .nowrap()
          .text_overflow(TextOverflow::Elipsis),
      )
      .child(
        Text::new(&size_words(ctx, self.size))
          .id(format!("{}-size", self.id))
          .variant(TypographyStyle::Caption)
          .nowrap(),
      );
    let face = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(self.tile())
      .child(words)
      .with_children(self.remove.clone().map(|(label, remove)| cross(&self.id, label, remove)));
    let chip = match self.open.clone() {
      Some((label, open)) => pressable(move || open())
        .describe("a11y_name", label)
        .hovered_style(Style::new().background(ExtraColor::Hover.role())),
      None => Button::empty().focusable(false),
    };
    chip
      .id(self.id.clone())
      .describe("name", self.name.clone())
      .describe("media_type", self.media.as_str())
      .describe("size", self.size.to_string())
      .describe("opened", self.opened.to_string())
      .max_width(Dimension::Px(super::CHIP_WIDTH))
      .flex_shrink(1.0)
      .padding(Padding::all(SpacingSize::Xs).right(SpacingSize::Sm))
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, stroke))
      .rounded(RadiusSize::Md)
      .child(face)
      .into()
  }

  /// An image's thumbnail, or the tile with the kind's icon, which also stands for a thumbnail while it decodes or
  /// when it cannot.
  fn tile(&self) -> Element {
    let media = self.media;
    let glyph = move || {
      Row::new()
        .size(Dimension::Px(TILE), Dimension::Px(TILE))
        .flex_shrink(0.0)
        .align_items(Alignment::Center)
        .justify(Justify::Center)
        .background(PaletteColor::SurfaceInput)
        .rounded(RadiusSize::Sm)
        .child(icon(kind_icon(media), IconSize::Md, PaletteColor::TextMuted))
    };
    let Some((source, key)) = self.thumbnail.clone() else {
      return glyph().into();
    };
    Image::from_source(source, key)
      .id(format!("{}-thumbnail", self.id))
      .downscale_to(TILE_PIXELS, TILE_PIXELS)
      .cover()
      .size(Dimension::Px(TILE), Dimension::Px(TILE))
      .flex_shrink(0.0)
      .rounded(RadiusSize::Sm)
      .clip()
      .placeholder(glyph())
      .error_view(move |_| glyph())
      .into()
  }
}

/// The cross that removes the chip's file: its own press, which does not open the chip too.
fn cross(chip: &str, label: Arc<str>, remove: Action) -> Element {
  Button::empty()
    .id(format!("{chip}-remove"))
    .align_items(Alignment::Center)
    .focusable(false)
    .cursor(CursorIcon::Pointer)
    .describe("a11y_name", label)
    .on_click(move |event: MouseEvent| {
      event.stop_propagation();
      remove();
    })
    .child(icon(crate::ui::icon::Icon::X, IconSize::Sm, PaletteColor::TextMuted))
    .into()
}
