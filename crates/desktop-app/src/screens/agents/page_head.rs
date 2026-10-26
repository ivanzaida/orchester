//! The top of a profile's page (`Top` `WSnxI` in 01b): the crumb back to Profiles, the page head with the CLI's mark,
//! the profile's name, its permissions in a few words and what enforces them, and the … menu with Remove; then the
//! draft bar (`W0sRO`) with Discard draft and Publish. 01e draws a profile whose connection cannot enforce it
//! (Publish unavailable: *Change the connection to publish*) and a stale draft.
use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::{Column, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::resource::AgentProfile;
use orchester_workspace::agents::data::{ProfileEntry, ProjectAgents, VersionState};

use super::{AgentsNav, Shown, entries, fields::Fields, policy_panel};
use crate::{
  screens::hub::{
    draft_bar::{DraftBar, Standing},
    parts::{status, tile},
  },
  theme::{ExtraSpacing, HubText, IconSize},
  ui::{
    icon::{Icon, icon},
    icon_button::IconButton,
    menu::{MenuItem, menu_at_end, menu_panel},
    pressable::opening,
  },
};

/// ← Profiles.
pub fn crumb(ctx: &Ctx, nav: &AgentsNav) -> Element {
  opening(nav.to(Shown::List))
    .id("agent-crumb")
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::ArrowLeft, IconSize::Base, PaletteColor::TextMuted))
    .child(Text::new(&ctx.t("hub.tabs.profiles")).variant(HubText::Crumb.role()))
    .into()
}

/// The … menu under the head: Remove.
pub struct More {
  pub open: Signal<bool>,
  pub anchor: ElementRef,
  pub remove: Arc<dyn Fn() + Send + Sync>,
}

pub fn head(ctx: &mut Ctx, (fields, listed): (&Fields, &ProjectAgents), more: Option<More>) -> Element {
  let role = fields.role.get();
  let title = if role.trim().is_empty() {
    ctx.t("agents.new")
  } else {
    role.as_str().into()
  };
  let (summary, enforced) = policy_panel::summary(ctx, fields, listed);
  let sub = Row::new()
    .id("agent-permission-summary")
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(Text::new(&summary).variant(HubText::Hint.role()))
    .with_children(enforced.map(|(tone, text)| {
      Row::new()
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(Text::new(&ctx.t("agents.summarySeparator")).variant(HubText::Hint.role()))
        .child(status("agent-enforced", tone, &text))
    }));
  let menu = more.map(|more| {
    let remove = more.remove.clone();
    let item = MenuItem::new("agent-remove", ctx.t("hub.remove.action"), move || remove())
      .icon(Icon::Trash2)
      .render(ctx, &more.open);
    let open = more.open.clone();
    let button = IconButton::new("agent-more", Icon::Ellipsis, ctx.t("hub.more"))
      .small()
      .element(more.anchor.clone())
      .on_click(move || open.update(|open| *open = !*open))
      .render(ctx);
    Column::new()
      .child(button)
      .child(menu_at_end(more.anchor, more.open, menu_panel("agent-more-menu").child(item)))
  });
  Row::new()
    .id("agent-page-head")
    .width(Dimension::Pct(100.0))
    .spacing(lurq::app::theme::SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(tile(entries::provider_glyph(fields.connection(listed)), true))
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Fine.role())
        .child(Text::new(&title).id("agent-title").variant(HubText::PageTitle.role()))
        .child(sub),
    )
    .with_children(menu)
    .into()
}

/// What the draft bar shows and offers.
pub struct Bar<'a> {
  pub entry: Option<&'a ProfileEntry>,
  pub published: Option<&'a AgentProfile>,
  pub saved_at: Option<orchester_core::time::Timestamp>,
  /// The connection can run and enforce the profile, so it can be published.
  pub publishable: bool,
  pub actions: Vec<Element>,
}

pub fn bar(ctx: &Ctx, fields: &Fields, listed: &ProjectAgents, bar: Bar<'_>) -> Element {
  let Some(entry) = bar.entry else {
    let mut draft = DraftBar::new("agent-draft-bar", Standing::Draft, ctx.t("agents.newDraft")).sub(ctx.t("agents.newDraftSub"));
    for action in bar.actions {
      draft = draft.action(action);
    }
    return draft.into();
  };
  let state = entry.state;
  let version = state.published().map_or(0, |v| v.get());
  let saved = bar.saved_at.map_or_else(
    || ctx.t("agents.notPublished"),
    |at| ctx.t_args("agents.savedAt", [("time", orchester_workspace::display::clock(at))]),
  );
  let (standing, status, sub) = match state {
    VersionState::Published(_) => (
      Standing::Published,
      ctx.t_args("agents.published", [("version", version.to_string())]),
      ctx.t("agents.versionNote"),
    ),
    VersionState::StaleDraft { based_on, published } => (
      Standing::Attention,
      ctx.t_args(
        "agents.stale",
        [
          ("base", based_on.map_or(0, |v| v.get()).to_string()),
          ("version", published.map_or(0, |v| v.get()).to_string()),
        ],
      ),
      ctx.t_args(
        "agents.staleNote",
        [("version", version.to_string()), ("next", (version + 1).to_string())],
      ),
    ),
    VersionState::Draft { published: None } => (
      Standing::Draft,
      ctx.t_args("agents.barFirstDraft", [("saved", saved.to_string())]),
      ctx.t("agents.firstNote"),
    ),
    VersionState::Draft { .. } => (
      Standing::Draft,
      ctx.t_args(
        "agents.barDraft",
        [
          ("next", (version + 1).to_string()),
          ("saved", saved.to_string()),
          ("version", version.to_string()),
        ],
      ),
      changes(ctx, fields, listed, bar.published),
    ),
  };
  let sub = if bar.publishable || !state.has_draft() {
    sub
  } else {
    ctx.t("agents.notEnforced")
  };
  let mut draft = DraftBar::new("agent-draft-bar", standing, status).sub(sub);
  for action in bar.actions {
    draft = draft.action(action);
  }
  draft.into()
}

/// What the draft changes from the published version: `Model a → b · Permissions · Changed`.
fn changes(ctx: &Ctx, fields: &Fields, listed: &ProjectAgents, base: Option<&AgentProfile>) -> Arc<str> {
  let Some(base) = base else {
    return ctx.t("agents.versionNote");
  };
  let on_provider = fields.connection(listed).is_some_and(|c| c.api_provider().is_some());
  let mut parts: Vec<String> = Vec::new();
  for (key, from, to) in [
    ("agents.model", base.model().as_str().to_owned(), fields.model_value()),
    (
      "agents.effort",
      orchester_workspace::agents::form::stored_effort(base).unwrap_or_default(),
      fields.effort.get(),
    ),
  ] {
    if from != to {
      let empty = ctx.t(if on_provider { "agents.providerDefaultChange" } else { "agents.defaultEffort" });
      let from = if from.is_empty() { empty.to_string() } else { from };
      let to = if to.is_empty() { empty.to_string() } else { to };
      parts.push(ctx.t_args("agents.changedField", [("field", ctx.t(key).to_string()), ("from", from), ("to", to)]).to_string());
    }
  }
  for (different, key) in [
    (fields.role.get() != base.role().as_str(), "agents.role"),
    (fields.responsibility.get() != base.responsibilities(), "agents.responsibility"),
    (fields.instructions.get() != base.instructions().as_str(), "agents.instructions"),
    (fields.connection.get() != base.connection().to_string(), "agents.connection"),
    (fields.permissions.get().0 != *base.permissions(), "agents.permissionsTab"),
    (fields.servers.get().0 != *base.mcp_servers(), "agents.mcp"),
  ] {
    if different {
      parts.push(ctx.t(key).to_string());
    }
  }
  if parts.is_empty() {
    return ctx.t("agents.versionNote");
  }
  parts.push(ctx.t("agents.changed").to_string());
  parts.join(&format!(" {} ", ctx.t("agents.summarySeparator"))).into()
}
