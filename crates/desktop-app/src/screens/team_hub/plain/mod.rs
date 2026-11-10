//! The panel of a team instruction and of a team skill (`Agents · Instructions` `c3jU1R` A4, `Agents · Skills` `c9YnL`
//! A5; `docs/design/team-configuration.md`, "Per resource kind"): its head and chips, *From the team* (where it is
//! added, which agents get it, who uses it and its text or files), *On this computer* (nothing: they are the same on
//! every computer) and, pinned at the bottom, the admin's *Edit*. A person who may not change the team's resources
//! reads it, with no bar and nothing disabled (`Member view`, notes rule 15, [`super::member`]).

mod instruction;
mod instruction_edit;
mod skill;
mod skill_edit;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::PaletteColor,
  },
  components::{Column, Row, Text},
  node::Element,
};
use orchester_core::id::ProjectId;
use orchester_workspace::{
  agents::team_text::{Added, TeamInstructionSheet, TeamSkillSheet},
  messages::Message,
  sharing::team::TeamConfiguration,
};

use super::{
  archive,
  fill::nothing_here,
  member,
  model::{Entry, Listing},
  panel,
  rows::{self, Sub},
  words,
};
use crate::{services::Services, theme::HubText};

/// What the team holds of the resource shown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Sheet {
  Instruction(TeamInstructionSheet),
  Skill(TeamSkillSheet),
}

#[derive(Clone, PartialEq)]
pub(super) struct Props {
  pub repo_root: PathBuf,
  pub project: ProjectId,
  /// The person is an admin of the team, reachable or not: they edit it in place, and what they typed stays while the
  /// server is lost.
  pub admin: bool,
  /// Why the team server cannot be reached now; Save waits for it.
  pub unreached: Option<Message>,
  /// The team's configuration: an admin may change its resources, and a member reads who the admins are.
  pub team: TeamConfiguration,
  pub listing: Listing,
  pub entry: Entry,
  pub sheet: Sheet,
}
impl DevtoolsInspectable for Props {}

pub(super) fn panel(ctx: &mut Ctx, props: Props) -> Element {
  let key = format!("team-hub-plain-{}", props.entry.id);
  match (&props.sheet, props.admin) {
    (Sheet::Instruction(sheet), true) if sheet.edit.is_some() => {
      let sheet = sheet.clone();
      ctx.mount_keyed::<instruction_edit::InstructionEditor>(&key, instruction_edit::EditorProps { props, sheet })
    }
    (Sheet::Skill(sheet), true) if sheet.edit.is_some() => {
      let sheet = sheet.clone();
      ctx.mount_keyed::<skill_edit::SkillEditor>(&key, skill_edit::EditorProps { props, sheet })
    }
    _ => ctx.mount_keyed::<PlainPanel>(&key, props),
  }
}

/// The panel of one instruction or skill, keyed by the resource so its scroll never carries over to another.
struct PlainPanel;

impl Component for PlainPanel {
  type Props = Props;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props {
      repo_root,
      team,
      listing,
      entry,
      sheet,
      ..
    } = ctx.props::<Props>().clone();
    let now = Services::of(ctx).clock.now();
    let (kind, published, shown) = match &sheet {
      Sheet::Instruction(sheet) => (
        ctx.t("team_hub.kind.where.instruction").to_string(),
        sheet.published.as_ref(),
        instruction::sections(ctx, sheet),
      ),
      Sheet::Skill(sheet) => (
        ctx.t("team_hub.kind.where.skill").to_string(),
        sheet.published.as_ref(),
        skill::sections(ctx, sheet),
      ),
    };
    let head_end = archive::menu(ctx, &repo_root, &entry, team.may_change);
    let version = words::published_line(ctx, now, &entry, published);
    let body = panel::body()
      .child(panel::head_of(&entry, (kind, version), head_end))
      .child(chips(ctx, &listing, &entry, &sheet))
      .with_children(member::lock_line(ctx, &team))
      .with_children(shown)
      .child(on_this_computer(ctx, &sheet));
    panel::frame(&entry, body, None)
  }
}

/// *Team · v6*, *Ready here* and what the panel says of where it is used at their end.
fn chips(ctx: &Ctx, listing: &Listing, entry: &Entry, sheet: &Sheet) -> Row {
  let end = match sheet {
    Sheet::Instruction(sheet) if matches!(sheet.added, Added::Project) => Some(ctx.t("team_hub.plain.everyRun")),
    Sheet::Instruction(TeamInstructionSheet { used_by, .. }) | Sheet::Skill(TeamSkillSheet { used_by, .. }) => {
      panel::used_by(ctx, used_by)
    }
  };
  panel::chips_saying(ctx, entry, words::chip(ctx, listing, entry), end)
}

/// *On this computer*: nothing is filled in on one.
fn on_this_computer(ctx: &Ctx, sheet: &Sheet) -> Column {
  let words = match sheet {
    Sheet::Instruction(_) => ctx.t("team_hub.plain.nothingInstruction"),
    Sheet::Skill(_) => ctx.t("team_hub.plain.nothingSkill"),
  };
  rows::on_this_computer(ctx, vec![nothing_here("team-hub-nothing", &words)], None)
}

/// *Used by*: the profiles' names in tags, or that none uses it.
fn used_by_row(ctx: &Ctx, names: &[String], sub: Option<Arc<str>>) -> Row {
  let value: Element = if names.is_empty() {
    rows::words("team-hub-from-team-used-by-value", &ctx.t("team_hub.panel.usedByNone"))
      .color(PaletteColor::TextMuted)
      .into()
  } else {
    rows::tags("team-hub-from-team-used-by-value", names.iter().cloned()).into()
  };
  rows::row(
    "team-hub-from-team-used-by",
    &ctx.t("team_hub.plain.usedBy"),
    sub.map(Sub::quiet),
    value,
  )
}

/// The *Added to* row.
fn added_row(ctx: &Ctx, added: &Added) -> Row {
  rows::row(
    "team-hub-from-team-added",
    &ctx.t("team_hub.plain.addedTo"),
    None,
    rows::words("team-hub-from-team-added-value", &words::added(ctx, added)),
  )
}

/// The *From the team* section of `rows`.
fn from_team(ctx: &Ctx, rows: Vec<Row>) -> Column {
  let note = Text::new(&ctx.t("team_hub.profile.fromTeamNote"))
    .id("team-hub-from-team-note")
    .variant(HubText::PanelHint.role())
    .nowrap();
  rows::section(
    "team-hub-from-team",
    &ctx.t("team_hub.profile.fromTeam"),
    vec![note.into()],
    rows,
  )
}
