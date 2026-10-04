//! The server item (`qqtph`, states 10.43 `hYres`): the connection to the servers of the open shared
//! projects, after the runner. One server: its name when connected (A), the reconnect attempt and the
//! next try (B), offline since when (C), signed out with *Sign in* (D), and *Not a member of* a
//! project the person was removed from (E), which leads. Several: the worst state leads and the
//! hint lists each (F). No shared project open, no item (G). A press opens Settings › Servers at the
//! server, or the project's Sharing for E; *Sign in* opens the server's sign-in there.

use std::{collections::HashSet, sync::Arc, time::Instant};

use lurq::{
  app::{component::Component, ctx::Ctx, theme::PaletteColor},
  core::Signal,
  layout::layout_kind::ShrinkLimit,
  node::Element,
};
use orchester_core::{id::ServerId, time::Timestamp};

use super::{
  hint::{HintAnchor, HintLine, Hints},
  item::{action, dropping_label, give_way, glyph, item, pressed, value},
  timed::ticking,
};
use crate::{
  display,
  navigation::route::{Page, ServerTab, SettingsTab},
  screens::project::ProjectNav,
  sharing::{
    Connection, ServerEntry,
    feed::{ServerFeed, SignInRequest},
    project::state::ProjectSharing,
  },
  tabs::feed::ProjectFeeds,
  ui::{icon::Icon, link::Press},
};

pub fn server_item(ctx: &mut Ctx, hints: &Hints) -> Vec<Element> {
  let (Some(feed), Some(nav)) = (ctx.use_context::<ServerFeed>(), ctx.use_context::<ProjectNav>()) else {
    return Vec::new();
  };
  let shared = ctx
    .use_context::<ProjectFeeds>()
    .map(|feeds| feeds.sharing())
    .unwrap_or_default();
  if let Some(removed) = shared.iter().find(|sharing| sharing.link.removed_at.is_some()) {
    return vec![not_member(ctx, removed, &nav)];
  }
  let servers: HashSet<ServerId> = shared.iter().map(|sharing| sharing.link.server).collect();
  let list = feed.list.get();
  let in_use: Vec<&ServerEntry> = list
    .entries()
    .iter()
    .filter(|entry| entry.in_use() && servers.contains(&entry.stored.id))
    .collect();
  let Some(worst) = in_use.iter().copied().max_by_key(|entry| severity(entry)) else {
    return Vec::new();
  };
  let connection = worst.connection.unwrap_or(Connection::Checking);
  let (icon, emphasized) = look(connection);
  let color = if emphasized {
    PaletteColor::TextPrimary
  } else {
    PaletteColor::TextSecondary
  };
  let label = if in_use.len() == 1 {
    single_label(ctx, worst, connection)
  } else {
    ctx.t_args("statusBar.server.many", [("count", in_use.len().to_string().as_str())])
  };
  let detail = if in_use.len() == 1 {
    single_detail(ctx, connection)
  } else {
    many_detail(ctx, &in_use, connection)
  };
  let row = item("status-server")
    .describe("state", state_name(connection))
    .flex_shrink(1.0)
    .shrink_order(give_way::SERVER_WORDS)
    .shrink_limit(ShrinkLimit::Content)
    .child(glyph(icon, color.clone()))
    .child(dropping_label(&label, color))
    .with_children(detail);
  let lines: Arc<[HintLine]> = in_use.iter().map(|entry| hint_line(ctx, entry)).collect();
  let row = hints.attach_lines(row, HintAnchor::Server, lines, ctx.t("statusBar.server.hint"));
  let page = Page::Servers(Some(worst.stored.id), ServerTab::Account);
  let mut items = vec![pressed(row, Press::Link(nav.link(page))).into()];
  if in_use.len() == 1 && connection == Connection::SignedOut {
    let (request, server) = (feed.sign_in, worst.stored.id);
    items.push(action(
      "status-server-sign-in",
      &ctx.t("statusBar.server.signIn"),
      Press::action(move || {
        request.set(SignInRequest(Some(server)));
        nav.show(page);
      }),
    ));
  }
  items
}

/// E: the person is no longer a member of an open shared project; a press opens its Sharing.
fn not_member(ctx: &Ctx, sharing: &ProjectSharing, nav: &ProjectNav) -> Element {
  let label = ctx.t_args("statusBar.server.notMember", [("key", sharing.link.key.as_str())]);
  let row = item("status-server")
    .describe("state", "not-member")
    .flex_shrink(1.0)
    .shrink_order(give_way::SERVER_WORDS)
    .shrink_limit(ShrinkLimit::Content)
    .child(glyph(Icon::UserRoundX, PaletteColor::TextPrimary))
    .child(dropping_label(&label, PaletteColor::TextPrimary))
    .child(drop_out(value(sharing.server_name(), PaletteColor::TextMuted)));
  pressed(row, Press::Link(nav.link(Page::Settings(SettingsTab::Sharing)))).into()
}

fn severity(entry: &ServerEntry) -> u8 {
  entry.connection.unwrap_or(Connection::Checking).severity()
}

/// The icon of a state, and whether the item is in the text color (C, D) or muted (A, B).
const fn look(connection: Connection) -> (Icon, bool) {
  match connection {
    Connection::Checking | Connection::Connected => (Icon::Cloud, false),
    Connection::Reconnecting { .. } => (Icon::RefreshCw, false),
    Connection::Offline { .. } => (Icon::CloudOff, true),
    Connection::SignedOut => (Icon::LogOut, true),
  }
}

fn single_label(ctx: &Ctx, entry: &ServerEntry, connection: Connection) -> Arc<str> {
  let name = [("name", entry.stored.name.as_str())];
  match connection {
    Connection::Checking | Connection::Connected => entry.stored.name.as_str().into(),
    Connection::Reconnecting { .. } => ctx.t_args("statusBar.server.reconnecting", name),
    Connection::Offline { .. } => ctx.t_args("statusBar.server.offline", name),
    Connection::SignedOut => ctx.t_args("statusBar.server.signedOut", name),
  }
}

/// The attempt and the next try (B), or since when it is offline and the next try (C); they drop out
/// whole when the bar is narrow.
fn single_detail(ctx: &mut Ctx, connection: Connection) -> Option<Element> {
  let props = match connection {
    Connection::Reconnecting { attempt, next } => RetryProps {
      wait: Wait::Attempt(attempt),
      next,
    },
    Connection::Offline { since, next } => RetryProps {
      wait: Wait::Since(since),
      next,
    },
    Connection::Checking | Connection::Connected | Connection::SignedOut => return None,
  };
  Some(ctx.mount_keyed::<ServerRetry>("status-server-retry", props))
}

/// How many servers share the worst state (F: *1 offline*); nothing when all are connected.
fn many_detail(ctx: &Ctx, entries: &[&ServerEntry], worst: Connection) -> Option<Element> {
  let count = entries
    .iter()
    .filter(|entry| entry.connection.unwrap_or(Connection::Checking).severity() == worst.severity())
    .count()
    .to_string();
  let key = match worst {
    Connection::Checking | Connection::Connected => return None,
    Connection::Reconnecting { .. } => "statusBar.server.manyReconnecting",
    Connection::Offline { .. } => "statusBar.server.manyOffline",
    Connection::SignedOut => "statusBar.server.manySignedOut",
  };
  Some(drop_out(value(
    &ctx.t_args(key, [("count", count.as_str())]),
    PaletteColor::TextMuted,
  )))
}

/// A server's line in the hint: name, state, and its address, account and when it was last reached.
fn hint_line(ctx: &Ctx, entry: &ServerEntry) -> HintLine {
  let connection = entry.connection.unwrap_or(Connection::Checking);
  let state = ctx.t(match connection {
    Connection::Checking => "statusBar.server.state.checking",
    Connection::Connected => "statusBar.server.state.connected",
    Connection::Reconnecting { .. } => "statusBar.server.state.reconnecting",
    Connection::Offline { .. } => "statusBar.server.state.offline",
    Connection::SignedOut => "statusBar.server.state.signedOut",
  });
  let username = entry
    .stored
    .account
    .as_ref()
    .map_or("", |account| account.username.as_str());
  let detail = match (connection, entry.last_reached) {
    (Connection::Offline { since, .. }, _) => ctx.t_args(
      "statusBar.server.detailSince",
      [("host", entry.address.host()), ("time", display::clock(since).as_str())],
    ),
    (_, Some(reached)) => ctx.t_args(
      "statusBar.server.detailReached",
      [
        ("host", entry.address.host()),
        ("username", username),
        ("time", display::clock(reached).as_str()),
      ],
    ),
    (_, None) => ctx.t_args(
      "statusBar.server.detail",
      [("host", entry.address.host()), ("username", username)],
    ),
  };
  HintLine {
    icon: look(connection).0,
    name: entry.stored.name.as_str().into(),
    state,
    detail,
  }
}

/// For tooling (`lurq_read_tree`): the state the item shows.
const fn state_name(connection: Connection) -> &'static str {
  match connection {
    Connection::Checking => "checking",
    Connection::Connected => "connected",
    Connection::Reconnecting { .. } => "reconnecting",
    Connection::Offline { .. } => "offline",
    Connection::SignedOut => "signed-out",
  }
}

/// A mono detail that keeps its size or drops out whole with the times.
fn drop_out(detail: Element) -> Element {
  lurq::components::Row::new()
    .flex_shrink(1.0)
    .shrink_order(give_way::TIMES)
    .shrink_limit(ShrinkLimit::Drop)
    .child(detail)
    .into()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Wait {
  Attempt(u32),
  Since(Timestamp),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RetryProps {
  wait: Wait,
  next: Instant,
}

impl lurq::app::component::DevtoolsInspectable for RetryProps {}

/// The detail of B and C, counting down to the next try every second.
struct ServerRetry {
  tick: Signal<u64>,
}

impl Component for ServerRetry {
  type Props = RetryProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self { tick: ticking(ctx) }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    self.tick.get();
    let props = *ctx.props::<RetryProps>();
    let seconds = props
      .next
      .saturating_duration_since(Instant::now())
      .as_millis()
      .div_ceil(1000)
      .to_string();
    let text = match props.wait {
      Wait::Attempt(attempt) => ctx.t_args(
        "statusBar.server.attempt",
        [("attempt", attempt.to_string().as_str()), ("seconds", seconds.as_str())],
      ),
      Wait::Since(since) => ctx.t_args(
        "statusBar.server.since",
        [("time", display::clock(since).as_str()), ("seconds", seconds.as_str())],
      ),
    };
    drop_out(value(&text, PaletteColor::TextMuted))
  }
}
