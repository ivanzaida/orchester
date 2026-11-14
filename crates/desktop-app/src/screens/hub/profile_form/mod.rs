//! The fields of an agent profile, one form for making a profile and for an admin's edit of a team one (`A7` `Cijpv`,
//! `L3` `JEkCL`; `A26` `WSw0W`, `L17` `b7Tbxb`): *Role* first (the hub's section, [`role`]), then what it runs with
//! (one field for a local profile's account, or a team profile's CLI, with the model and the effort: [`runs`]) and the
//! lists it gives its agents (instructions, MCP servers, tools and skills, each in tags with *Add*: [`given`]). The
//! fields, their checks and the profile they make are the same whether it is made or edited; the panel around them
//! decides the head, the bar, what the fields start from and where the profile goes.
//!
//! Where it is decides only what is local by nature: a team's profile runs on a CLI kind and gives the team's
//! resources; a local one names its connection and gives the project's.

mod connect;
mod held;
pub(in crate::screens) mod here;
mod made;
mod offer;
mod runs;

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::Row,
  core::{ElementRef, Signal},
  node::Element,
};
use orchester_core::{
  connection::{REASONING_EFFORT, mcp::McpServerSelection},
  id::ResourceId,
  resource::{AgentProfile, TeamAgentProfile},
};
use orchester_workspace::agents::role::{RoleErrors, RoleText};

use self::runs::{Runs, field::Menu, picker};
pub(in crate::screens) use self::{
  connect::Connect,
  held::{HeldFields, Here, OnEdit, OnHere, Standing},
  offer::Offer,
};
use crate::{
  screens::{
    hub::{
      given,
      role::{self, Change},
    },
    team_hub::rows::Sub,
  },
  services::{accounts, plan_limits},
};

/// Where the profile is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::screens) enum Scope {
  /// A team's: its CLI is a kind, chosen on each computer's connection, and it gives the team's resources.
  Team,
  /// A local project's: it names its connection and gives the project's resources.
  Local,
}

/// The lists a profile gives its agents, in the order the form has them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::screens) enum ListKind {
  Instructions,
  McpServers,
  Tools,
  Skills,
}

impl ListKind {
  pub(in crate::screens) const ALL: [Self; 4] = [Self::Instructions, Self::McpServers, Self::Tools, Self::Skills];

  pub(super) const fn at(self) -> usize {
    match self {
      Self::Instructions => 0,
      Self::McpServers => 1,
      Self::Tools => 2,
      Self::Skills => 3,
    }
  }

  /// Its row's id after the form's, and what one entry is in its menu's ids.
  const fn name(self) -> &'static str {
    match self {
      Self::Instructions => "instructions",
      Self::McpServers => "mcp",
      Self::Tools => "tools",
      Self::Skills => "skills",
    }
  }

  const fn label(self) -> &'static str {
    match self {
      Self::Instructions => "team_hub.profile.instructions",
      Self::McpServers => "team_hub.profile.mcpServers",
      Self::Tools => "team_hub.profile.tools",
      Self::Skills => "team_hub.profile.skills",
    }
  }
}

/// The ids the form's parts take: the role section's, and the rows' prefix (`<rows>-runs-on`, `<rows>-mcp`, …).
#[derive(Clone, Copy)]
pub(in crate::screens) struct Ids {
  pub role: &'static str,
  pub rows: &'static str,
}

/// How the form is drawn now.
pub(in crate::screens) struct Form<'a> {
  pub scope: Scope,
  pub ids: Ids,
  pub offer: &'a Offer,
  /// The version an edit is made on, whose permissions, other settings and secrets it keeps; `None` for a new profile.
  pub base: Option<&'a TeamAgentProfile>,
  /// Create was pressed (an edit always is): a required field that is empty says so.
  pub tried: bool,
  /// Nothing is changed now: the profile is being made or saved.
  pub locked: bool,
  /// What the role's field says instead of its own error: the name is taken.
  pub name_said: Option<Arc<str>>,
  /// The role's field, which a form that makes a profile focuses when it opens.
  pub first: Option<&'a ElementRef>,
  /// Makes the project's connection on an account looked at in the picker, in a local project's form; a team's
  /// chooses a CLI, never an account.
  pub connect: Option<&'a Connect>,
}

/// The profile the fields make.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::screens) enum Made {
  Local(Box<AgentProfile>),
  Team(Box<TeamAgentProfile>),
}

/// What the form draws: the *Role* section, the rows of what it runs with and gives (for the panel's own section),
/// and the profile they make, or why they make none.
pub(in crate::screens) struct Drawn {
  pub role: Element,
  pub rows: Vec<Row>,
  pub made: Result<Made, Arc<str>>,
  /// What it runs on is chosen, and the project's connection on that account is still being made.
  pub waits: bool,
}

/// The resources of each list the profile gives, by [`ListKind`], as the fields hold them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Given([Vec<ResourceId>; 4]);
impl lurq::app::component::DevtoolsInspectable for Given {}

/// The fields. Cloning shares them.
#[derive(Clone)]
pub(in crate::screens) struct ProfileFields {
  role: Signal<RoleText>,
  /// What it runs on, by the id of its item on the picker's rail (a local profile's account, a team profile's CLI);
  /// empty while nothing is chosen.
  on: Signal<String>,
  model: Signal<String>,
  /// The effort chosen; empty is the CLI's default.
  effort: Signal<String>,
  given: Signal<Given>,
  /// The menu of the field of what it runs with.
  menu: Menu,
}

impl ProfileFields {
  /// Empty fields, as a new profile's.
  pub(in crate::screens) fn new(ctx: &mut Ctx) -> Self {
    Self {
      role: ctx.signal(RoleText::default()),
      on: ctx.signal(String::new()),
      model: ctx.signal(String::new()),
      effort: ctx.signal(String::new()),
      given: ctx.signal(Given::default()),
      menu: Menu::new(ctx),
    }
  }

  /// The role's name as typed.
  pub(in crate::screens) fn name(&self) -> String {
    self.role.get().role
  }

  /// Every field takes the value `body` has.
  pub(in crate::screens) fn load_team(&self, body: &TeamAgentProfile) {
    self.role.set(RoleText::of_team(body));
    self.on.set(picker::kind_id(body.cli()));
    self.model.set(body.model().as_str().to_owned());
    let effort = body
      .settings()
      .iter()
      .find(|(name, _)| *name == REASONING_EFFORT)
      .map(|(_, value)| value.to_owned());
    self.effort.set(effort.unwrap_or_default());
    self.given.set(Given([
      body.instruction_resources().to_vec(),
      body.mcp_servers().to_vec(),
      body.tools().to_vec(),
      body.skills().to_vec(),
    ]));
  }

  /// The form as `form` has it.
  pub(in crate::screens) fn draw(&self, ctx: &mut Ctx, form: &Form<'_>) -> Drawn {
    let text = self.role.get();
    let given = self.given.get();
    let accounts = accounts::listed(ctx);
    let readings = plan_limits::account_readings(ctx);
    let api = accounts::api_listed(ctx);
    let who = picker::Who {
      accounts: &accounts,
      readings: &readings,
      api: &api.0,
      connecting: form.connect.and_then(Connect::connecting),
    };
    let runs = Runs::of(self, form, &who);
    let role = self.role_section(ctx, form, &text);
    let mut rows = runs.rows(ctx, self, (form, &who));
    rows.extend(ListKind::ALL.map(|kind| self.list_row(ctx, form, (kind, &given))));
    let made = made::made(ctx, form, (&text, &given), &runs);
    Drawn {
      role,
      rows,
      made,
      waits: runs.waits(),
    }
  }

  /// *Role*: the hub's section over the role as typed. A new profile's empty name says it is required only once Create
  /// was pressed.
  fn role_section(&self, ctx: &mut Ctx, form: &Form<'_>, text: &RoleText) -> Element {
    let errors = text.errors().and_then(|errors| {
      let shown = RoleErrors {
        role: errors.role.filter(|_| form.tried || !text.role.trim().is_empty()),
        instructions: errors.instructions,
      };
      (shown.role.is_some() || shown.instructions.is_some()).then_some(shown)
    });
    let change = |set: fn(&mut RoleText, String)| -> Change {
      let held = self.role.clone();
      Arc::new(move |value| held.update(|role| set(role, value)))
    };
    role::section(
      ctx,
      role::Props {
        id: form.ids.role.to_owned(),
        errors,
        locked: form.locked,
        on_role: change(|role, value| role.role = value),
        on_responsibilities: change(|role, value| role.responsibilities = value),
        on_instructions: change(|role, value| role.instructions = value),
        name_said: form.name_said.clone(),
        first: form.first.cloned(),
        role: text.clone(),
      },
    )
  }

  /// The row of the list `kind`: what the profile gives in tags, and *Add*, whose menu checks them and offers the
  /// others.
  fn list_row(&self, ctx: &mut Ctx, form: &Form<'_>, (kind, given): (ListKind, &Given)) -> Row {
    let chosen = &given.0[kind.at()];
    let held = self.given.clone();
    let on_toggle = Arc::new(move |resource: ResourceId, choose: bool| {
      held.update(|given| {
        let listed = &mut given.0[kind.at()];
        listed.retain(|found| *found != resource);
        if choose {
          listed.push(resource);
        }
      });
    });
    given::row(
      ctx,
      given::List {
        id: format!("{}-{}", form.ids.rows, kind.name()),
        what: kind.name(),
        label: ctx.t(kind.label()),
        sub: too_many(ctx, form.scope, kind, chosen.len()),
        chosen: chosen.iter().map(|id| (*id, form.offer.name_of(*id))).collect(),
        before: form.base.map(|base| base_list(base, kind)),
        offered: form.offer.lists[kind.at()]
          .iter()
          .filter(|(id, _)| !chosen.contains(id))
          .cloned()
          .collect(),
        on_toggle,
        locked: form.locked,
      },
    )
  }
}

/// The list `kind` of the version an edit starts from.
fn base_list(base: &TeamAgentProfile, kind: ListKind) -> Vec<ResourceId> {
  match kind {
    ListKind::Instructions => base.instruction_resources(),
    ListKind::McpServers => base.mcp_servers(),
    ListKind::Tools => base.tools(),
    ListKind::Skills => base.skills(),
  }
  .to_vec()
}

/// What the row of the list `kind` says while it holds `count`, more than a profile may give: a local profile chooses
/// at most so many MCP servers, and a team's lists each hold at most so many.
fn too_many(ctx: &Ctx, scope: Scope, kind: ListKind, count: usize) -> Option<Sub> {
  let words = match (scope, kind) {
    (Scope::Local, ListKind::McpServers) if count > McpServerSelection::MAX => ctx.t_args(
      "local_new.profile.serversTooMany",
      [("max", McpServerSelection::MAX.to_string())],
    ),
    (Scope::Team, _) if count > TeamAgentProfile::MAX_LISTED => ctx.t_args(
      "team_hub.edit.tooMany",
      [("max", TeamAgentProfile::MAX_LISTED.to_string())],
    ),
    _ => return None,
  };
  Some(Sub {
    words,
    color: PaletteColor::Danger,
  })
}
