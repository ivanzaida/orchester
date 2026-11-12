//! The panel of a team tool (`Agents · Tools` `u9Hcc` A6, `yTVeE` M6; `docs/design/team-configuration.md`, "What a
//! computer fills in"): its head and chips, *From the team* (what it runs or calls, its arguments, who may call it
//! without asking and the secret it needs by name, the same on every computer), *On this computer* (the value of that
//! secret here, and how the tool stands) and, pinned at the bottom, *Add value* while the secret has none here, or the
//! admin's *Edit* of a command.
//!
//! A secret's value is a person's alone to set, as an MCP server's is: a press automation made does nothing and says
//! so. A latest version published before the team's form names its secret by the id it had on the publisher's
//! computer, which is never shown: nothing can be filled in for it here, and an admin publishes it again by name
//! ([`republish`]).

mod computer;
mod edit;
pub(super) mod from_team;
mod republish;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  node::Element,
};
use orchester_core::{id::ProjectId, secret::SecretName, sharing::Missing};
use orchester_workspace::{
  agents::team_tool::{TeamToolSheet, ToolSecret},
  messages::Message,
  readiness::{ReadinessStanding, Undecided},
  sharing::team::TeamConfiguration,
  tools::form::CommandText,
};

use self::republish::Republish;
use super::{
  actions::Actions,
  archive, bar, member,
  model::{Entry, Listing},
  panel, secret, words,
};
use crate::{
  screens::settings::orchester::PersonOnly,
  services::Services,
  ui::{
    button::{ActionButton, ButtonKind},
    chip::BadgeTone,
    field::field_error,
    icon::Icon,
    message::message_text,
    pressable::Press,
  },
};

#[derive(Clone, PartialEq)]
pub(super) struct Props {
  pub repo_root: PathBuf,
  pub project: ProjectId,
  /// The person is an admin of the team, reachable or not: they edit a command tool in place, and what they typed
  /// stays while the server is lost.
  pub admin: bool,
  /// Why the team server cannot be reached now; Save waits for it.
  pub unreached: Option<Message>,
  /// The team's configuration: an admin may change its resources, and a member reads who the admins are.
  pub team: TeamConfiguration,
  pub listing: Listing,
  pub entry: Entry,
  pub sheet: TeamToolSheet,
}
impl DevtoolsInspectable for Props {}

pub(super) fn panel(ctx: &mut Ctx, props: Props) -> Element {
  let key = format!("team-hub-tool-{}", props.entry.id);
  let command = props
    .sheet
    .edit
    .as_ref()
    .is_some_and(|latest| CommandText::of(&latest.body).is_some());
  if props.admin && command {
    return ctx.mount_keyed::<edit::ToolEditor>(&key, edit::EditorProps { props });
  }
  ctx.mount_keyed::<ToolPanel>(&key, props)
}

/// The id the person-only check of *Add value* is made under.
const ADD_VALUE: &str = "team-hub-fix";

/// The panel of one tool, keyed by the resource so its scroll, an open dialog or a typed value never carries over to
/// another.
struct ToolPanel {
  actions: Actions,
  secret: secret::Entry,
  person: PersonOnly,
  republish: Republish,
}

impl Component for ToolPanel {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let panel = Self {
      actions: Actions::new(ctx, &props.repo_root),
      secret: secret::Entry::new(ctx),
      person: PersonOnly::new(ctx),
      republish: Republish::new(ctx, &props.repo_root, props.entry.id),
    };
    panel.secret.follow_save(ctx, &panel.actions.add_value);
    panel
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let Props {
      repo_root,
      team,
      listing,
      entry,
      sheet,
      ..
    } = &props;
    let missing = entry.standing.as_ref().map_or(&[][..], |standing| standing.missing());
    let now = Services::of(ctx).clock.now();
    let head_end = archive::menu(ctx, repo_root, entry, team.may_change);
    let kind = ctx.t_args(
      "team_hub.tool.where",
      [("kind", from_team::kind(ctx, sheet).to_string())],
    );
    let version = words::published_line(ctx, now, entry, sheet.published.as_ref());
    let chips = panel::chips_saying(
      ctx,
      entry,
      chip(ctx, listing, entry),
      panel::used_by(ctx, &sheet.used_by),
    );
    let body = panel::body()
      .child(panel::head_of(entry, (kind.to_string(), version), head_end))
      .child(chips)
      .with_children(member::lock_line(ctx, team))
      .child(from_team::section(ctx, sheet, super::rows::FromTeam::Shared))
      .child(computer::section(ctx, entry, sheet, missing))
      .with_children(self.failure(ctx));
    let dialog = self.secret.dialog(
      ctx,
      secret::Entry::save_by(&self.actions.add_value, &self.person),
      self.actions.add_value.state().get().is_pending(),
      self.person.reason(ctx, secret::SAVE),
    );
    panel::frame(entry, body, self.bar(ctx, &props)).with_children(dialog)
  }
}

impl Drop for ToolPanel {
  fn drop(&mut self) {
    self.secret.wipe();
  }
}

impl ToolPanel {
  /// The pinned bar: *Add value* while the secret the tool names has none here, or an admin's *Publish again by name*
  /// of a version that names it by id.
  fn bar(&self, ctx: &Ctx, props: &Props) -> Option<Element> {
    let missing = props
      .entry
      .standing
      .as_ref()
      .map_or(&[][..], |standing| standing.missing());
    match &props.sheet.secret {
      ToolSecret::Named(name) if missing.contains(&Missing::Secret { name: name.clone() }) => {
        Some(self.add_value(ctx, name))
      }
      ToolSecret::ById if props.admin => {
        let latest = props.sheet.edit.as_ref()?;
        Some(self.republish.bar(ctx, latest, props.unreached.as_ref()))
      }
      ToolSecret::Named(_) | ToolSecret::ById | ToolSecret::None => None,
    }
  }

  fn add_value(&self, ctx: &Ctx, name: &SecretName) -> Element {
    let (entry, name, person) = (self.secret.clone(), name.clone(), self.person.clone());
    let press = Press::run(move || {
      if person.allows(ADD_VALUE) {
        entry.ask(&name);
      }
    });
    let button = ActionButton::new(
      "team-hub-fix-add-value",
      ButtonKind::Primary,
      ctx.t("team_hub.mcp.bar.addValue"),
    )
    .icon(Icon::KeyRound)
    .pinned()
    .press(press);
    bar::bar(
      vec![button.into()],
      Some(ctx.t("team_hub.mcp.bar.addValueHint").to_string()),
    )
  }

  /// Why the last press did nothing: automation pressed *Add value*, or the team refused the version published again.
  fn failure(&self, ctx: &Ctx) -> Option<Element> {
    if let Some(reason) = self.republish.refused() {
      return Some(field_error("team-hub-fix-error", &message_text(ctx, &reason)).into());
    }
    self.person.reason(ctx, ADD_VALUE)
  }
}

/// The panel's readiness chip: the line of the list, but for a secret named by id, which the panel says in its own
/// words.
fn chip(ctx: &Ctx, listing: &Listing, entry: &Entry) -> (Arc<str>, BadgeTone) {
  match &entry.standing {
    Some(ReadinessStanding::Undecided(Undecided::TeamToolSecretById)) => {
      (ctx.t("team_hub.tool.chip.secretById"), BadgeTone::Warning)
    }
    _ => words::chip(ctx, listing, entry),
  }
}
