//! What an agent wrote in a chat, the task Conversation's and the Orchestrator's alike (`Text` `D8JGX4` in `Agent
//! message` `dcd0m`; `Orchestrator · reply with links` `w262B`, `Orchestrator · links · states` `agv8r`): lurq's
//! Markdown in 13/1.5 `text_primary`, the places it names as text linked ([`super::references`]), with every link
//! presented as the chat follows it ([`super::presentation`]), a link to a place in a project with a tooltip that
//! names it ([`super::place`]), and a list of labelled links drawn as their grid ([`super::grid`]). Every text goes
//! through the presentation, which lurq runs on the exact destination a press opens: no link shows words from one
//! destination and opens another. A fenced code block that stands on its own is drawn by the chat, with a head and
//! Copy ([`super::code_block`]), 8 from the Markdown around it ([`super::fences`]). What a search found in it is
//! filled behind its words as the Markdown renders them ([`super::found`]); the words of a labelled links' grid,
//! which the app draws itself, are not.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::MouseEvent,
    theme::{FormTextRole, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Markdown, MarkdownLink, MarkdownProps},
  node::{Element, dimension::Dimension},
};
use orchester_core::id::ProjectId;

use super::{
  code_block::code_block,
  fences::{self, Piece},
  found::Found,
  grid,
  names::ProjectNames,
  place,
  presentation::{LinkWords, presentation},
  references,
};
use crate::{routes::Route, theme::ChatLinkLooks, ui::markdown::LinkHandler};

/// What an agent wrote, `text`, in a chat of `project`; `names` are the projects' names, for what a link to another
/// project's place says, and a press on a link goes to `links`. What a search found in it (`found`) is filled behind
/// its words. `id` names it.
pub fn agent_text(
  ctx: &mut Ctx,
  id: &str,
  text: &Arc<str>,
  (project, names): (ProjectId, &ProjectNames),
  (links, found): (&LinkHandler, Option<&Found>),
) -> Element {
  ctx.mount_keyed::<AgentText>(
    &format!("agent-text-{id}"),
    AgentTextProps {
      id: Arc::from(id),
      text: text.clone(),
      project,
      names: names.clone(),
      links: links.clone(),
      found: found.cloned(),
    },
  )
}

#[derive(Clone)]
struct AgentTextProps {
  id: Arc<str>,
  text: Arc<str>,
  project: ProjectId,
  names: ProjectNames,
  links: LinkHandler,
  found: Option<Found>,
}

/// Compared by what it shows; the handler by identity, as the window keeps one.
impl PartialEq for AgentTextProps {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
      && self.text == other.text
      && self.project == other.project
      && self.names == other.names
      && Arc::ptr_eq(&self.links, &other.links)
      && self.found == other.found
  }
}

impl DevtoolsInspectable for AgentTextProps {}

struct AgentText;

impl Component for AgentText {
  type Props = AgentTextProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<AgentTextProps>().clone();
    let pieces = fences::pieces(&props.text);
    if !pieces.iter().any(|piece| matches!(piece, Piece::Code(_))) {
      return markdown(ctx, &props, &props.text, props.found.as_ref())
        .0
        .id(props.id.to_string());
    }
    // Each part marks its own places, the current one at its rank among the places of the parts before it.
    let mut before = 0;
    let mut parts = Vec::with_capacity(pieces.len());
    for (index, piece) in pieces.iter().enumerate() {
      let found = props.found.as_ref().map(|found| found.after(before));
      match piece {
        Piece::Text(text) => {
          let (part, places) = markdown(ctx, &props, &Arc::from(*text), found.as_ref());
          before += places;
          parts.push(part.id(format!("{}-part-{index}", props.id)));
        }
        Piece::Code(block) => {
          before += found.as_ref().map_or(0, |found| found.count(&block.code));
          parts.push(code_block(ctx, (&props.id, index), block, found.as_ref()));
        }
      }
    }
    Column::new()
      .id(props.id.to_string())
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .with_children(parts)
      .into()
  }
}

/// `text`, Markdown of what an agent wrote, as lurq draws it with the chat's links, what a search found in it
/// (`found`) filled behind its words, and how many places it marks.
fn markdown(ctx: &mut Ctx, props: &AgentTextProps, text: &Arc<str>, found: Option<&Found>) -> (Element, usize) {
  let (style, looks) = {
    let theme = ctx.theme();
    let style = FormTextRole {
      typography: TypographyStyle::Body,
      color: PaletteColor::TextPrimary,
    }
    .resolve(&theme.typography(), &theme.palette());
    let looks = ChatLinkLooks::of(theme, &style);
    (style, looks)
  };
  let i18n = ctx.i18n().clone();
  let (project, names) = (props.project, props.names.clone());
  let on_link = props.links.clone();
  let text = references::shown(ctx, text, props.project);
  let markdown = MarkdownProps::styled(text, style)
    .width(Dimension::Pct(100.0))
    .selectable(true)
    .on_link_activate(move |link: &MarkdownLink, event: &MouseEvent| on_link(link.destination(), event))
    .link_presentation(move |link: &MarkdownLink| {
      let place = |route: &Route| Some(place::tip(&i18n, route, (project, &names)));
      let words = LinkWords {
        i18n: &i18n,
        place: &place,
      };
      presentation(link.destination(), link.text(), &looks, &words)
    })
    .list_view(grid::grid);
  let Some(found) = found else {
    return (Markdown::mount(ctx, markdown), 0);
  };
  // The places are searched in the words the document renders, the bytes its fills count in.
  let highlights = found.highlights(ctx, &markdown.rendered_text());
  let places = highlights.len();
  let words = Markdown::mount(ctx, markdown.highlights(highlights));
  (found.classed(words, places), places)
}
