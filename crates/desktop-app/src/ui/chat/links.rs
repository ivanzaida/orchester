//! Links in what an agent writes in a chat (`docs/design/orchestrator-toolset.md`, "Links"). An `orchester:` link
//! names a place in the app by its route path and a press goes there in the active tab; an `https:` link shows its
//! host after it and a person's press opens it in the default browser; synthetic input (an MCP client driving the
//! window) may follow a link in the app but never opens the browser. A link to anything else, or to a route the app
//! does not know, is shown as text with its address and cannot be pressed.

use std::{borrow::Cow, ops::Range, sync::Arc};

use lurq::app::events::is_synthetic_input;
use orchester_exec::external;
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use url::Url;

use crate::{navigation::Navigation, routes::Route, ui::markdown::LinkHandler};

/// The scheme of a link to a place in the app.
const APP_SCHEME: &str = "orchester:";

/// Where a link leads, by its destination.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatLink {
  /// A place in the app.
  App(Route),
  /// A page on the web, opened in the default browser.
  Web(Url),
  /// Nothing the chat follows: another scheme, or a route the app does not know.
  Text,
}

impl ChatLink {
  pub fn of(destination: &str) -> Self {
    if let Some(path) = strip_scheme(destination, APP_SCHEME) {
      return Route::parse(path).map_or(Self::Text, Self::App);
    }
    match Url::parse(destination) {
      Ok(url) if url.scheme() == "https" && url.host_str().is_some() => Self::Web(url),
      _ => Self::Text,
    }
  }
}

/// `destination` past `scheme`, which is matched ignoring case as schemes are.
fn strip_scheme<'a>(destination: &'a str, scheme: &str) -> Option<&'a str> {
  let head = destination.get(..scheme.len())?;
  head.eq_ignore_ascii_case(scheme).then(|| &destination[scheme.len()..])
}

/// What a link the chat cannot follow is shown as, or what a link to the web is shown with; the caller words it.
/// Every value is Markdown source, its data escaped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shown<'a> {
  /// The link as written, then the host it leads to.
  Web { link: &'a str, host: &'a str },
  /// What the link said, then its address, as text.
  Address { text: &'a str, address: &'a str },
}

/// How many times a reply is read again for links its own text made: each time removes one link's syntax at least,
/// so a reply settles in as many as it has links.
const MAX_PASSES: usize = 32;

/// `source` with every link the chat cannot follow made text (what it said and its address, or the address alone
/// when it said nothing else) and every link to the web followed by its host; a link to a place in the app stays as
/// written. A link that holds another is text too. Making a link text can make a new one of the brackets around
/// it, so the reply is read again until no such link is left; a reply that does not settle, or whose links change
/// once the hosts are in, is shown as its source, as text. `words` words what is shown. Borrowed when no link
/// changes.
pub fn present<'a>(source: &'a str, words: &dyn Fn(Shown<'_>) -> String) -> Cow<'a, str> {
  if !source.contains(['[', '<']) {
    return Cow::Borrowed(source);
  }
  let mut text = Cow::Borrowed(source);
  for _ in 0..MAX_PASSES {
    match unfollowed_as_text(&text, words) {
      Some(next) => text = Cow::Owned(next),
      None => return with_hosts(text, words).unwrap_or_else(|| Cow::Owned(escaped(source))),
    }
  }
  Cow::Owned(escaped(source))
}

/// `source` with each outermost link the chat does not follow made text; `None` when it has none.
fn unfollowed_as_text(source: &str, words: &dyn Fn(Shown<'_>) -> String) -> Option<String> {
  let mut out = String::new();
  let mut copied = 0;
  for link in links(source) {
    // Inside a link made text: it is text with it.
    if link.range.start < copied || link.followed() {
      continue;
    }
    let address = escaped(&link.destination);
    let said = link.label.trim();
    let shown = if said.is_empty() || said == link.destination {
      address
    } else {
      words(Shown::Address {
        text: &escaped(said),
        address: &address,
      })
    };
    out.push_str(&source[copied..link.range.start]);
    out.push_str(&shown);
    copied = link.range.end;
  }
  (copied > 0).then(|| out + &source[copied..])
}

/// `source`, whose links the chat all follows, with each link to the web followed by its host; `None` when that
/// changed its links.
fn with_hosts<'a>(source: Cow<'a, str>, words: &dyn Fn(Shown<'_>) -> String) -> Option<Cow<'a, str>> {
  let links = links(&source);
  let mut out = String::new();
  let mut copied = 0;
  for link in &links {
    let ChatLink::Web(url) = ChatLink::of(&link.destination) else {
      continue;
    };
    let shown = words(Shown::Web {
      link: &source[link.range.clone()],
      host: &escaped(url.host_str().unwrap_or_default()),
    });
    out.push_str(&source[copied..link.range.start]);
    out.push_str(&shown);
    copied = link.range.end;
  }
  if copied == 0 {
    return Some(source);
  }
  out.push_str(&source[copied..]);
  let kept = links.iter().map(|link| &link.destination);
  let now = self::links(&out);
  (now.iter().all(WrittenLink::followed) && now.iter().map(|link| &link.destination).eq(kept))
    .then_some(Cow::Owned(out))
}

/// A link of the source: where it is written, what it says as plain text, its destination, and whether it holds
/// another link.
struct WrittenLink {
  range: Range<usize>,
  label: String,
  destination: String,
  holds_link: bool,
}

impl WrittenLink {
  /// Whether the chat follows it: it leads somewhere the chat goes, and holds no other link, whose text would read
  /// as it while a press went here.
  fn followed(&self) -> bool {
    !self.holds_link && ChatLink::of(&self.destination) != ChatLink::Text
  }
}

/// The links of `source`, nested ones too, in the order they start, parsed as lurq's Markdown parses it.
fn links(source: &str) -> Vec<WrittenLink> {
  let mut found = Vec::new();
  let mut open: Vec<WrittenLink> = Vec::new();
  for (event, range) in Parser::new_ext(source, options()).into_offset_iter() {
    let said = match event {
      Event::Start(Tag::Link { dest_url, .. }) => {
        if let Some(outer) = open.last_mut() {
          outer.holds_link = true;
        }
        open.push(WrittenLink {
          range,
          label: String::new(),
          destination: dest_url.into_string(),
          holds_link: false,
        });
        continue;
      }
      Event::End(TagEnd::Link) => {
        found.extend(open.pop());
        continue;
      }
      Event::Text(text)
      | Event::Code(text)
      | Event::InlineMath(text)
      | Event::DisplayMath(text)
      | Event::InlineHtml(text)
      | Event::Html(text) => text,
      Event::SoftBreak | Event::HardBreak => " ".into(),
      _ => continue,
    };
    for link in &mut open {
      link.label.push_str(&said);
    }
  }
  found.sort_by_key(|link| link.range.start);
  found
}

/// The extensions lurq's Markdown reads (`lurq::markdown::parse_markdown`), so both find the same links.
pub(crate) fn options() -> Options {
  Options::ENABLE_TABLES
    | Options::ENABLE_STRIKETHROUGH
    | Options::ENABLE_TASKLISTS
    | Options::ENABLE_FOOTNOTES
    | Options::ENABLE_MATH
}

/// `text` as Markdown that reads as written: every ASCII punctuation character escaped.
fn escaped(text: &str) -> String {
  let mut out = String::with_capacity(text.len());
  for glyph in text.chars() {
    if glyph.is_ascii_punctuation() {
      out.push('\\');
    }
    out.push(glyph);
  }
  out
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
    ChatLink::Web(_) if is_synthetic_input() => Followed::NotOpened,
    ChatLink::Web(url) => match external::open_url(url.as_str()) {
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
