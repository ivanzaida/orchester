//! How every Markdown of the window shows a link (`Orchestrator · links · states` `agv8r`), in lurq's link
//! presentation, which runs on the exact destination a press opens. A place in a project: a link in the chat's link
//! looks, with the tooltip its caller words, if any. A page on the web: the same, followed by its host and an arrow,
//! its tooltip the address it opens in the browser; with no words, its address as text, so no host can be placed
//! without the link it belongs to. lurq 0.46 already shows a link whose words draw nothing as those words, without
//! asking: the check here is defence in depth, should a caller present such a link. lurq also draws every word and
//! run without bidirectional controls, so nothing here strips them. Anything else: its words as text, followed by its
//! address in mono, or the address alone when the words are empty or the address itself.

use std::sync::Arc;

use lurq::{
  app::i18n::I18n,
  components::{MarkdownLinkPresentation, MarkdownRun},
};

use super::links::ChatLink;
use crate::{routes::Route, theme::ChatLinkLooks, ui::icon::Icon};

/// What words a link's presentation: the app's words, and the tooltip of a place in a project when the caller can
/// name the place.
pub struct LinkWords<'a> {
  pub i18n: &'a I18n,
  pub place: &'a dyn Fn(&Route) -> Option<Arc<str>>,
}

/// How a link to `destination` that says `said` is shown.
pub fn presentation(
  destination: &str,
  said: &str,
  looks: &ChatLinkLooks,
  words: &LinkWords<'_>,
) -> MarkdownLinkPresentation {
  let followed = MarkdownLinkPresentation::link()
    .style(looks.link.clone())
    .hovered(looks.hovered)
    .focused(looks.focused)
    .radius(looks.radius);
  match ChatLink::of(destination) {
    ChatLink::App(route) => match (words.place)(&route) {
      Some(tip) => followed.tooltip(tip),
      None => followed,
    },
    ChatLink::Web { .. } if said.trim().is_empty() => as_text(destination, said, looks, words),
    ChatLink::Web { url, host } => followed
      .trailing([
        MarkdownRun::new(" ", looks.host_gap.clone()),
        MarkdownRun::new(host, looks.host.clone()),
        MarkdownRun::new(" ", looks.icon_gap.clone()),
        MarkdownRun::new(Icon::ArrowUpRight.glyph().to_string(), looks.icon.clone()),
      ])
      .tooltip(words.i18n.t_args("ui.link.opensBrowser", [("url", url.as_str())])),
    ChatLink::Text => as_text(destination, said, looks, words),
  }
}

/// A link the chat does not follow, or one to the web with no words: its words as text, then its address.
fn as_text(destination: &str, said: &str, looks: &ChatLinkLooks, words: &LinkWords<'_>) -> MarkdownLinkPresentation {
  let said = said.trim();
  if said == destination {
    return MarkdownLinkPresentation::text();
  }
  if said.is_empty() {
    return MarkdownLinkPresentation::text().trailing([MarkdownRun::new(destination, looks.address.clone())]);
  }
  let note = words.i18n.t_args("ui.link.addressNote", [("address", destination)]);
  MarkdownLinkPresentation::text().trailing([MarkdownRun::plain(" "), MarkdownRun::new(note, looks.address.clone())])
}
