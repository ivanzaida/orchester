//! An Orchestrator's reply (`Orchestrator · reply with links` `w262B`, `Orchestrator · links · states` `agv8r`):
//! lurq's Markdown, the places it names as text linked (`ui::chat::references`), with every link presented as the
//! chat follows it (`present`) and a list of labelled links drawn as their grid (`grid`). Every reply goes through
//! the presentation, which lurq runs on the exact destination a press opens: no link shows words from one
//! destination and opens another.

mod grid;
mod place;
mod present;

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

use self::present::Words;
use super::data::ProjectNames;
use crate::{
  theme::ChatLinkLooks,
  ui::{chat::references, markdown::LinkHandler},
};

/// The reply `text`, its links followed by `links`; `project` is the conversation's, `names` the projects' names.
/// `id` names it.
pub(super) fn reply(
  ctx: &mut Ctx,
  id: &str,
  text: &Arc<str>,
  (project, names): (ProjectId, &ProjectNames),
  links: &LinkHandler,
) -> Element {
  ctx.mount_keyed::<Reply>(
    &format!("reply-{id}"),
    ReplyProps {
      id: Arc::from(id),
      text: text.clone(),
      project,
      names: names.clone(),
      links: links.clone(),
    },
  )
}

#[derive(Clone)]
struct ReplyProps {
  id: Arc<str>,
  text: Arc<str>,
  project: ProjectId,
  names: ProjectNames,
  links: LinkHandler,
}

/// Compared by what it shows; the handler by identity, as the screen keeps one.
impl PartialEq for ReplyProps {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
      && self.text == other.text
      && self.project == other.project
      && self.names == other.names
      && Arc::ptr_eq(&self.links, &other.links)
  }
}

impl DevtoolsInspectable for ReplyProps {}

struct Reply;

impl Component for Reply {
  type Props = ReplyProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ReplyProps>().clone();
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
    let words = Words::new(ctx.i18n().clone(), (props.project, props.names.clone()));
    let on_link = props.links.clone();
    let text = references::shown(ctx, &props.text, props.project);
    let markdown = MarkdownProps::styled(text, style)
      .width(Dimension::Pct(100.0))
      .selectable(true)
      .on_link_activate(move |link: &MarkdownLink, event: &MouseEvent| on_link(link.destination(), event))
      .link_presentation(move |link| present::presentation(link, &looks, &words))
      .list_view(grid::grid);
    Markdown::mount(ctx, markdown).id(props.id.to_string())
  }
}
