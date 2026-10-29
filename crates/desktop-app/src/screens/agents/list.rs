//! The Profiles tab's list (`01 Profiles` `B94gsw`): the group's head with its counts, one row per profile and the
//! versions note; with no profile yet, what a profile is, a quiet row with New agent and the project's connections
//! (`06 Profiles · nothing yet` `KeGdE`), or, without a connection, what to add first (06b).
use lurq::{
  app::ctx::Ctx,
  components::{Column, Text},
  node::{Element, dimension::Dimension},
};
use orchester_workspace::agents::data::ProjectAgents;

use super::{AgentsNav, Shown, entries};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{AgentTab, ConnectionsView, ProjectPage, Route},
  screens::hub::{
    self, Head,
    parts::{Entry, Tone, boxed, group, head as group_head, note, status},
  },
  theme::{ExtraTypography, HubSpacing},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
  },
};

/// Import agents and New agent, at the hub head's right.
pub(super) fn head(ctx: &Ctx, listed: &ProjectAgents, nav: &AgentsNav) -> Head {
  Head::actions([
    import(ctx, "agents-import", nav).into(),
    new_button(ctx, "agents-new", ButtonKind::Primary, listed, nav).into(),
  ])
}

/// Import agents and New agent while the profiles are read (`KHDar`): New agent opens a new profile, whose page says
/// what it needs once the project's connections are read.
pub(super) fn reading_head(ctx: &Ctx, nav: &AgentsNav) -> Head {
  let new = ActionButton::new("agents-new", ButtonKind::Primary, ctx.t("agents.new"))
    .icon(Icon::Plus)
    .opens(nav.to(Shown::New(AgentTab::default())));
  Head::actions([import(ctx, "agents-import", nav).into(), new.into()])
}

pub(super) fn list(ctx: &mut Ctx, project: &ShownProject, listed: &ProjectAgents, nav: &AgentsNav) -> Element {
  if listed.profiles.is_empty() {
    return hub::list_body("agents-list", empty(ctx, project, listed, nav));
  }
  let published = listed
    .profiles
    .iter()
    .filter(|entry| entry.state.published().is_some())
    .count();
  let missing = listed
    .profiles
    .iter()
    .filter(|entry| !listed.connections.iter().any(|c| c.id() == entry.profile.connection()))
    .count();
  let counts = ctx.t_args(
    "agents.counts",
    [
      ("profiles", listed.profiles.len().to_string()),
      ("published", published.to_string()),
      ("missing", missing.to_string()),
    ],
  );
  let rows = listed
    .profiles
    .iter()
    .map(|entry| entries::row(ctx, entry, listed, nav));
  let pane = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(HubSpacing::Groups.role())
    .child(group(
      "agents-profiles",
      group_head("agents-profiles", &ctx.t("agents.listTitle"), Some(&counts), None),
      boxed("agents-rows", rows),
    ))
    .child(note("agents-version-note", Icon::History, &ctx.t("agents.versionNote")));
  hub::list_body("agents-list", pane)
}

fn new_button(ctx: &Ctx, id: &str, kind: ButtonKind, listed: &ProjectAgents, nav: &AgentsNav) -> ActionButton {
  let button = ActionButton::new(id, kind, ctx.t("agents.new")).icon(Icon::Plus);
  if listed.connections.is_empty() {
    button.unavailable(ctx.t("agents.needsConnection"))
  } else {
    button.opens(nav.to(Shown::New(AgentTab::default())))
  }
}

/// Import agents, which opens Import resources on its Agents tab.
fn import(ctx: &Ctx, id: &str, nav: &AgentsNav) -> ActionButton {
  ActionButton::new(id, ButtonKind::Ghost, ctx.t("agents.import"))
    .icon(Icon::Download)
    .opens(nav.import())
}

fn empty(ctx: &mut Ctx, project: &ShownProject, listed: &ProjectAgents, nav: &AgentsNav) -> Element {
  let navigation = Navigation::of(ctx);
  let id = project.open.project.id();
  let open_connections = || navigation.to(Route::Project(id, ProjectPage::Connections(ConnectionsView::default())));
  let name = project.open.project.name().as_str();
  let intro = Column::new()
    .id("agents-empty")
    .width(Dimension::Pct(100.0))
    .spacing(lurq::app::theme::SpacingSize::Sm)
    .child(Text::new(&ctx.t("agents.emptyTitle")).variant(ExtraTypography::FormSection.role()))
    .child(
      Text::new(&ctx.t(if listed.connections.is_empty() {
        "agents.noConnections"
      } else {
        "agents.emptyDetail"
      }))
      .variant(ExtraTypography::Prose.role())
      .width(Dimension::Pct(100.0)),
    );
  let first = if listed.connections.is_empty() {
    Entry::new("agents-no-connection", ctx.t("agents.noConnectionTitle"))
      .tile(Icon::Plug)
      .hint(ctx.t("agents.noConnections"))
      .end(
        ActionButton::new(
          "agents-open-connections",
          ButtonKind::Secondary,
          ctx.t("agents.openConnections"),
        )
        .icon(Icon::ArrowUpRight)
        .opens(open_connections()),
      )
  } else {
    Entry::new("agents-none", ctx.t_args("agents.noProfiles", [("project", name)]))
      .tile(Icon::Bot)
      .hint(ctx.t("agents.emptyNote"))
      .end(new_button(ctx, "agents-empty-new", ButtonKind::Secondary, listed, nav))
  };
  let mut pane = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(HubSpacing::Groups.role())
    .child(intro)
    .child(group(
      "agents-profiles",
      group_head("agents-profiles", &ctx.t("agents.listTitle"), None, None),
      boxed("agents-rows", [first.into()]),
    ));
  if !listed.connections.is_empty() {
    let link = ActionButton::new(
      "agents-connections-open",
      ButtonKind::Ghost,
      ctx.t("agents.openConnections"),
    )
    .icon(Icon::ArrowUpRight)
    .opens(open_connections());
    pane = pane.child(connections(ctx, (name, listed), link));
  }
  pane
    .child(note("agents-version-note", Icon::History, &ctx.t("agents.versionNote")))
    .into()
}

/// The project's connections, each with its status, under no profile yet (06).
fn connections(ctx: &Ctx, (name, listed): (&str, &ProjectAgents), link: ActionButton) -> Column {
  let rows = listed.connections.iter().map(|connection| {
    use crate::screens::connections::words::{Tone as Said, connection_status};
    let shown = connection_status(connection, connection.status());
    let tone = match shown.tone {
      Said::Success => Tone::Success,
      Said::Warning => Tone::Warning,
      Said::Danger => Tone::Danger,
      Said::Muted | Said::Busy => Tone::Muted,
    };
    let provider = ctx.t(crate::screens::connections::words::provider_key(connection.provider()));
    Entry::new(
      format!("agents-connection-{}", connection.id()),
      connection.name().as_str(),
    )
    .tile(entries::provider_glyph(Some(connection)))
    .hint(provider)
    .end(status(
      &format!("agents-connection-status-{}", connection.id()),
      tone,
      &ctx.t(shown.key),
    ))
    .into()
  });
  group(
    "agents-connections",
    group_head(
      "agents-connections",
      &ctx.t_args("agents.connections", [("project", name)]),
      None,
      Some(link.into()),
    ),
    boxed("agents-connection-rows", rows),
  )
}
