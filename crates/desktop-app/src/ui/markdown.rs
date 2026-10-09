//! Text an agent or a person wrote, read as Markdown (lurq's Markdown with `theme::markdown`): by default
//! in a chat message's 13/1.5 `text_primary` (`D8JGX4` in `Agent message` `dcd0m`). It is selectable,
//! and what a link press does is the screen's: the kit never opens anything by itself. It is laid out
//! again only when what it shows changes; the link handler counts by identity, so a screen keeps it in
//! its state rather than making a new one on every render. What it reads as once rendered, as plain text, is
//! [`plain`]'s.

pub mod plain;

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{FormTextRole, PaletteColor, TypographyStyle},
  },
  components::{Markdown, MarkdownLink, MarkdownProps},
  node::{Element, dimension::Dimension},
};

/// What a press on a link does with its destination.
pub type LinkHandler = Arc<dyn Fn(&str) + Send + Sync>;

#[derive(Clone)]
pub struct MarkdownView {
  id: Arc<str>,
  text: Arc<str>,
  typography: TypographyStyle,
  color: PaletteColor,
  on_link: LinkHandler,
}

impl PartialEq for MarkdownView {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
      && self.text == other.text
      && self.typography == other.typography
      && self.color == other.color
      && Arc::ptr_eq(&self.on_link, &other.on_link)
  }
}

impl DevtoolsInspectable for MarkdownView {}

impl MarkdownView {
  /// `text` read as Markdown; a press on a link hands its destination to `on_link`.
  pub fn new(id: impl Into<Arc<str>>, text: impl Into<Arc<str>>, on_link: LinkHandler) -> Self {
    Self {
      id: id.into(),
      text: text.into(),
      typography: TypographyStyle::Body,
      color: PaletteColor::TextPrimary,
      on_link,
    }
  }

  /// The body's text style and color, where the design's differ from a chat message's.
  pub fn style(mut self, typography: TypographyStyle, color: PaletteColor) -> Self {
    self.typography = typography;
    self.color = color;
    self
  }

  pub fn render(self, ctx: &mut Ctx) -> Element {
    let key = format!("markdown-{}", self.id);
    ctx.mount_keyed::<MarkdownHost>(&key, self)
  }
}

struct MarkdownHost;

impl Component for MarkdownHost {
  type Props = MarkdownView;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let view = ctx.props::<MarkdownView>().clone();
    let style = {
      let theme = ctx.theme();
      FormTextRole {
        typography: view.typography,
        color: view.color,
      }
      .resolve(&theme.typography(), &theme.palette())
    };
    let on_link = view.on_link;
    let props = MarkdownProps::styled(view.text, style)
      .width(Dimension::Pct(100.0))
      .selectable(true)
      .on_link_click(move |link: &MarkdownLink| on_link(link.destination()));
    Markdown::mount(ctx, props).id(view.id.to_string())
  }
}
