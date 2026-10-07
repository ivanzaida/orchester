//! Text an agent or the person wrote, read as Markdown with plan review's styling (`theme::markdown`):
//! bold, lists, headings, quotes, tables and inline code by lurq's Markdown component, top-level code
//! blocks laid out here so their long lines scroll instead of being cut off ([`segments`],
//! [`code_block`]). The line breaks it was written with stay; it is selectable, and an `https` link
//! opens in the browser, as in plan review. Laid out again only when the text changes: each part keeps
//! its parsed Markdown while a growing message adds to its end.

mod code_block;
pub mod plain;
pub mod segments;

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{FormTextRole, PaletteColor, TypographyStyle},
  },
  components::{Column, Markdown, MarkdownLink, MarkdownProps},
  node::{Element, dimension::Dimension},
};

use self::{
  code_block::{CodeStyle, code_block},
  segments::{Segment, segments},
};
use crate::{
  external,
  theme::{ExtraSpacing, ExtraTypography},
};

/// Markdown text in one of the text styles agents' text has, mounted under its `id`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownText {
  id: Arc<str>,
  text: Arc<str>,
  typography: TypographyStyle,
  /// The code blocks' lines: the body's size in Geist Mono, as lurq sizes inline code.
  code: TypographyStyle,
  color: PaletteColor,
}

impl lurq::app::component::DevtoolsInspectable for MarkdownText {}

impl MarkdownText {
  /// What a message in the chat says (`V17jfc`): 13 at 1.6, its code at 12.
  pub fn message(id: impl Into<Arc<str>>, text: impl Into<Arc<str>>) -> Self {
    Self::new(
      id,
      text,
      ExtraTypography::TimelineMessage.role(),
      ExtraTypography::Code.role(),
    )
  }

  /// Text an agent wrote in a card, a plan or a step's result (`FP6BD`): 12 at 1.6, its code at 11.
  pub fn agent_text(id: impl Into<Arc<str>>, text: impl Into<Arc<str>>) -> Self {
    Self::new(
      id,
      text,
      ExtraTypography::AgentText.role(),
      ExtraTypography::Output.role(),
    )
  }

  fn new(
    id: impl Into<Arc<str>>,
    text: impl Into<Arc<str>>,
    typography: TypographyStyle,
    code: TypographyStyle,
  ) -> Self {
    Self {
      id: id.into(),
      text: text.into(),
      typography,
      code,
      color: PaletteColor::TextPrimary,
    }
  }

  /// The body's color, when the design's differs from the text style's.
  #[must_use]
  pub fn color(mut self, color: PaletteColor) -> Self {
    self.color = color;
    self
  }

  /// Mounts the text, keyed by its id among the parent's children.
  pub fn mount(self, ctx: &mut Ctx) -> Element {
    let key = self.id.clone();
    ctx.mount_keyed::<MarkdownTextView>(&key, self)
  }
}

pub struct MarkdownTextView;

impl Component for MarkdownTextView {
  type Props = MarkdownText;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<MarkdownText>().clone();
    let style = {
      let theme = ctx.theme();
      FormTextRole {
        typography: props.typography,
        color: props.color.clone(),
      }
      .resolve(&theme.typography(), &theme.palette())
    };
    let code = CodeStyle {
      label: props.typography,
      code: props.code,
    };
    let parts: Vec<Element> = segments(&props.text)
      .into_iter()
      .enumerate()
      .map(|(index, segment)| match segment {
        Segment::Markdown(source) => {
          let markdown = MarkdownProps::styled(source, style.clone())
            .width(Dimension::Pct(100.0))
            .selectable(true)
            .on_link_click(open_link);
          ctx.mount_keyed::<Markdown>(&format!("markdown-{index}"), markdown)
        }
        Segment::Code { language, code: lines } => {
          code_block(format!("{}-code-{index}", props.id), code, (language, lines))
        }
      })
      .collect();
    // Parts sit as far apart as the blocks of one Markdown text (`theme::markdown`).
    Column::new()
      .id(props.id.to_string())
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Stack.role())
      .with_children(parts)
  }
}

fn open_link(link: &MarkdownLink) {
  if let Err(error) = external::open_url(link.destination()) {
    tracing::warn!(%error, "the message's link was not opened");
  }
}
