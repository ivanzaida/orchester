//! How a reply shows each link (`agv8r`). A place in a project: a link in the chat's link looks, its tooltip naming
//! the place and that it opens in this tab. A page on the web: the same, followed by its host and an arrow, its
//! tooltip the address it opens in the browser. Anything else: its words as text, followed by its address in mono,
//! or the address alone when the words are empty or the address itself.

use lurq::{
  app::i18n::I18n,
  components::{MarkdownLink, MarkdownLinkPresentation, MarkdownRun},
  markdown::strip_bidi_controls,
};
use orchester_core::id::ProjectId;

use super::{super::data::ProjectNames, place};
use crate::{
  theme::ChatLinkLooks,
  ui::{chat::links::ChatLink, icon::Icon},
};

/// What the presentation words: the app's words, the conversation's project and the projects' names.
pub(super) struct Words {
  i18n: I18n,
  project: ProjectId,
  names: ProjectNames,
}

impl Words {
  pub(super) fn new(i18n: I18n, (project, names): (ProjectId, ProjectNames)) -> Self {
    Self { i18n, project, names }
  }
}

/// How `link` is shown, from the destination a press on it opens.
pub(super) fn presentation(link: &MarkdownLink, looks: &ChatLinkLooks, words: &Words) -> MarkdownLinkPresentation {
  let followed = MarkdownLinkPresentation::link()
    .style(looks.link.clone())
    .hovered(looks.hovered)
    .radius(looks.radius);
  match ChatLink::of(link.destination()) {
    ChatLink::App(route) => followed.tooltip(place::tip(&words.i18n, &route, (words.project, &words.names))),
    ChatLink::Web(url) => {
      let host = strip_bidi_controls(url.host_str().unwrap_or_default()).into_owned();
      followed
        .trailing([
          MarkdownRun::new(" ", looks.host_gap.clone()),
          MarkdownRun::new(host, looks.host.clone()),
          MarkdownRun::new(" ", looks.icon_gap.clone()),
          MarkdownRun::new(Icon::ArrowUpRight.glyph().to_string(), looks.icon.clone()),
        ])
        .tooltip(
          words
            .i18n
            .t_args("orchestrator.link.opensBrowser", [("url", url.as_str())]),
        )
    }
    ChatLink::Text => as_text(link, looks, words),
  }
}

/// A link the chat does not follow: its words as text, then its address.
fn as_text(link: &MarkdownLink, looks: &ChatLinkLooks, words: &Words) -> MarkdownLinkPresentation {
  let address = strip_bidi_controls(link.destination());
  let said = link.text().trim();
  if said == address {
    return MarkdownLinkPresentation::text();
  }
  if said.is_empty() {
    return MarkdownLinkPresentation::text().trailing([MarkdownRun::new(address.into_owned(), looks.address.clone())]);
  }
  let note = words
    .i18n
    .t_args("orchestrator.link.addressNote", [("address", &*address)]);
  MarkdownLinkPresentation::text().trailing([MarkdownRun::plain(" "), MarkdownRun::new(note, looks.address.clone())])
}
