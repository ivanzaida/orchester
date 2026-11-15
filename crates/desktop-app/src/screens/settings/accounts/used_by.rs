//! An account's *Used by* (`Used by` `CQkPP` in `03b · Account`; an API provider's on `Agents on accounts` B1
//! `gZTuX`): every agent profile of the listed projects that runs on the account on this computer, a row each with
//! its project, and *Open* to the profile in its project's Agents hub. How many there are in how many projects is
//! in the group's heading; projects that could not be read are named under it. No connection is named: Orchester
//! keeps one for each account a project's profiles run on.

use std::collections::BTreeSet;

use lurq::{
  app::ctx::Ctx,
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::accounts::AccountUses;

use super::words::{t, ta};
use crate::{
  navigation::Navigation,
  routes::{AgentTab, ProjectPage, Route},
  theme::ExtraSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    setting_rows::{SettingGroup, SettingRow, group_note},
  },
};

/// The group `id` of what `uses` holds; nothing until it is read.
pub(super) fn group(ctx: &mut Ctx, uses: Option<&AccountUses>, id: &str) -> Element {
  let Some(uses) = uses else {
    return Column::new().into();
  };
  let projects = uses
    .profiles
    .iter()
    .map(|used| used.project)
    .collect::<BTreeSet<_>>()
    .len();
  let meta = match projects {
    0 => None,
    1 => Some(ta(
      ctx,
      "accounts.usedBy.metaOne",
      [("count", uses.profiles.len().to_string())],
    )),
    projects => Some(ta(
      ctx,
      "accounts.usedBy.meta",
      [
        ("count", uses.profiles.len().to_string()),
        ("projects", projects.to_string()),
      ],
    )),
  };
  let navigation = Navigation::of(ctx);
  let rows: Vec<Element> = uses
    .profiles
    .iter()
    .map(|used| {
      let (project, profile) = (used.project, used.profile);
      let open = ActionButton::new(
        format!("{id}-{profile}-open"),
        ButtonKind::Ghost,
        t(ctx, "accounts.usedBy.open"),
      )
      .icon(Icon::ArrowUpRight)
      .small()
      .opens(navigation.to(Route::Project(project, ProjectPage::Agent(profile, AgentTab::Profile))));
      let role = used
        .role
        .as_ref()
        .map_or_else(|| t(ctx, "accounts.usedBy.teamProfile"), |role| role.to_string());
      SettingRow::new(format!("{id}-{profile}"), role)
        .hint(ta(
          ctx,
          "accounts.usedBy.profileHint",
          [("project", used.project_name.to_string())],
        ))
        .control(open)
        .render(ctx)
    })
    .collect();
  let mut group = SettingGroup::new(id.to_owned(), t(ctx, "accounts.usedBy.group")).rows(rows);
  if let Some(meta) = meta {
    group = group.meta(meta);
  }
  if uses.profiles.is_empty() {
    group = group.note(t(ctx, "accounts.usedBy.empty"));
  }
  let unreadable = (!uses.unreadable.is_empty()).then(|| {
    let names: Vec<String> = uses.unreadable.iter().map(ToString::to_string).collect();
    group_note(
      &format!("{id}-unreadable"),
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
