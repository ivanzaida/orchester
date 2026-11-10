//! An admin's edit of a team profile in its panel, in place (`A26 · Agents · Profile · admin · editing` `WSw0W`): the
//! *Role* section first (the role, what it answers for and the role instructions), then *From the team* with its
//! Agent CLI and model as the version has them, the instructions, MCP servers, tools and skills it gives its agents in
//! tags with *Add*, a menu of the team's live resources of that kind that checks the ones it gives, and the secrets it
//! needs (read-only: they follow from what it uses). *On this computer* and *Ready here?* stay the panel's own. Save
//! publishes the body the fields make as the next version at once ([`super::super::edit`]).
//!
//! The *Role* section is drawn here for a team profile; the local panel's Role component (`L17` `b7Tbxb`, being made
//! in `screens/hub/`) is the one both should share once it lands.

use std::sync::Arc;

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{PromptText, ResourceKind, TeamAgentProfile},
};
use orchester_workspace::{
  agents::{form::prompt_error, team_profile::TeamProfileSheet},
  messages::{self, Message},
  team_config::TeamConfig,
};

use crate::{
  screens::{
    hub::{agent_row::tag_chip, area::Area},
    local_hub::profile::add_menu,
    team_hub::{
      edit::{Edit, Parts, Target, View},
      profile::from_team::effort_words,
      rows::{self, Sub},
      words::cli_key,
    },
  },
  theme::HubText,
  ui::{field::FieldInput, message::message_text},
};

/// The responsibilities' area is at least 64 high, the role instructions' 180 (`Role` in `WSw0W`).
const RESPONSIBILITIES_AREA: f32 = 64.0;
const ROLE_INSTRUCTIONS_AREA: f32 = 180.0;

/// The team's resources the profile gives its agents, by kind, as the fields hold them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Given {
  instructions: Vec<ResourceId>,
  mcp_servers: Vec<ResourceId>,
  tools: Vec<ResourceId>,
  skills: Vec<ResourceId>,
}
impl DevtoolsInspectable for Given {}

/// One list the profile gives: its kind, its row's id and label, and where it is kept in [`Given`].
#[derive(Clone, Copy)]
struct List {
  kind: ResourceKind,
  name: &'static str,
  label: &'static str,
  of: fn(&mut Given) -> &mut Vec<ResourceId>,
}

const LISTS: [List; 4] = [
  List {
    kind: ResourceKind::Instruction,
    name: "instructions",
    label: "team_hub.profile.instructions",
    of: |given| &mut given.instructions,
  },
  List {
    kind: ResourceKind::McpServer,
    name: "mcp",
    label: "team_hub.profile.mcpServers",
    of: |given| &mut given.mcp_servers,
  },
  List {
    kind: ResourceKind::Tool,
    name: "tools",
    label: "team_hub.profile.tools",
    of: |given| &mut given.tools,
  },
  List {
    kind: ResourceKind::Skill,
    name: "skills",
    label: "team_hub.profile.skills",
    of: |given| &mut given.skills,
  },
];

/// What the panel keeps of an admin's edit of a team profile. Cloning shares it.
#[derive(Clone)]
pub(super) struct ProfileEdit {
  role: Signal<String>,
  responsibilities: Signal<String>,
  instructions: Signal<String>,
  given: Signal<Given>,
  edit: Edit<TeamAgentProfile>,
}

/// What the edit puts in the panel: its parts ([`Parts`]) and the sections that take the place of *From the team*.
pub(super) struct Drawn {
  pub parts: Parts,
  pub sections: Vec<Element>,
}

impl ProfileEdit {
  pub(super) fn new(ctx: &mut Ctx, target: Target, sheet: &TeamProfileSheet) -> Self {
    let (role, responsibilities, instructions) = (
      ctx.signal(String::new()),
      ctx.signal(String::new()),
      ctx.signal(String::new()),
    );
    let given = ctx.signal(Given::default());
    let load = {
      let (role, responsibilities, instructions, given) = (
        role.clone(),
        responsibilities.clone(),
        instructions.clone(),
        given.clone(),
      );
      Arc::new(move |body: &TeamAgentProfile| {
        role.set(body.role().as_str().to_owned());
        responsibilities.set(body.responsibilities().to_owned());
        instructions.set(body.instructions().as_str().to_owned());
        given.set(Given {
          instructions: body.instruction_resources().to_vec(),
          mcp_servers: body.mcp_servers().to_vec(),
          tools: body.tools().to_vec(),
          skills: body.skills().to_vec(),
        });
      })
    };
    let edit = Edit::new(ctx, target, sheet.edit.as_ref(), load);
    Self {
      role,
      responsibilities,
      instructions,
      given,
      edit,
    }
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
    let made = self.made(ctx, &base.body);
    self.edit.keep_up(
      sheet.edit.as_ref(),
      made.as_ref().map_or(true, |body| *body != base.body),
    );
    let role_error = Name::new(&self.role.get())
      .err()
      .map(|error| message_text(ctx, &messages::name_error(&error)));
    let parts = self.edit.parts(
      ctx,
      View {
        made,
        unreached,
        published_by: None,
      },
    )?;
    let sections = vec![
      self.role_section(ctx, role_error).into(),
      self.from_team(ctx, sheet, config).into(),
    ];
    Some(Drawn { parts, sections })
  }

  /// The profile the fields make over `base`, which keeps its CLI, model, settings, permissions and secrets.
  fn made(&self, ctx: &Ctx, base: &TeamAgentProfile) -> Result<TeamAgentProfile, Arc<str>> {
    let role = Name::new(&self.role.get()).map_err(|error| message_text(ctx, &messages::name_error(&error)))?;
    let instructions =
      PromptText::new(&self.instructions.get()).map_err(|error| message_text(ctx, &prompt_error(&error)))?;
    let given = self.given.get();
    TeamAgentProfile::new(
      role,
      self.responsibilities.get().trim().to_owned(),
      instructions,
      (base.cli(), base.model().clone(), base.settings().clone()),
    )
    .with_permissions(base.permissions().clone())
    .and_then(|profile| profile.with_mcp_servers(given.mcp_servers))
    .and_then(|profile| profile.with_tools(given.tools))
    .and_then(|profile| profile.with_skills(given.skills))
    .and_then(|profile| profile.with_instruction_resources(given.instructions))
    .and_then(|profile| profile.with_secrets(base.secrets().to_vec()))
    .map_err(|_| {
      ctx.t_args(
        "team_hub.edit.tooMany",
        [("max", TeamAgentProfile::MAX_LISTED.to_string())],
      )
    })
  }

  /// *Role*: the role, what it answers for and the role instructions, the first part of the agent's prompt.
  fn role_section(&self, ctx: &mut Ctx, role_error: Option<Arc<str>>) -> Column {
    let label = ctx.t("team_hub.edit.role");
    let mut role =
      FieldInput::new("team-hub-role-name-input", label.clone(), self.role.clone()).width(Dimension::Pct(100.0));
    let sub = role_error.map(|words| Sub {
      words,
      color: PaletteColor::Danger,
    });
    if sub.is_some() {
      role = role.invalid();
    }
    let rows = vec![
      rows::row("team-hub-role-name", &label, sub, role.render(ctx)),
      Area {
        id: "team-hub-role-responsibilities".to_owned(),
        label: ctx.t("team_hub.edit.responsibilities"),
        sub: ctx.t("team_hub.edit.responsibilitiesSub"),
        placeholder: ctx.t("team_hub.edit.responsibilitiesPlaceholder"),
        height: RESPONSIBILITIES_AREA,
      }
      .row(ctx, &self.responsibilities, None),
      Area {
        id: "team-hub-role-instructions".to_owned(),
        label: ctx.t("team_hub.edit.roleInstructions"),
        sub: ctx.t("team_hub.edit.roleInstructionsSub"),
        placeholder: ctx.t("team_hub.edit.roleInstructionsPlaceholder"),
        height: ROLE_INSTRUCTIONS_AREA,
      }
      .row(ctx, &self.instructions, None),
    ];
    rows::section(
      "team-hub-role",
      &label,
      vec![note("team-hub-role-note", &ctx.t("team_hub.edit.roleNote"))],
      rows,
    )
  }

  /// *From the team*: the CLI and model as the version has them, the lists the profile gives with *Add*, and the
  /// secrets it needs.
  fn from_team(&self, ctx: &mut Ctx, sheet: &TeamProfileSheet, config: Option<&TeamConfig>) -> Column {
    let mut rows = vec![
      rows::row(
        "team-hub-from-team-cli",
        &ctx.t("team_hub.profile.agentCli"),
        Some(Sub::quiet(ctx.t("team_hub.profile.agentCliSub"))),
        rows::words("team-hub-from-team-cli-value", &ctx.t(cli_key(sheet.cli))),
      ),
      rows::row(
        "team-hub-from-team-model",
        &ctx.t("team_hub.profile.model"),
        None,
        rows::mono("team-hub-from-team-model-value", &sheet.model),
      ),
    ];
    if let Some(effort) = &sheet.effort {
      rows.push(rows::row(
        "team-hub-from-team-effort",
        &ctx.t("team_hub.profile.effort"),
        None,
        rows::words("team-hub-from-team-effort-value", &effort_words(ctx, effort)),
      ));
    }
    let given = self.given.get();
    for list in LISTS {
      rows.push(self.list_row(ctx, list, (&given, config)));
    }
    if !sheet.secrets.is_empty() {
      rows.push(rows::row(
        "team-hub-from-team-secrets",
        &ctx.t("team_hub.profile.secrets"),
        Some(Sub::quiet(ctx.t("team_hub.profile.secretsSub"))),
        rows::tags(
          "team-hub-from-team-secrets-value",
          sheet.secrets.iter().map(ToString::to_string),
        ),
      ));
    }
    rows::section(
      "team-hub-from-team",
      &ctx.t("team_hub.profile.fromTeam"),
      vec![note("team-hub-from-team-note", &ctx.t("team_hub.profile.fromTeamNote"))],
      rows,
    )
  }

  /// One list the profile gives, in tags, and *Add*, whose menu checks the ones it gives among the team's live
  /// resources of the kind.
  fn list_row(&self, ctx: &mut Ctx, list: List, (given, config): (&Given, Option<&TeamConfig>)) -> Row {
    let id = format!("team-hub-from-team-{}", list.name);
    let chosen = (list.of)(&mut given.clone()).clone();
    let name_of = |resource: ResourceId| {
      config
        .and_then(|config| config.name(resource))
        .map_or_else(|| resource.to_string(), str::to_owned)
    };
    let items: Vec<add_menu::Item<ResourceId>> = config
      .map(|config| config.resources(list.kind))
      .unwrap_or_default()
      .into_iter()
      .map(|resource| add_menu::Item {
        id: resource.id,
        name: resource.name.as_str().to_owned(),
        chosen: chosen.contains(&resource.id),
      })
      .collect();
    let tags: Vec<Element> = chosen
      .iter()
      .enumerate()
      .map(|(index, resource)| tag_chip(&format!("{id}-value-{index}"), &name_of(*resource)).into())
      .collect();
    let toggle = {
      let given = self.given.clone();
      Arc::new(move |resource: ResourceId, chosen: bool| {
        given.update(|given| {
          let listed = (list.of)(given);
          listed.retain(|held| *held != resource);
          if chosen {
            listed.push(resource);
          }
        });
      })
    };
    let add = (!items.is_empty()).then(|| {
      add_menu::add(
        ctx,
        add_menu::Props {
          what: list.name,
          items,
          label: ctx.t("team_hub.edit.add"),
          locked: false,
          on_toggle: toggle,
        },
      )
    });
    let value = Row::new()
      .id(format!("{id}-value"))
      .width(Dimension::Pct(100.0))
      .wrap()
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .with_children(tags)
      .with_children(add);
    rows::row(&id, &ctx.t(list.label), None, value)
  }
}

fn note(id: &str, words: &str) -> Element {
  Text::new(words)
    .id(id.to_owned())
    .variant(HubText::PanelHint.role())
    .nowrap()
    .into()
}
