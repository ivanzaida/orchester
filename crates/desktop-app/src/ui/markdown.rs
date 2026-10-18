//! Text an agent or a person wrote, read as Markdown (lurq's Markdown with `theme::markdown`): by default
//! in a chat message's 13/1.5 `text_primary` (`D8JGX4` in `Agent message` `dcd0m`). It is selectable,
//! and what a link press does is the screen's: the kit never opens anything by itself. Its links are shown as the
//! chat shows them (`ui::chat::presentation`): a web page with its host, anything not followed as text. It is laid out
//! again only when what it shows changes; the link handler counts by identity, so a screen keeps it in
//! its state rather than making a new one on every render. What it reads as once rendered, as plain text, is
//! [`plain`]'s.

pub mod plain;

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::MouseEvent,
    theme::{FormTextRole, PaletteColor, TypographyStyle},
  },
  components::{Markdown, MarkdownLink, MarkdownProps},
  node::{Element, dimension::Dimension},
};

use crate::{
  theme::{ChatLinkLooks, blank_line_between_blocks},
  ui::chat::presentation::{LinkWords, presentation},
};

/// What a press on a link does with its destination: the event that activated it, a left or middle click or Enter
/// on the focused link (`MarkdownProps::on_link_activate`), says with which button and keys.
pub type LinkHandler = Arc<dyn Fn(&str, &MouseEvent) + Send + Sync>;

/// How far apart the text's blocks (paragraphs, lists, code) sit.
#[derive(Clone, Copy, PartialEq, Eq)]
enum BlockGap {
  /// The theme's Markdown spacing.
  Theme,
  /// One empty line of the body text.
  BlankLine,
}

#[derive(Clone)]
pub struct MarkdownView {
  id: Arc<str>,
  text: Arc<str>,
  typography: TypographyStyle,
  color: PaletteColor,
  block_gap: BlockGap,
  on_link: LinkHandler,
}

impl PartialEq for MarkdownView {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
      && self.text == other.text
      && self.typography == other.typography
      && self.color == other.color
      && self.block_gap == other.block_gap
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
      block_gap: BlockGap::Theme,
      on_link,
    }
  }

  /// The body's text style and color, where the design's differ from a chat message's.
  pub fn style(mut self, typography: TypographyStyle, color: PaletteColor) -> Self {
    self.typography = typography;
    self.color = color;
    self
  }

  /// An empty line of the body between paragraphs, where the design draws the text that way (a task's
  /// description in Details, `bU9bE` in `ZSiHd`) rather than with the theme's tighter Markdown spacing.
  pub fn blank_line_between_blocks(mut self) -> Self {
    self.block_gap = BlockGap::BlankLine;
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
    let looks = ChatLinkLooks::of(ctx.theme(), &style);
    let spacing = match view.block_gap {
      BlockGap::Theme => None,
      BlockGap::BlankLine => Some(blank_line_between_blocks(&ctx.theme().markdown(), &style)),
    };
    let i18n = ctx.i18n().clone();
    let on_link = view.on_link;
    let mut props = MarkdownProps::styled(view.text, style)
      .width(Dimension::Pct(100.0))
      .selectable(true)
      .on_link_activate(move |link: &MarkdownLink, event: &MouseEvent| on_link(link.destination(), event))
      .link_presentation(move |link: &MarkdownLink| {
        let words = LinkWords {
          i18n: &i18n,
          place: &|_| None,
        };
        presentation(link.destination(), link.text(), &looks, &words)
      });
    if let Some(markdown) = spacing {
      props = props.theme(markdown);
    }
    Markdown::mount(ctx, props).id(view.id.to_string())
  }
}
