//! A shared project's task list (10.42, 10.44): beside the description where the list stands with
//! its server; while it is read-only, a notice that says why, when the copy here was last synced,
//! what waits and what to do (*Try now*, *Sign in*, *Make local…*, *Check again*), and the footer's
//! last sync. New tasks, Create multiple, Manage tags and the bulk changes are unavailable then; the
//! search, the filters and opening tasks keep working.

use std::{sync::Arc, time::Instant};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, TypographyStyle},
  },
  components::{Row, Text},
  layout::Alignment,
  node::Element,
};
use orchester_core::{sharing::CLAIM_STALE_MS, time::Timestamp};

use super::super::ProjectNav;
use crate::{
  display,
  navigation::route::{Page, ServerTab, SettingsTab},
  sharing::{
    feed::{ServerFeed, SignInRequest},
    project::state::{ProjectSharing, TaskListAccess},
  },
  theme::ExtraSpacing,
  ui::{
    button::ActionButton,
    icon::{Icon, IconSize, icon},
    notice::Notice,
  },
};

/// Where the list stands with its server, beside the description (`rLZVU`, `Kori8`).
pub(super) fn badge(ctx: &Ctx, sharing: Option<&ProjectSharing>) -> Option<Element> {
  let sharing = sharing?;
  let (glyph, text) = match sharing.access() {
    TaskListAccess::Writable => (
      Icon::Cloud,
      ctx.t_args(
        "tasks.shared.on",
        [("server", sharing.server_name()), ("key", sharing.link.key.as_str())],
      ),
    ),
    TaskListAccess::Offline { synced, .. } => (
      Icon::CloudOff,
      ctx.t_args("tasks.shared.offline", [("time", display::clock(synced).as_str())]),
    ),
    TaskListAccess::SignedOut { synced } => (
      Icon::CloudOff,
      ctx.t_args(
        "tasks.shared.signedOut",
        [
          ("server", sharing.server_name()),
          ("time", display::clock(synced).as_str()),
        ],
      ),
    ),
    TaskListAccess::NotMember { since } => (
      Icon::UserRoundX,
      ctx.t_args(
        "tasks.shared.notMember",
        [
          ("key", sharing.link.key.as_str()),
          ("time", display::clock(since).as_str()),
        ],
      ),
    ),
  };
  Some(
    Row::new()
      .id("tasks-shared")
      .spacing(ExtraSpacing::Tight.role())
      .align_items(Alignment::Center)
      .child(icon(glyph, IconSize::Small, PaletteColor::TextPrimary))
      .child(
        Text::new(&text)
          .variant(TypographyStyle::Caption)
          .color(PaletteColor::TextPrimary),
      )
      .into(),
  )
}

/// Why the list is read-only and what to do about it; `None` while it takes changes.
pub(super) fn notice(ctx: &mut Ctx, sharing: Option<&ProjectSharing>, nav: &ProjectNav) -> Option<Element> {
  let sharing = sharing?;
  let feed = ServerFeed::of(ctx);
  let notice = match sharing.access() {
    TaskListAccess::Writable => return None,
    TaskListAccess::Offline { synced, since, next } => offline(ctx, sharing, feed, (synced, since, next)),
    TaskListAccess::SignedOut { synced } => signed_out(ctx, sharing, (feed, nav.clone()), synced),
    TaskListAccess::NotMember { since } => not_member(ctx, sharing, (feed, nav), since),
  };
  Some(notice.emphasized().id("tasks-shared-notice").into())
}

/// 10.42: the server cannot be reached; *Try now* and when the next attempt starts.
fn offline(
  ctx: &Ctx,
  sharing: &ProjectSharing,
  feed: ServerFeed,
  (synced, since, next): (Timestamp, Option<Timestamp>, Instant),
) -> Notice {
  let since = since.unwrap_or(synced);
  // Claims go stale 15 minutes after the last heartbeat, at about the last sync.
  let stale = Timestamp::from_unix_millis(synced.unix_millis().saturating_add(CLAIM_STALE_MS));
  let (servers, server) = (feed.servers, sharing.link.server);
  Notice::new(
    Icon::CloudOff,
    ctx.t("tasks.shared.offlineHeading"),
    ctx.t_args(
      "tasks.shared.offlineDetail",
      [
        ("server", sharing.server_name()),
        ("since", display::clock(since).as_str()),
        ("stale", display::clock(stale).as_str()),
      ],
    ),
  )
  .below(
    buttons(vec![
      ActionButton::new(ctx.t("tasks.shared.tryNow"))
        .icon(Icon::RefreshCw)
        .id("tasks-shared-try-now")
        .on_click(move || servers.retry_now(server))
        .render(ctx),
    ])
    .child(Text::new(&next_try(ctx, next)).variant(TypographyStyle::Caption)),
  )
}

/// Nobody on this computer is signed in to the server any more: *Sign in*.
fn signed_out(ctx: &Ctx, sharing: &ProjectSharing, (feed, nav): (ServerFeed, ProjectNav), synced: Timestamp) -> Notice {
  let (request, id, server) = (feed.sign_in, sharing.link.server, sharing.server_name());
  Notice::new(
    Icon::CloudOff,
    ctx.t_args("tasks.shared.signedOutHeading", [("server", server)]),
    ctx.t_args(
      "tasks.shared.signedOutDetail",
      [("server", server), ("time", display::clock(synced).as_str())],
    ),
  )
  .below(buttons(vec![
    ActionButton::new(ctx.t("servers.signIn.action"))
      .primary()
      .icon(Icon::LogIn)
      .id("tasks-shared-sign-in")
      .on_click(move || {
        nav.show(Page::Servers(Some(id), ServerTab::Account));
        request.set(SignInRequest(Some(id)));
      })
      .render(ctx),
  ]))
}

/// 10.44: the person is no longer a member; *Make local…* and *Check again*.
fn not_member(ctx: &Ctx, sharing: &ProjectSharing, (feed, nav): (ServerFeed, &ProjectNav), since: Timestamp) -> Notice {
  let servers = feed.servers;
  let key = sharing.link.key.as_str();
  let username = sharing.username.clone().unwrap_or_default();
  Notice::new(
    Icon::UserRoundX,
    ctx.t_args(
      "tasks.shared.notMemberHeading",
      [("key", key), ("server", sharing.server_name())],
    ),
    ctx.t_args(
      "tasks.shared.notMemberDetail",
      [
        ("time", display::clock(since).as_str()),
        ("key", key),
        ("username", username.as_str()),
      ],
    ),
  )
  .below(buttons(vec![
    ActionButton::new(ctx.t("sharing.makeLocal.action"))
      .icon(Icon::HardDriveDownload)
      .id("tasks-shared-make-local")
      .link(nav.link(Page::Settings(SettingsTab::Sharing)))
      .render(ctx),
    ActionButton::new(ctx.t("tasks.shared.checkAgain"))
      .icon(Icon::RefreshCw)
      .id("tasks-shared-check-again")
      .on_click(move || servers.sync_now())
      .render(ctx),
  ]))
}

fn buttons(buttons: Vec<Element>) -> Row {
  Row::new()
    .padding_top(ExtraSpacing::Tight.role())
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .with_children(buttons)
}

/// When the next attempt starts, in seconds from now.
fn next_try(ctx: &Ctx, next: Instant) -> Arc<str> {
  let seconds = next.saturating_duration_since(Instant::now()).as_secs();
  ctx.t_args("tasks.shared.nextTry", [("seconds", seconds.to_string().as_str())])
}

/// The footer's note while the list is read-only: as of its last sync.
pub(super) fn footer_sync(ctx: &Ctx, sharing: Option<&ProjectSharing>) -> Option<Arc<str>> {
  let sharing = sharing?;
  let at = match sharing.access() {
    TaskListAccess::Writable => return None,
    TaskListAccess::Offline { synced, .. } | TaskListAccess::SignedOut { synced } => synced,
    TaskListAccess::NotMember { since } => since,
  };
  Some(ctx.t_args("tasks.shared.asSynced", [("time", display::clock(at).as_str())]))
}
