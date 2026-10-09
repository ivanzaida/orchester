//! Text an agent or a person wrote, read as Markdown (lurq's Markdown with `theme::markdown`): by default
//! in a chat message's 13/1.5 `text_primary` (`D8JGX4` in `Agent message` `dcd0m`). It is selectable,
//! and what a link press does is the screen's: the kit never opens anything by itself.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{FormTextRole, PaletteColor, TypographyStyle},
  },
  components::{Markdown, MarkdownLink, MarkdownProps},
  node::{Element, dimension::Dimension},
};

pub struct MarkdownView {
  id: Arc<str>,
  text: Arc<str>,
  typography: TypographyStyle,
  color: PaletteColor,
  on_link: Arc<dyn Fn(&str) + Send + Sync>,
}

impl MarkdownView {
  /// `text` read as Markdown; a press on a link hands its destination to `on_link`.
  pub fn new(
    id: impl Into<Arc<str>>,
    text: impl Into<Arc<str>>,
    on_link: impl Fn(&str) + Send + Sync + 'static,
  ) -> Self {
    Self {
      id: id.into(),
      text: text.into(),
      typography: TypographyStyle::Body,
      color: PaletteColor::TextPrimary,
      on_link: Arc::new(on_link),
    }
  }

  /// The body's text style and color, where the design's differ from a chat message's.
  pub fn style(mut self, typography: TypographyStyle, color: PaletteColor) -> Self {
    self.typography = typography;
    self.color = color;
    self
  }

  pub fn render(self, ctx: &mut Ctx) -> Element {
    let style = {
      let theme = ctx.theme();
      FormTextRole {
        typography: self.typography,
        color: self.color,
      }
      .resolve(&theme.typography(), &theme.palette())
    };
    let on_link = self.on_link;
    let props = MarkdownProps::styled(self.text, style)
      .width(Dimension::Pct(100.0))
      .selectable(true)
      .on_link_click(move |link: &MarkdownLink| on_link(link.destination()));
    let key = format!("markdown-{}", self.id);
    ctx.mount_keyed::<Markdown>(&key, props).id(self.id.to_string())
  }
}
