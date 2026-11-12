//! An admin's edit of a team profile in its panel, in place (`A26 · Agents · Profile · admin · editing` `WSw0W`): the
//! profile's fields as a new one's form has them ([`ProfileFields`]), over the latest version: *Role* first, then
//! *From the team* with the Agent CLI, model and effort as selects, the instructions, MCP servers, tools and skills it
//! gives its agents in tags with *Add* (a menu of the team's live resources of that kind), and the secrets it needs
//! (read-only: they follow from what it uses). *On this computer* and *Ready here?* stay the panel's own. Save
//! publishes the body the fields make as the next version at once ([`super::super::edit`]).

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::ctx::Ctx,
  components::{Column, Text},
  node::Element,
};
use orchester_core::resource::TeamAgentProfile;
use orchester_workspace::{agents::team_profile::TeamProfileSheet, messages::Message, team_config::TeamConfig};

use crate::{
  queries::{self, Session, hub},
  screens::{
    hub::profile_form::{Form, Ids, Made, Offer, ProfileFields, Scope},
    team_hub::{
      edit::{Edit, Parts, Target, View},
      rows::{self, Sub},
    },
  },
  theme::HubText,
};

/// The ids of the edit's parts, which tests and automation address: `team-hub-role-name-input`,
/// `team-hub-from-team-cli`, `team-hub-from-team-mcp-value`, …
const IDS: Ids = Ids {
  role: "team-hub-role",
  rows: "team-hub-from-team",
};

/// What the panel keeps of an admin's edit of a team profile. Cloning shares it.
#[derive(Clone)]
pub(super) struct ProfileEdit {
  fields: ProfileFields,
  edit: Edit<TeamAgentProfile>,
  root: PathBuf,
}

/// What the edit puts in the panel: its parts ([`Parts`]) and the sections that take the place of *From the team*.
pub(super) struct Drawn {
  pub parts: Parts,
  pub sections: Vec<Element>,
}

impl ProfileEdit {
  pub(super) fn new(ctx: &mut Ctx, target: Target, sheet: &TeamProfileSheet) -> Self {
    let fields = ProfileFields::new(ctx);
    let load = {
      let fields = fields.clone();
      Arc::new(move |body: &TeamAgentProfile| fields.load_team(body))
    };
    let root = target.0.clone();
    let edit = Edit::new(ctx, target, sheet.edit.as_ref(), load);
    Self { fields, edit, root }
  }

  /// The edit's parts and sections, over the team's configuration `config` (what *Add* offers); `None` while it has no
  /// version to be made on.
  pub(super) fn draw(
    &self,
    ctx: &mut Ctx,
    (sheet, config): (&TeamProfileSheet, Option<&TeamConfig>),
    unreached: Option<&Message>,
  ) -> Option<Drawn> {
    let base = self.edit.base()?;
    let session = Session::of(ctx);
    let read = queries::state(&ctx.query(hub::profile_form(session, self.root.clone()))).data;
    let offer = Offer::read(read.as_ref(), Some(config));
    let drawn = self.fields.draw(
      ctx,
      &Form {
        scope: Scope::Team,
        ids: IDS,
        offer: &offer,
        base: Some(&base.body),
        tried: true,
        locked: false,
        name_said: None,
        first: None,
        held: None,
      },
    );
    // Fields of a team's scope make a team profile; the other arm never runs.
    let made = drawn.made.and_then(|made| match made {
      Made::Team(profile) => Ok(*profile),
      Made::Local(_) => Err(ctx.t("team_hub.edit.fixFields")),
    });
    self.edit.keep_up(
      sheet.edit.as_ref(),
      made.as_ref().map_or(true, |body| *body != base.body),
    );
    let parts = self.edit.parts(
      ctx,
      View {
        made,
        unreached,
        published_by: None,
      },
    )?;
    let mut team_rows = drawn.rows;
    if !sheet.secrets.is_empty() {
      team_rows.push(rows::row(
        "team-hub-from-team-secrets",
        &ctx.t("team_hub.profile.secrets"),
        Some(Sub::quiet(ctx.t("team_hub.profile.secretsSub"))),
        rows::tags(
          "team-hub-from-team-secrets-value",
          sheet.secrets.iter().map(ToString::to_string),
        ),
      ));
    }
    let sections = vec![drawn.role, team_section(ctx, team_rows).into()];
    Some(Drawn { parts, sections })
  }
}

/// *From the team*: what the team holds of it, the same on every computer.
fn team_section(ctx: &Ctx, rows: Vec<lurq::components::Row>) -> Column {
  rows::section(
    "team-hub-from-team",
    &ctx.t("team_hub.profile.fromTeam"),
    vec![
      Text::new(&ctx.t("team_hub.profile.fromTeamNote"))
        .id("team-hub-from-team-note")
        .variant(HubText::PanelHint.role())
        .nowrap()
        .into(),
    ],
    rows,
  )
}
