//! The `orchester://` link (`docs/architecture/server.md`, Invites › *Accepting outside Orchester*, row 77): a page in
//! the browser (the cloud's Invites page, the team server's landing) starts the app with [`ARGUMENT`] and the link, and
//! the window that has the data folder open takes it.
//!
//! - Forms: `orchester://invite/<invite id>` opens that invite in Settings › Orchester, and `orchester://open` brings
//!   the window to the front. Anything else is [`OpenLink::Unknown`]: the window comes to the front with *This link
//!   isn't for anything here* and nothing else happens.
//! - A link carries ids only and acts on nothing by itself: it never accepts, declines, joins, connects or sends
//!   without the person's click, never reaches an agent, a chat, a prompt, memory or the app's MCP tools, and is logged
//!   by its form only ([`OpenLink::form`]).
//! - Windows: the installer registers the scheme per user (`tools/installer/orchester.iss`); the app never does, so a
//!   development build or a QA copy never takes links. A program started with a link while another app holds the data
//!   folder hands the link over its pipe ([`pipe`]) and ends. macOS is not supported yet: there the link opens the
//!   *already open* screen, and a running app takes no links.

pub mod host;
mod inbox;
pub mod pipe;

use std::ffi::OsString;

use orchester_core::id::InviteId;

pub use self::{host::LinkHost, inbox::LinkInbox};

/// The command-line switch the installer registers the scheme with: `"{app}\orchester.exe" --open-link "%1"`.
pub const ARGUMENT: &str = "--open-link";
/// The longest link the pipe carries, in bytes; a longer one is no link of Orchester's.
pub const MAX_LINE: usize = 256;

const SCHEME: &str = "orchester://";
const INVITE: &str = "invite/";
const OPEN: &str = "open";
/// The length of an id in its hyphenated form, the only form a link carries.
const HYPHENATED_ID: usize = 36;

/// What a link asks for, read by the app that takes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenLink {
  /// `orchester://invite/<id>`: Settings › Orchester at that invite's row, when the account here lists it.
  Invite(InviteId),
  /// `orchester://open`: the window to the front.
  Open,
  /// Any other text: the window to the front with *This link isn't for anything here*.
  Unknown,
}

impl OpenLink {
  /// Reads `link`. The scheme is matched without regard to case, as browsers may write it; the rest exactly, with no
  /// query string, fragment or other path.
  pub fn parse(link: &str) -> Self {
    if link.len() > MAX_LINE {
      return Self::Unknown;
    }
    let Some(rest) = link
      .get(..SCHEME.len())
      .filter(|scheme| scheme.eq_ignore_ascii_case(SCHEME))
      .and_then(|_| link.get(SCHEME.len()..))
    else {
      return Self::Unknown;
    };
    if rest == OPEN {
      return Self::Open;
    }
    rest
      .strip_prefix(INVITE)
      .filter(|id| id.len() == HYPHENATED_ID)
      .and_then(|id| id.parse().ok())
      .map_or(Self::Unknown, Self::Invite)
  }

  /// The link's form, which is all the log says of it: never the id or the text.
  pub const fn form(self) -> &'static str {
    match self {
      Self::Invite(_) => "orchester://invite/…",
      Self::Open => "orchester://open",
      Self::Unknown => "unknown",
    }
  }
}

/// The link after [`ARGUMENT`] on the command line `args`, unread; `None` without the switch. A switch with nothing
/// after it is an empty link, which reads as [`OpenLink::Unknown`].
pub fn argument(args: impl IntoIterator<Item = OsString>) -> Option<String> {
  let mut args = args.into_iter().skip_while(|arg| arg != ARGUMENT);
  args.next()?;
  Some(
    args
      .next()
      .map(|link| link.to_string_lossy().into_owned())
      .unwrap_or_default(),
  )
}
