//! Profile list Yr8vx and empty states CUsAe.
use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, VirtualizedList},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::agents::data::ProjectAgents;

use super::{
  View,
  entries::{AgentRow, RowProps},
  words,
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ConnectionsView, ProjectPage, Route},
  theme::{ExtraSpacing, ExtraTypography, IconSize, agents::page_spacing},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    state::EmptyState,
  },
};

pub(super) fn list(ctx: &mut Ctx, project: &ShownProject, listed: &ProjectAgents, view: Signal<View>) -> Element {
  let page = Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .padding(Padding::symmetric(page_spacing(), SpacingSize::Section))
    .spacing(ExtraSpacing::Tabs.role())
    .child(header(ctx, listed, view.clone()));
  if listed.profiles.is_empty() {
    return page.child(empty(ctx, project, listed, view)).into();
  }
  let data = listed.clone();
  let rows = VirtualizedList::new_shared(ctx, listed.profiles.clone())
    .list_key("agents-list")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .max_height(Dimension::Px(super::entries::ROW_HEIGHT * listed.profiles.len() as f32))
    .mount_keyed::<AgentRow, _, _, _>(
      |entry| entry.id.to_string(),
      move |entry| RowProps {
        entry: entry.clone(),
        listed: data.clone(),
        view: view.clone(),
      },
    );
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
  page
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .max_height(Dimension::Px(
          super::entries::ROW_HEIGHT * listed.profiles.len() as f32 + super::entries::TABLE_CHROME,
        ))
        .rounded(RadiusSize::Lg)
        .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .child(super::entries::header(ctx))
        .child(rows)
        .child(
          Row::new()
            .width(Dimension::Pct(100.0))
            .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
            .background(PaletteColor::SurfacePanel)
            .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
            .child(Text::new(&counts).variant(TypographyStyle::Caption)),
        ),
    )
    .child(
      Row::new()
        .spacing(ExtraSpacing::Snug.role())
        .child(icon(Icon::History, IconSize::Sm, PaletteColor::TextMuted))
        .child(Text::new(&ctx.t("agents.versionNote")).variant(TypographyStyle::Caption)),
    )
    .into()
}

fn new_button(ctx: &Ctx, id: &str, listed: &ProjectAgents, view: Signal<View>) -> ActionButton {
  let button = ActionButton::new(id, ButtonKind::Primary, ctx.t("agents.new")).icon(Icon::Plus);
  if listed.connections.is_empty() {
    button.unavailable(ctx.t("agents.needsConnection"))
  } else {
    button.on_click(move || view.set(View::New))
  }
}
fn import(ctx: &Ctx, id: &str) -> ActionButton {
  ActionButton::new(id, ButtonKind::Secondary, ctx.t("agents.import"))
    .icon(Icon::Download)
    .unavailable(ctx.t("agents.importUnavailable"))
}
fn header(ctx: &Ctx, listed: &ProjectAgents, view: Signal<View>) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::End)
    .child(
      Column::new()
        .flex(1.0)
        .spacing(ExtraSpacing::Snug.role())
        .child(Text::new(&ctx.t("agents.title")).variant(ExtraTypography::Display.role()))
        .child(
          Text::new(&ctx.t("agents.description"))
            .variant(TypographyStyle::Body)
            .color(PaletteColor::TextSecondary),
        ),
    )
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .child(import(ctx, "agents-import"))
        .child(new_button(ctx, "agents-new", listed, view)),
    )
}
fn empty(ctx: &mut Ctx, project: &ShownProject, listed: &ProjectAgents, view: Signal<View>) -> Element {
  let has_connection = !listed.connections.is_empty();
  let mut empty = EmptyState::new(
    "agents-empty",
    if has_connection { Icon::Bot } else { Icon::Unplug },
    ctx.t("agents.emptyTitle"),
    ctx.t(if has_connection {
      "agents.emptyDetail"
    } else {
      "agents.noConnections"
    }),
  );
  if has_connection {
    empty = empty
      .action(new_button(ctx, "agents-empty-new", listed, view))
      .action(import(ctx, "agents-empty-import"));
  } else {
    let nav = Navigation::of(ctx);
    let id = project.open.project.id();
    empty = empty.action(
      ActionButton::new(
        "agents-open-connections",
        ButtonKind::Primary,
        ctx.t("agents.openConnections"),
      )
      .icon(Icon::Plug)
      .on_click(move || nav.go(Route::Project(id, ProjectPage::Connections(ConnectionsView::default())))),
    );
  }
  empty.child(empty_connections(ctx, project, listed)).into()
}
fn empty_connections(ctx: &Ctx, project: &ShownProject, listed: &ProjectAgents) -> Column {
  let mut connections = Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().top(SpacingSize::Md))
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .spacing(ExtraSpacing::Snug.role())
    .child(
      Text::new(&ctx.t_args(
        "agents.connections",
        [("project", project.open.project.name().as_str())],
      ))
      .variant(TypographyStyle::Caption),
    );
  for connection in listed.connections.iter() {
    let shown = crate::screens::connections::words::agent_status(connection.status());
    let detail = ctx.t_args(
      "agents.connectionState",
      [
        (
          "provider",
          ctx.t(crate::screens::connections::words::provider_key(connection.provider())),
        ),
        ("status", ctx.t(shown.key)),
      ],
    );
    connections = connections.child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(icon(shown.icon, IconSize::Base, shown.tone.color()))
        .child(Text::new(connection.name().as_str()).variant(TypographyStyle::Label))
        .child(words::note(&detail)),
    );
  }
  if listed.connections.is_empty() {
    connections = connections.child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .child(icon(Icon::Circle, IconSize::Base, PaletteColor::TextMuted))
        .child(Text::new(&ctx.t("agents.none")).variant(TypographyStyle::Label))
        .child(words::note(&ctx.t("agents.needsConnection"))),
    );
  }
  connections
}
