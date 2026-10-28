//! The top of the page (`Header` `w3Z3Cp`): what the page shows — the Review hub's head and tab name it — and at its end the
//! repository (`Repository label` `mhbBg`), Refresh and the sync status under them. Refresh waits while a
//! sync runs and until a rate limit resets, and says so under itself (`Reason · Refresh` in `uuaob`).
//! Without a linked repository there is nothing to refresh: only the status says GitHub is not connected,
//! 6 under the empty row of buttons (`Actions` `y97UZW` in 08.D, `nG8p5`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{repository::GitHubRepo, time::Timestamp};

use super::status::{Shown, status_line};
use crate::{
  theme::{ControlHeight, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    github_mark::github_mark,
    icon::{Icon, icon},
  },
};

/// What Refresh can do now.
pub(super) enum Refresh {
  /// Syncs on a press.
  Ready(std::sync::Arc<dyn Fn() + Send + Sync>),
  /// A sync runs: Refresh waits for it.
  Syncing,
  /// A rate limit holds syncs back until it resets.
  RateLimited,
}

/// What the header's end shows: nothing to refresh without a repository.
pub(super) struct Linked<'a> {
  pub repo: &'a GitHubRepo,
  pub refresh: Refresh,
}

pub(super) fn header(ctx: &mut Ctx, linked: Option<Linked<'_>>, shown: &Shown, now: Timestamp) -> Row {
  // The Review hub's head and tab name the page (`screens::review`): what it shows leads it.
  let title = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .child(
      Text::new(&ctx.t("pull_requests.description"))
        .id("pull-requests-description")
        .variant(TypographyStyle::Body)
        .color(PaletteColor::TextSecondary),
    );
  let actions = Column::new()
    .id("pull-requests-actions")
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::End);
  let Some(linked) = linked else {
    let status = Row::new()
      .padding(Padding::new().top(ExtraSpacing::Snug.role()))
      .child(status_line(ctx, shown, now));
    return frame(title, actions.child(status));
  };
  let reason = reason(ctx, &linked.refresh);
  let actions = actions
    .child(
      Row::new()
        .spacing(SpacingSize::Md)
        .align_items(Alignment::Center)
        .child(repository(ctx, linked.repo))
        .child(refresh(ctx, linked.refresh)),
    )
    .with_children(reason)
    .child(status_line(ctx, shown, now));
  frame(title, actions)
}

fn frame(title: Column, actions: Column) -> Row {
  Row::new()
    .id("pull-requests-header")
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().bottom(SpacingSize::Xs))
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::Start)
    .child(title)
    .child(actions)
}

/// The linked repository, read only.
fn repository(ctx: &Ctx, repo: &GitHubRepo) -> Row {
  let name = ctx.t_args(
    "pull_requests.repository",
    [("owner", repo.owner()), ("name", repo.name())],
  );
  Row::new()
    .id("pull-requests-repository")
    .height(Dimension::Px(ControlHeight::Md.value()))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(github_mark(ctx, IconSize::Base, PaletteColor::TextMuted))
    .child(Text::new(&name).variant(TypographyStyle::Description).nowrap())
}

fn refresh(ctx: &Ctx, refresh: Refresh) -> ActionButton {
  let button = ActionButton::new(
    "pull-requests-refresh",
    ButtonKind::Secondary,
    ctx.t("pull_requests.refresh"),
  )
  .icon(Icon::RefreshCw);
  match refresh {
    Refresh::Ready(sync) => button.on_click(move || sync()),
    Refresh::Syncing | Refresh::RateLimited => button.disabled(),
  }
}

/// Why Refresh waits, under it: a 12 lock and the words in 11.5 `text_muted`, 6 apart.
fn reason(ctx: &Ctx, refresh: &Refresh) -> Option<Element> {
  let key = match refresh {
    Refresh::Ready(_) => return None,
    Refresh::Syncing => "pull_requests.refreshAfterSync",
    Refresh::RateLimited => "pull_requests.refreshAfterReset",
  };
  Some(
    Row::new()
      .id("pull-requests-refresh-reason")
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .describe("role", "unavailable-reason")
      .child(icon(Icon::Lock, IconSize::Sm, PaletteColor::TextMuted))
      .child(Text::new(&ctx.t(key)).variant(ExtraTypography::Note.role()).nowrap())
      .into(),
  )
}
