//! The panel of a team instruction and of a team skill (`Agents · Instructions` `c3jU1R` A4, `Agents · Skills` `c9YnL`
//! A5; `docs/design/team-configuration.md`, "Per resource kind"): its head and chips, *From the team* (where it is
//! added, which agents get it, who uses it and its text or files), *On this computer* (nothing: they are the same on
//! every computer) and, pinned at the bottom, the admin's *Edit*. A person who may not change the team's resources
//! reads it, with no bar and nothing disabled (`Member view`, notes rule 15, [`super::member`]).

mod instruction;
mod skill;

use std::sync::Arc;

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
  sharing::team::TeamConfiguration,
};

use super::{
  bar,
  fill::nothing_here,
  member,
  model::{Entry, Listing},
  panel, rows, words,
};
use crate::{
  navigation::Navigation,
  routes::{InstructionsTab, InstructionsView, ProjectPage, Route},
  services::Services,
  theme::HubText,
};

/// What the team holds of the resource shown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Sheet {
  Instruction(TeamInstructionSheet),
  Skill(TeamSkillSheet),
}

#[derive(Clone, PartialEq)]
pub(super) struct Props {
  pub project: ProjectId,
  /// The team's configuration: an admin may change its resources, and a member reads who the admins are.
  pub team: TeamConfiguration,
  pub listing: Listing,
  pub entry: Entry,
  pub sheet: Sheet,
}
impl DevtoolsInspectable for Props {}

pub(super) fn panel(ctx: &mut Ctx, props: Props) -> Element {
  ctx.mount_keyed::<PlainPanel>(&format!("team-hub-plain-{}", props.entry.id), props)
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
      project,
      team,
      listing,
      entry,
      sheet,
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
    let route = team.may_change.then(|| editor(&entry, project, &sheet));
    let navigation = Navigation::of(ctx);
    let head_edit = route.clone().map(|route| panel::edit_button(ctx, navigation.to(route)));
    let version = words::published_line(ctx, now, &entry, published);
    let body = panel::body()
      .child(panel::head_of(&entry, (kind, version), head_edit))
      .child(chips(ctx, &listing, &entry, &sheet))
      .with_children(member::lock_line(ctx, &team))
      .with_children(shown)
      .child(on_this_computer(ctx, &sheet));
    let bar = route.map(|route| bar::edit(ctx, &entry, navigation.to(route)));
    panel::frame(&entry, body, bar)
  }
}

/// The editor the resource has today: the skill's page, and the instruction's page of its scope.
fn editor(entry: &Entry, project: ProjectId, sheet: &Sheet) -> Route {
  let page = match sheet {
    Sheet::Skill(_) => ProjectPage::Skill(entry.id),
    Sheet::Instruction(sheet) => {
      let tab = match sheet.added {
        Added::Project => InstructionsTab::Project,
        Added::Role(_) => InstructionsTab::Roles,
        Added::Pipeline(_) => InstructionsTab::Pipelines,
        Added::Step { .. } => InstructionsTab::Steps,
      };
      ProjectPage::Instructions(InstructionsView::rule(tab, Some(entry.id)))
    }
  };
  Route::Project(project, page)
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
fn used_by_row(ctx: &Ctx, names: &[String]) -> Row {
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
    None,
    value,
  )
}

/// Where a resource is added, in words.
fn added_words(ctx: &Ctx, added: &Added) -> Arc<str> {
  match added {
    Added::Project => ctx.t("team_hub.plain.added.project"),
    Added::Role(role) => ctx.t_args("team_hub.plain.added.role", [("role", role.as_str())]),
    Added::Pipeline(pipeline) => ctx.t_args("team_hub.plain.added.pipeline", [("pipeline", pipeline.name.as_str())]),
    Added::Step { pipeline, step } => ctx.t_args(
      "team_hub.plain.added.step",
      [("step", step.as_str()), ("pipeline", pipeline.name.as_str())],
    ),
  }
}

/// The *Added to* row.
fn added_row(ctx: &Ctx, added: &Added) -> Row {
  rows::row(
    "team-hub-from-team-added",
    &ctx.t("team_hub.plain.addedTo"),
    None,
    rows::words("team-hub-from-team-added-value", &added_words(ctx, added)),
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
