//! An account's *Used by* (`Used by` `CQkPP` in `03b · Account`): every connection of the listed projects that runs on
//! the account, a row each with its project and the roles of the agent profiles on it, and *Open* to the connection in
//! its project. How many there are in how many projects is in the group's heading; projects that could not be read
//! are named under it.

use lurq::{
  app::ctx::Ctx,
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::accounts::{AccountUses, ConnectionUse};

use super::words::{t, ta};
use crate::{
  navigation::Navigation,
  routes::{ConnectionsView, ProjectPage, Route},
  theme::ExtraSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    setting_rows::{SettingGroup, SettingRow, group_note},
  },
};

/// The connection's project and the roles of the profiles on it.
fn hint(ctx: &Ctx, used: &ConnectionUse) -> String {
  if used.profiles.is_empty() {
    return ta(
      ctx,
      "accounts.usedBy.hintNoProfile",
      [("project", used.project_name.to_string())],
    );
  }
  let profiles: Vec<&str> = used.profiles.iter().map(|role| &**role).collect();
  ta(
    ctx,
    "accounts.usedBy.hint",
    [
      ("project", used.project_name.to_string()),
      ("profiles", profiles.join(", ")),
    ],
  )
}

pub(super) fn group(ctx: &mut Ctx, uses: Option<&AccountUses>) -> Element {
  let Some(uses) = uses else {
    return Column::new().into();
  };
  let meta = match uses.project_count() {
    0 => None,
    1 => Some(ta(
      ctx,
      "accounts.usedBy.metaOne",
      [("count", uses.connections.len().to_string())],
    )),
    projects => Some(ta(
      ctx,
      "accounts.usedBy.meta",
      [
        ("count", uses.connections.len().to_string()),
        ("projects", projects.to_string()),
      ],
    )),
  };
  let navigation = Navigation::of(ctx);
  let rows: Vec<Element> = uses
    .connections
    .iter()
    .map(|used| {
      let (project, connection) = (used.project, used.connection);
      let view = ConnectionsView::of(connection);
      let open = ActionButton::new(
        format!("account-used-by-{connection}-open"),
        ButtonKind::Ghost,
        t(ctx, "accounts.usedBy.open"),
      )
      .icon(Icon::ArrowUpRight)
      .small()
      .opens(navigation.to(Route::Project(project, ProjectPage::Connections(view))));
      SettingRow::new(
        format!("account-used-by-{connection}"),
        used.connection_name.to_string(),
      )
      .hint(hint(ctx, used))
      .control(open)
      .render(ctx)
    })
    .collect();
  let mut group = SettingGroup::new("account-used-by", t(ctx, "accounts.usedBy.group")).rows(rows);
  if let Some(meta) = meta {
    group = group.meta(meta);
  }
  if uses.connections.is_empty() {
    group = group.note(t(ctx, "accounts.usedBy.empty"));
  }
  let unreadable = (!uses.unreadable.is_empty()).then(|| {
    let names: Vec<String> = uses.unreadable.iter().map(ToString::to_string).collect();
    group_note(
      "account-used-by-unreadable",
      &ta(ctx, "accounts.usedBy.unreadable", [("projects", names.join(", "))]),
    )
  });
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(group)
    .with_children(unreadable)
    .into()
}
