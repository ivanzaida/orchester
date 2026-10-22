//! A file's preview, opened from its chip, over a composer's box or under a sent message: a panel on
//! `surface_raised` in a `border` stroke, radius 10, 12 inside, 8 between its parts. Its head names the file (the
//! kind's icon, the name in 12/500, the size in 11 `text_muted`) with what can be done with it — put pasted text back
//! in the message, open the kept file with its default program — and closes with its cross. Under it, a text file's
//! text in the read-only code box with Copy, or an image as large as fits, 360 high at most.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Image, Row, Spacer, Text, TextOverflow},
  images::{ImageKey, ImageSource},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::attachment::MediaType;

use super::{PREVIEW_HEIGHT, PREVIEW_PIXELS, kind_icon, size_words};
use crate::{
  theme::IconSize,
  ui::{
    button::{ActionButton, ButtonKind},
    code_box::CodeBox,
    icon::{Icon, icon},
    icon_button::IconButton,
  },
};

/// The turning loader and what is being read, on one line.
pub fn busy(id: &str, words: &str) -> Row {
  Row::new()
    .id(id.to_owned())
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .describe("state", "loading")
    .child(icon(Icon::LoaderCircle, IconSize::Md, PaletteColor::TextMuted))
    .child(Text::new(words).variant(TypographyStyle::Description).nowrap())
}

type Action = Arc<dyn Fn() + Send + Sync>;

/// What the preview shows of the file.
pub enum Shown {
  /// A text file's text.
  Text(Arc<str>),
  /// An image, decoded from `source`, cached under its key.
  Image(ImageSource, ImageKey),
  /// The text is being read.
  Reading,
  /// It cannot be shown, as the words say: a copy that cannot be read, a PDF.
  Not(Arc<str>),
}

pub struct Preview {
  id: Arc<str>,
  name: Arc<str>,
  media: MediaType,
  size: u64,
  shown: Shown,
  close: Action,
  open: Option<Action>,
  put_back: Option<Action>,
}

impl Preview {
  /// The preview of the file named `name`, of `media`, `size` bytes, showing `shown`; `close` closes it. `id` is its
  /// element's.
  pub fn new(
    id: impl Into<Arc<str>>,
    (name, media, size): (&str, MediaType, u64),
    shown: Shown,
    close: impl Fn() + Send + Sync + 'static,
  ) -> Self {
    Self {
      id: id.into(),
      name: Arc::from(name),
      media,
      size,
      shown,
      close: Arc::new(close),
      open: None,
      put_back: None,
    }
  }

  /// *Open*: the kept file opens with its default program.
  #[must_use]
  pub fn opens(mut self, open: impl Fn() + Send + Sync + 'static) -> Self {
    self.open = Some(Arc::new(open));
    self
  }

  /// *Put back in the message*: pasted text goes back to the words of the message.
  #[must_use]
  pub fn puts_back(mut self, put_back: impl Fn() + Send + Sync + 'static) -> Self {
    self.put_back = Some(Arc::new(put_back));
    self
  }

  pub fn render(self, ctx: &mut Ctx) -> Element {
    let head = self.head(ctx);
    let reading = ctx.t("attachments.adding");
    let body: Element = match self.shown {
      Shown::Text(text) => CodeBox::new(format!("{}-text", self.id), ctx.t("attachments.kind.text"), text).render(ctx),
      Shown::Image(source, key) => Image::from_source(source, key)
        .id(format!("{}-image", self.id))
        .downscale_to(PREVIEW_PIXELS, PREVIEW_PIXELS)
        .contain()
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(PREVIEW_HEIGHT))
        .placeholder(busy(&format!("{}-decoding", self.id), &reading))
        .into(),
      Shown::Reading => busy(&format!("{}-reading", self.id), &reading).into(),
      Shown::Not(why) => Text::new(&why)
        .id(format!("{}-why", self.id))
        .variant(TypographyStyle::Description)
        .into(),
    };
    Column::new()
      .id(self.id.clone())
      .describe("name", self.name.clone())
      .width(Dimension::Pct(100.0))
      .padding(SpacingSize::Md)
      .spacing(SpacingSize::Sm)
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Lg)
      .child(head)
      .child(body)
      .into()
  }

  fn head(&self, ctx: &mut Ctx) -> Row {
    let put_back = self.put_back.clone().map(|put_back| {
      ActionButton::new(
        format!("{}-put-back", self.id),
        ButtonKind::Ghost,
        ctx.t("attachments.putBack"),
      )
      .icon(Icon::CornerDownLeft)
      .on_click(move || put_back())
    });
    let open = self.open.clone().map(|open| {
      ActionButton::new(
        format!("{}-open", self.id),
        ButtonKind::Ghost,
        ctx.t("attachments.openFile"),
      )
      .icon(Icon::ExternalLink)
      .on_click(move || open())
    });
    let close = self.close.clone();
    let close = IconButton::new(format!("{}-close", self.id), Icon::X, ctx.t("attachments.closePreview"))
      .small()
      .on_click(move || close())
      .render(ctx);
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(icon(kind_icon(self.media), IconSize::Md, PaletteColor::TextMuted))
      .child(
        Text::new(&self.name)
          .variant(TypographyStyle::Label)
          .nowrap()
          .text_overflow(TextOverflow::Elipsis)
          .flex_shrink(1.0)
          .min_width(Dimension::Px(0.0)),
      )
      .child(
        Text::new(&size_words(ctx, self.size))
          .variant(TypographyStyle::Caption)
          .nowrap(),
      )
      .child(Spacer::new().flex(1.0))
      .with_children(put_back)
      .with_children(open)
      .child(close)
  }
}
