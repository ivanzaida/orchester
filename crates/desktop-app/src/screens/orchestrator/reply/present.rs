//! How a reply shows each link (`agv8r`): as every Markdown of the window does (`ui::chat::presentation`), a link to
//! a place in a project with its tooltip naming the place and that it opens in this tab.

use lurq::{
  app::i18n::I18n,
  components::{MarkdownLink, MarkdownLinkPresentation},
};
use orchester_core::id::ProjectId;

use super::{super::data::ProjectNames, place};
use crate::{
  routes::Route,
  theme::ChatLinkLooks,
  ui::chat::presentation::{LinkWords, presentation as chat_presentation},
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

/// How `link` is shown, from the destination a press on it opens: as every Markdown of the window shows it, a place
/// in a project with a tooltip that names it.
pub(super) fn presentation(link: &MarkdownLink, looks: &ChatLinkLooks, words: &Words) -> MarkdownLinkPresentation {
  let place = |route: &Route| Some(place::tip(&words.i18n, route, (words.project, &words.names)));
  let shown = LinkWords {
    i18n: &words.i18n,
    place: &place,
  };
  chat_presentation(link.destination(), link.text(), looks, &shown)
}
