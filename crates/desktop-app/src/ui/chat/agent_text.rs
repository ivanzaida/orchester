//! What an agent wrote in a chat, the task Conversation's and the Orchestrator's alike (`Text` `D8JGX4` in `Agent
//! message` `dcd0m`; `Orchestrator · reply with links` `w262B`, `Orchestrator · links · states` `agv8r`): lurq's
//! Markdown in 13/1.5 `text_primary`, the places it names as text linked ([`super::references`]), with every link
//! presented as the chat follows it ([`super::presentation`]), a link to a place in a project with a tooltip that
//! names it ([`super::place`]), and a list of labelled links drawn as their grid ([`super::grid`]). Every text goes
//! through the presentation, which lurq runs on the exact destination a press opens: no link shows words from one
//! destination and opens another.

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
use orchester_core::id::ProjectId;

use super::{
  grid,
  names::ProjectNames,
  place,
  presentation::{LinkWords, presentation},
  references,
};
use crate::{routes::Route, theme::ChatLinkLooks, ui::markdown::LinkHandler};

/// What an agent wrote, `text`, in a chat of `project`; `names` are the projects' names, for what a link to another
/// project's place says, and a press on a link goes to `links`. `id` names it.
pub fn agent_text(
  ctx: &mut Ctx,
  id: &str,
  text: &Arc<str>,
  (project, names): (ProjectId, &ProjectNames),
  links: &LinkHandler,
) -> Element {
  ctx.mount_keyed::<AgentText>(
    &format!("agent-text-{id}"),
    AgentTextProps {
      id: Arc::from(id),
      text: text.clone(),
      project,
      names: names.clone(),
      links: links.clone(),
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
}

/// Compared by what it shows; the handler by identity, as the window keeps one.
impl PartialEq for AgentTextProps {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
      && self.text == other.text
      && self.project == other.project
      && self.names == other.names
      && Arc::ptr_eq(&self.links, &other.links)
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
    let text = references::shown(ctx, &props.text, props.project);
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
    Markdown::mount(ctx, markdown).id(props.id.to_string())
  }
}
