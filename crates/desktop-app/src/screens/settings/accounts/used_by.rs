//! *Used by* (`Connections` in `HzJSR`): every connection of the listed projects that runs on the account, with the
//! roles of the agent profiles on it, as links (`Used-by link` `Q91kML`) to the connection in its project.

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Text},
  node::{Element, dimension::Dimension},
};
use orchester_workspace::accounts::AccountUses;

use super::words::{t, ta};
use crate::{
  navigation::Navigation,
  routes::{ConnectionsTab, ConnectionsView, ProjectPage, Route},
  theme::ExtraTypography,
  ui::{
    icon::Icon,
    inspector::{section_title, used_by_link},
  },
};

/// A connection with its project and the roles of the profiles on it.
fn label(ctx: &Ctx, used: &orchester_workspace::accounts::ConnectionUse) -> String {
  if used.profiles.is_empty() {
    return ta(
      ctx,
      "accounts.usedBy.noProfile",
      [
        ("project", used.project_name.to_string()),
        ("connection", used.connection_name.to_string()),
      ],
    );
  }
  let profiles: Vec<&str> = used.profiles.iter().map(|role| &**role).collect();
  ta(
    ctx,
    "accounts.usedBy.profiles",
    [
      ("project", used.project_name.to_string()),
      ("connection", used.connection_name.to_string()),
      ("profiles", profiles.join(", ")),
    ],
  )
}

pub(super) fn list(ctx: &mut Ctx, uses: Option<&AccountUses>) -> Element {
  let Some(uses) = uses else {
    return Column::new().into();
  };
  let projects = uses.project_count();
  let meta = match projects {
    0 => None,
    1 => Some(ta(
      ctx,
      "accounts.usedBy.metaOne",
      [("count", uses.connections.len().to_string())],
    )),
    _ => Some(ta(
      ctx,
      "accounts.usedBy.meta",
      [
        ("count", uses.connections.len().to_string()),
        ("projects", projects.to_string()),
      ],
    )),
  };
  let navigation = Navigation::of(ctx);
  let mut list = Column::new()
    .id("account-used-by")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(section_title(
      "account-used-by-title",
      &t(ctx, "accounts.usedBy.title"),
      meta.as_deref(),
    ));
  if uses.connections.is_empty() {
    list = list.child(
      Text::new(&t(ctx, "accounts.usedBy.empty"))
        .id("account-used-by-empty")
        .variant(ExtraTypography::Note.role())
        .width(Dimension::Pct(100.0)),
    );
  }
  for used in &uses.connections {
    let label = label(ctx, used);
    let navigation = navigation.clone();
    let (project, connection) = (used.project, used.connection);
    let open = std::sync::Arc::new(move || {
      let view = ConnectionsView::new(ConnectionsTab::AgentProviders).with_selected(connection);
      navigation.go(Route::Project(project, ProjectPage::Connections(view)));
    });
    list = list.child(used_by_link(
      &format!("account-used-by-{connection}"),
      Icon::Bot,
      &label,
      Some(open),
    ));
  }
  if !uses.unreadable.is_empty() {
    let names: Vec<String> = uses.unreadable.iter().map(ToString::to_string).collect();
    list = list.child(
      Text::new(&ta(ctx, "accounts.usedBy.unreadable", [("projects", names.join(", "))]))
        .variant(ExtraTypography::Note.role())
        .width(Dimension::Pct(100.0)),
    );
  }
  list.into()
}
