//! Links in what an agent writes in a chat (`docs/design/orchestrator-toolset.md`, "Links"). An `orchester:` link
//! names a place in the app by its route path and a press goes there in the active tab; an `https:` link shows its
//! host after it and a person's press opens it in the default browser; synthetic input (an MCP client driving the
//! window) may follow a link in the app but never opens the browser. A link to anything else, to a route the app
//! does not know, or to a place outside a project (Add project, Settings: what an agent may never open for the
//! person) is shown as text with its address and cannot be pressed. How every Markdown draws each kind is
//! `presentation`'s; the window follows them with one handler, [`ChatLinks`].

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, events::is_synthetic_input},
  markdown::strip_bidi_controls,
};
use orchester_exec::external;
use url::Url;

use crate::{navigation::Navigation, routes::Route, ui::markdown::LinkHandler};

/// The scheme of a link to a place in the app.
const APP_SCHEME: &str = "orchester:";

/// Where a link leads, by its destination.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatLink {
  /// A place in a project of the app: every link the tools return is one.
  App(Route),
  /// A page on the web, opened in the default browser, and its host as shown: without bidirectional controls.
  Web { url: Url, host: String },
  /// Nothing the chat follows: another scheme, a route the app does not know, or one outside a project, such as
  /// Add project with a folder, which would read that folder.
  Text,
}

impl ChatLink {
  pub fn of(destination: &str) -> Self {
    if let Some(path) = strip_scheme(destination, APP_SCHEME) {
      return match Route::parse(path) {
        Ok(route @ Route::Project(..)) => Self::App(route),
        Ok(_) | Err(_) => Self::Text,
      };
    }
    let Ok(url) = Url::parse(destination) else {
      return Self::Text;
    };
    match url.host_str() {
      Some(host) if url.scheme() == "https" => Self::Web {
        host: strip_bidi_controls(host).into_owned(),
        url,
      },
      _ => Self::Text,
    }
  }
}

/// `destination` past `scheme`, which is matched ignoring case as schemes are.
fn strip_scheme<'a>(destination: &'a str, scheme: &str) -> Option<&'a str> {
  let head = destination.get(..scheme.len())?;
  head.eq_ignore_ascii_case(scheme).then(|| &destination[scheme.len()..])
}

/// What happened to a press on a link.
#[derive(Debug)]
pub enum Followed {
  /// The active tab went to the place.
  Went,
  /// The default browser was asked to open the page.
  Opened,
  /// Synthetic input pressed a link to the web, which only a person opens.
  NotOpened,
  /// The browser could not be asked: no launcher on this platform, or it did not start.
  Failed(std::io::Error),
  /// The destination is nothing the chat follows; it is shown as text, so no press should reach it.
  NotALink,
}

/// Follows a press on the link to `destination`.
pub fn follow(navigation: &Navigation, destination: &str) -> Followed {
  match ChatLink::of(destination) {
    ChatLink::App(route) => {
      navigation.go(route);
      Followed::Went
    }
    ChatLink::Web { .. } if is_synthetic_input() => Followed::NotOpened,
    ChatLink::Web { url, .. } => match external::open_url(url.as_str()) {
      Ok(()) => Followed::Opened,
      Err(error) => Followed::Failed(error),
    },
    ChatLink::Text => Followed::NotALink,
  }
}

/// What a press on a link in a chat does: [`follow`] in `navigation`. A screen makes it once and keeps it, so a
/// message is laid out again only when what it says changes.
pub fn handler(navigation: Navigation) -> LinkHandler {
  Arc::new(move |destination: &str| match follow(&navigation, destination) {
    Followed::Went | Followed::Opened => {}
    Followed::NotOpened => {
      tracing::warn!("synthetic input pressed a link to the web; it opens only on a person's press")
    }
    Followed::Failed(error) => tracing::warn!(%error, "the message's link was not opened"),
    Followed::NotALink => tracing::warn!("a press reached a link that is shown as text; nothing followed"),
  })
}

/// The window's link handler, which every Markdown of the window follows its links with: made once, so a message is
/// laid out again only when what it says changes. The root provides it beside the navigation.
#[derive(Clone)]
pub struct ChatLinks(LinkHandler);

impl ChatLinks {
  pub fn new(navigation: Navigation) -> Self {
    Self(handler(navigation))
  }

  /// The handler the window provides.
  pub fn of(ctx: &mut Ctx) -> LinkHandler {
    ctx
      .use_context::<Self>()
      .expect("the window provides its link handler beside the navigation")
      .0
  }
}
