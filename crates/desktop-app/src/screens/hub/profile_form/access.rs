//! What a team profile's agents may do, in the table of what the team gives (`A38` `DiLWx`, `A39` `cLeY8`; a member
//! reads it as words, `M7` `NyLgu`): *Repository access*, *Commands* and *Network access* after the reasoning effort,
//! each a select. *Repository access* is the most a pipeline step of the profile may do: only a profile that can write
//! offers a step *Can write*. A select offers what the profile's CLI can enforce on this computer and what the profile
//! holds now; a list of allowed commands or hosts a version already has is one more option, kept as it is (this table
//! edits no list). The fields hold only what the person chose here ([`Chosen`]), over the permissions the form starts
//! from ([`base`]): an edit that touches none of them publishes the version's own.

use std::sync::Arc;

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx},
  components::Row,
  core::Signal,
};
use orchester_core::{
  connection::{AgentProvider, invocation::Platform},
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, NetworkAccess, RepositoryAccess,
    capability::{PermissionOption, support},
  },
  resource::TeamAgentProfile,
};
use orchester_workspace::agents::permissions::set_network;

use super::Form;
use crate::screens::{
  pipeline_hub::pick::{self, Pick},
  team_hub::rows::{self, Sub},
};

/// What the person chose in the three selects; `None` leaves a permission as the form's base has it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Chosen {
  repository: Option<RepositoryAccess>,
  commands: Option<Commands>,
  /// Network access on or off.
  network: Option<bool>,
}
impl DevtoolsInspectable for Chosen {}

/// The commands a select chooses; a list of allowed commands is the base's, never chosen here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Commands {
  Ask,
  Any,
}

/// The permissions the form starts from: the edited version's while its CLI stays, else the most restrictive `cli`
/// enforces on this computer; before a CLI is chosen, the most restrictive there is.
pub(super) fn base(kept: Option<&TeamAgentProfile>, cli: Option<AgentProvider>) -> AgentPermissions {
  match (kept, cli) {
    (Some(kept), _) => kept.permissions().clone(),
    (None, Some(cli)) => AgentPermissions::most_restrictive(cli, Platform::current()),
    (None, None) => AgentPermissions::new(
      RepositoryAccess::ReadOnly,
      CommandPermission::Allowed(AllowedCommands::default()),
      NetworkAccess::Off,
    ),
  }
}

impl Chosen {
  /// `base` with what was chosen here, each choice only where `cli` enforces it on this computer: a choice made
  /// before another CLI was picked does not outlive what that CLI can do.
  pub(super) fn over(self, mut base: AgentPermissions, cli: Option<AgentProvider>) -> AgentPermissions {
    let enforced = |option| cli.is_none_or(|cli| support(cli, Platform::current(), option).is_enforced());
    if let Some(repository) = self.repository
      && enforced(PermissionOption::of_repository(repository))
    {
      base.set_repository(repository);
    }
    match self.commands {
      Some(Commands::Ask) if enforced(PermissionOption::AskBeforeCommands) => {
        base.set_commands(CommandPermission::AskBefore);
      }
      Some(Commands::Any) if enforced(PermissionOption::AnyCommand) => {
        base.set_commands(CommandPermission::AnyInWorktree);
      }
      _ => {}
    }
    match self.network {
      Some(true) if enforced(PermissionOption::NetworkOn) => set_network(&mut base, NetworkAccess::On),
      Some(false) if enforced(PermissionOption::NetworkOff) => set_network(&mut base, NetworkAccess::Off),
      _ => {}
    }
    base
  }
}

/// One of the three permissions, as its row names it.
#[derive(Clone, Copy)]
enum Setting {
  Repository,
  Commands,
  Network,
}

impl Setting {
  const ALL: [Self; 3] = [Self::Repository, Self::Commands, Self::Network];

  const fn name(self) -> &'static str {
    match self {
      Self::Repository => "repository",
      Self::Commands => "commands",
      Self::Network => "network",
    }
  }

  const fn label(self) -> &'static str {
    match self {
      Self::Repository => "team_hub.profile.repository",
      Self::Commands => "team_hub.profile.commands",
      Self::Network => "team_hub.profile.network",
    }
  }

  /// The options of the setting, each with what enforcing it takes: `None` for a list this table does not edit, which
  /// is offered only while the profile holds it.
  const fn options(self) -> &'static [(&'static str, Option<PermissionOption>)] {
    match self {
      Self::Repository => &[
        ("read", Some(PermissionOption::ReadOnly)),
        ("write", Some(PermissionOption::EditWorktree)),
      ],
      Self::Commands => &[
        ("ask", Some(PermissionOption::AskBeforeCommands)),
        ("allowed", None),
        ("any", Some(PermissionOption::AnyCommand)),
      ],
      Self::Network => &[
        ("off", Some(PermissionOption::NetworkOff)),
        ("hosts", None),
        ("on", Some(PermissionOption::NetworkOn)),
      ],
    }
  }

  /// The option `permissions` hold. Allowed commands with none listed ask before every command, as *Ask before each
  /// command* does.
  fn held(self, permissions: &AgentPermissions) -> &'static str {
    match self {
      Self::Repository => match permissions.repository() {
        RepositoryAccess::ReadOnly => "read",
        RepositoryAccess::Write => "write",
      },
      Self::Commands => match permissions.commands() {
        CommandPermission::Allowed(allowed) if !allowed.is_empty() => "allowed",
        CommandPermission::AskBefore | CommandPermission::Allowed(_) => "ask",
        CommandPermission::AnyInWorktree => "any",
      },
      Self::Network => match permissions.network() {
        NetworkAccess::Off => "off",
        NetworkAccess::Hosts(_) => "hosts",
        NetworkAccess::On => "on",
      },
    }
  }

  /// What the option `value` says, for `permissions` that hold a list.
  fn words(self, ctx: &Ctx, value: &str, permissions: &AgentPermissions) -> String {
    let key = match (self, value) {
      (Self::Repository, "read") => "team_hub.profile.readOnly",
      (Self::Repository, _) => "team_hub.profile.canWrite",
      (Self::Commands, "ask") => "team_hub.profile.commandsAsk",
      (Self::Commands, "allowed") => {
        let count = match permissions.commands() {
          CommandPermission::Allowed(allowed) => allowed.entries().len(),
          _ => 0,
        };
        return ctx
          .t_args("team_hub.profile.commandsAllowed", [("count", count.to_string())])
          .to_string();
      }
      (Self::Commands, _) => "team_hub.profile.commandsAny",
      (Self::Network, "off") => "team_hub.profile.networkOff",
      (Self::Network, "hosts") => "team_hub.profile.networkHosts",
      (Self::Network, _) => "team_hub.profile.networkOn",
    };
    ctx.t(key).to_string()
  }

  fn sub(self, ctx: &Ctx) -> Option<Sub> {
    matches!(self, Self::Repository).then(|| Sub::quiet(ctx.t("team_hub.profile.repositorySub")))
  }

  /// What choosing `value` keeps in `chosen`; a list's option leaves the permission as the base has it.
  fn choose(self, chosen: &mut Chosen, value: &str) {
    match self {
      Self::Repository => {
        chosen.repository = Some(if value == "write" {
          RepositoryAccess::Write
        } else {
          RepositoryAccess::ReadOnly
        });
      }
      Self::Commands => {
        chosen.commands = match value {
          "ask" => Some(Commands::Ask),
          "any" => Some(Commands::Any),
          _ => None,
        };
      }
      Self::Network => {
        chosen.network = match value {
          "off" => Some(false),
          "on" => Some(true),
          _ => None,
        };
      }
    }
  }
}

/// The three rows of the form, over the `permissions` the fields make now: each a select of what `cli` can enforce
/// here and of what the profile holds.
pub(super) fn rows(
  ctx: &mut Ctx,
  form: &Form<'_>,
  chosen: &Signal<Chosen>,
  (permissions, cli): (&AgentPermissions, Option<AgentProvider>),
) -> Vec<Row> {
  Setting::ALL
    .into_iter()
    .map(|setting| {
      let held = setting.held(permissions);
      let enforced = |option| cli.is_none_or(|cli| support(cli, Platform::current(), option).is_enforced());
      let options = setting
        .options()
        .iter()
        .filter(|(value, option)| *value == held || option.is_some_and(enforced))
        .map(|(value, _)| ((*value).to_owned(), setting.words(ctx, value, permissions)))
        .collect();
      let label = ctx.t(setting.label());
      let fields = chosen.clone();
      let on_pick: Pick = Arc::new(move |value: String| fields.update(|chosen| setting.choose(chosen, &value)));
      let id = format!("{}-{}", form.ids.rows, setting.name());
      let select = pick::pick(
        ctx,
        pick::Props {
          id: format!("{id}-input"),
          name: label.to_string(),
          value: held.to_owned(),
          options,
          placeholder: None,
          mono: false,
          locked: form.locked,
          choosing: false,
          on_pick,
        },
      );
      rows::row(&id, &label, setting.sub(ctx), select)
    })
    .collect()
}

/// The three rows as a member reads them (`M7` `NyLgu`): what `permissions` hold, in words.
pub(in crate::screens) fn read_rows(ctx: &Ctx, id: &str, permissions: &AgentPermissions) -> Vec<Row> {
  Setting::ALL
    .into_iter()
    .map(|setting| {
      let row = format!("{id}-{}", setting.name());
      let words = setting.words(ctx, setting.held(permissions), permissions);
      rows::row(
        &row,
        &ctx.t(setting.label()),
        setting.sub(ctx),
        rows::words(&format!("{row}-value"), &words),
      )
    })
    .collect()
}
