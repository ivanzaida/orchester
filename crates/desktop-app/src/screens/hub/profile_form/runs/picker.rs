//! What the compact *Runs on* picker of a profile's one field is given (`Agents on accounts` C1 `RwzH2`, C2 `t7s3I`,
//! C4 `IgFWn`): everything the profile can run on as the rail and the line under it show it, and what is chosen.
//!
//! A local profile's rail is this computer's accounts, not the project's connections: one item for each sign-in of
//! Claude Code, then of Codex, whether the project has a connection on it yet or not (choosing one that has none makes
//! it), then what runs on a key. A connection on a sign-in this computer no longer lists stands by its own name. A
//! sign-in is named by its account, and its line says *CLI · plan* with the plan's 5-hour use; one that is signed out
//! says so in `warning`. An API provider says its CLI and host, takes an effort of the person's own, and says where
//! its data goes. A team profile's rail is the CLIs ([`kinds`]): the team chooses a kind, and each computer its
//! account.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{
  account::{AgentAccount, ApiAccount, ApiSetup},
  connection::{
    AgentProvider, REASONING_EFFORT,
    plan_limits::{LimitState, PlanReadings},
  },
  id::{AgentAccountId, ApiAccountId, ConnectionId},
  resource::TeamAgentProfile,
  time::Timestamp,
};
use orchester_workspace::agents::runs_on::{ModelChoice, RunsOnChoice};

use crate::{
  screens::{
    hub::profile_form::connect::On,
    team_hub::{profile::from_team::effort_words, words::cli_key},
  },
  services::{
    accounts::{Accounts, account_name, plan_name},
    plan_limits::five_hour_use,
  },
  ui::{
    agent_mark::AgentMark,
    icon::Icon,
    runs_on_picker::{Chosen, Connection, Detail, Mark, Model, Notice, NoticeTone},
  },
};

/// Who the sign-ins are, and what their plans last said.
pub(in crate::screens::hub::profile_form) struct Who<'a> {
  pub accounts: &'a Accounts,
  pub readings: &'a PlanReadings,
  /// The person's API accounts.
  pub api: &'a [ApiAccount],
  /// What the project's connection is being made on now: its line says its models are being read.
  pub connecting: Option<On>,
}

/// A sign-in of this computer on the rail.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::screens::hub::profile_form) struct OnAccount {
  pub id: AgentAccountId,
  pub cli: AgentProvider,
  /// The CLI's own sign-in, which is never another account to move work to.
  pub default: bool,
  /// The project's connection on it: the one the profile runs on when that is one of them, else the oldest.
  pub connection: Option<RunsOnChoice>,
}

/// An API account of the person's on the rail: an agent CLI on a model API, by a key the app keeps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::screens::hub::profile_form) struct OnApi {
  pub id: ApiAccountId,
  pub cli: AgentProvider,
  pub name: Arc<str>,
  /// The host its prompts and repository content go to.
  pub host: String,
  /// The models the account serves, as the person listed them in Settings.
  pub models: Vec<ModelChoice>,
  /// The project's connection on it, when it has one.
  pub connection: Option<RunsOnChoice>,
}

/// What an item of the rail stands for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::screens::hub::profile_form) enum Target {
  Account(OnAccount),
  /// One of the person's API accounts for an agent CLI.
  Api(OnApi),
  /// A connection that is no listed account's: one on a key of the project's, or on a sign-in this computer no
  /// longer lists.
  Connection(RunsOnChoice),
  /// A CLI, as a team's profile names it: the kind, and the models this computer lists for it.
  Cli(AgentProvider, Vec<ModelChoice>),
}

impl Target {
  /// The project's connection behind it, where it has one.
  pub(in crate::screens::hub::profile_form) const fn connection(&self) -> Option<&RunsOnChoice> {
    match self {
      Self::Account(account) => account.connection.as_ref(),
      Self::Api(api) => api.connection.as_ref(),
      Self::Connection(choice) => Some(choice),
      Self::Cli(..) => None,
    }
  }

  pub(in crate::screens::hub::profile_form) const fn cli(&self) -> AgentProvider {
    match self {
      Self::Account(account) => account.cli,
      Self::Api(api) => api.cli,
      Self::Connection(choice) => choice.cli,
      Self::Cli(cli, _) => *cli,
    }
  }

  /// The models it lists; none for a sign-in the project has no connection on yet.
  pub(in crate::screens::hub::profile_form) fn models(&self) -> &[ModelChoice] {
    match self {
      Self::Cli(_, models) => models,
      Self::Api(api) => &api.models,
      on => on.connection().map_or(&[], |choice| &choice.models),
    }
  }

  /// Whether it takes an effort of the person's own, not one of a list: an API provider.
  /// What the project gets a connection on to run on it, while it has none: a sign-in, or an API account.
  pub(in crate::screens::hub::profile_form) const fn unconnected(&self) -> Option<On> {
    match self {
      Self::Account(OnAccount {
        id, connection: None, ..
      }) => Some(On::Account(*id)),
      Self::Api(OnApi {
        id, connection: None, ..
      }) => Some(On::Api(*id)),
      _ => None,
    }
  }

  /// The host of the model API its work goes to instead of a sign-in's.
  pub(in crate::screens::hub::profile_form) fn host(&self) -> Option<&str> {
    match self {
      Self::Api(api) => Some(&api.host),
      Self::Connection(choice) => choice.host.as_deref(),
      Self::Account(_) | Self::Cli(..) => None,
    }
  }

  pub(in crate::screens::hub::profile_form) const fn types_effort(&self) -> bool {
    matches!(
      self,
      Self::Api(_) | Self::Connection(RunsOnChoice { host: Some(_), .. })
    )
  }
}

/// An item of the rail, by the id the picker hands back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::screens::hub::profile_form) struct Item {
  pub id: Arc<str>,
  pub target: Target,
}

impl Item {
  /// Whether the profile on `current` runs on it.
  pub(in crate::screens::hub::profile_form) fn is(&self, current: Option<ConnectionId>) -> bool {
    current.is_some() && self.target.connection().map(|choice| choice.id) == current
  }
}

/// What a profile on `current` can run on among `runs_on`, in the rail's order: Claude Code's sign-ins, Codex's, then
/// the connections that are no listed account's.
pub(in crate::screens::hub::profile_form) fn items(
  runs_on: &[RunsOnChoice],
  current: Option<ConnectionId>,
  who: &Who<'_>,
) -> Vec<Item> {
  let mut items = Vec::new();
  let mut taken: Vec<ConnectionId> = Vec::new();
  for cli in [AgentProvider::ClaudeCodeCli, AgentProvider::CodexCli] {
    for account in who.accounts.of_provider(cli) {
      let on: Vec<&RunsOnChoice> = runs_on
        .iter()
        .filter(|choice| choice.host.is_none() && choice.cli == cli && choice.account == Some(account.id()))
        .collect();
      taken.extend(on.iter().map(|choice| choice.id));
      let connection = on
        .iter()
        .find(|choice| Some(choice.id) == current)
        .or_else(|| on.first())
        .map(|choice| (*choice).clone());
      items.push(Item {
        id: format!("account-{}", account.id()).into(),
        target: Target::Account(OnAccount {
          id: account.id(),
          cli,
          default: account.is_default(),
          connection,
        }),
      });
    }
  }
  for account in who.api {
    let ApiSetup::Cli { cli, base_url, models } = account.setup() else {
      continue;
    };
    let connection = runs_on
      .iter()
      .find(|choice| choice.cli == cli.provider() && choice.api_account == Some(account.id()));
    taken.extend(connection.map(|choice| choice.id));
    items.push(Item {
      id: format!("api-{}", account.id()).into(),
      target: Target::Api(OnApi {
        id: account.id(),
        cli: cli.provider(),
        name: account.name().as_str().into(),
        host: base_url.host().to_owned(),
        models: models
          .iter()
          .map(|model| ModelChoice {
            id: model.as_str().to_owned(),
            efforts: Vec::new(),
          })
          .collect(),
        connection: connection.cloned(),
      }),
    });
  }
  let mut rest: Vec<&RunsOnChoice> = runs_on.iter().filter(|choice| !taken.contains(&choice.id)).collect();
  rest.sort_by_key(|choice| choice.host.is_some());
  items.extend(rest.into_iter().map(|choice| Item {
    id: choice.id.to_string().into(),
    target: Target::Connection(choice.clone()),
  }));
  items
}

/// The account the picker opens on while nothing is chosen, when the project has no connection on it yet: the first
/// of the rail.
pub(in crate::screens::hub::profile_form) fn opens_unconnected(items: &[Item]) -> Option<On> {
  items.first().and_then(|item| item.target.unconnected())
}

/// The id of the rail's item of the CLI `cli` in a team profile's form: an identifier, never shown.
pub(in crate::screens::hub::profile_form) fn kind_id(cli: AgentProvider) -> String {
  format!("cli-{cli:?}")
}

/// The rail of a team profile's form: the CLIs this computer has a connection of, and the one the edited version
/// `base` has, each with the models the first connection of its kind lists here. The version's own model and effort
/// are kept though this computer does not list them.
pub(in crate::screens::hub::profile_form) fn kinds(
  runs_on: &[RunsOnChoice],
  base: Option<&TeamAgentProfile>,
) -> Vec<Item> {
  let mut clis: Vec<AgentProvider> = Vec::new();
  for cli in runs_on
    .iter()
    .map(|choice| choice.cli)
    .chain(base.map(TeamAgentProfile::cli))
  {
    if !clis.contains(&cli) {
      clis.push(cli);
    }
  }
  clis
    .into_iter()
    .map(|cli| {
      let mut models = runs_on
        .iter()
        .find(|choice| choice.cli == cli)
        .map_or_else(Vec::new, |choice| choice.models.clone());
      if let Some(base) = base.filter(|base| base.cli() == cli) {
        keep(&mut models, base);
      }
      Item {
        id: kind_id(cli).into(),
        target: Target::Cli(cli, models),
      }
    })
    .collect()
}

/// Adds the model of `base`, and the effort it has, to `models` where they are not listed: what the version has,
/// though this computer does not list it.
fn keep(models: &mut Vec<ModelChoice>, base: &TeamAgentProfile) {
  let model = base.model().as_str();
  if !models.iter().any(|listed| listed.id == model) {
    models.push(ModelChoice {
      id: model.to_owned(),
      efforts: Vec::new(),
    });
  }
  let effort = base
    .settings()
    .iter()
    .find(|(name, _)| *name == REASONING_EFFORT)
    .map(|(_, value)| value.to_owned());
  if let (Some(effort), Some(listed)) = (effort, models.iter_mut().find(|listed| listed.id == model))
    && !listed.efforts.contains(&effort)
  {
    listed.efforts.push(effort);
  }
}

/// What the field holds: the item the profile runs on, its model and its effort.
#[derive(Clone, Copy)]
pub(in crate::screens::hub::profile_form) struct Held<'a> {
  pub on: Option<&'a Item>,
  pub model: Option<&'a str>,
  pub effort: Option<&'a str>,
  /// A team's profile, whose field chooses a CLI where a local one's chooses an account.
  pub team: bool,
}

/// What the field says, and whether the profile runs on something this computer has.
pub(in crate::screens::hub::profile_form) struct Label {
  pub words: Arc<str>,
  pub chosen: bool,
}

/// *Claude Code · Work · claude-opus-5-5 · High*, a team's without the account; while the profile's connection is
/// gone, that an account is to be chosen, with the model and effort it keeps; while nothing is chosen, that.
pub(in crate::screens::hub::profile_form) fn label(ctx: &Ctx, held: Held<'_>, who: &Who<'_>) -> Label {
  let effort = held.effort.map(|effort| effort_words(ctx, effort));
  let words = match (held.on, held.model) {
    (Some(item), Some(model)) => {
      let cli = ctx.t(cli_key(item.target.cli()));
      let name = name_of(ctx, &item.target, who);
      let mut said = vec![("cli", &*cli), ("name", &*name), ("model", model)];
      let key = match (&effort, held.team) {
        (Some(_), false) => "local_hub.runsOn.valueEffort",
        (None, false) => "local_hub.runsOn.value",
        (Some(_), true) => "team_hub.runsOn.valueEffort",
        (None, true) => "team_hub.runsOn.value",
      };
      if let Some(effort) = &effort {
        said.push(("effort", effort));
      }
      if held.team {
        said.retain(|(name, _)| *name != "name");
      }
      ctx.t_args(key, said)
    }
    (None, Some(model)) => match &effort {
      Some(effort) => ctx.t_args(
        "local_hub.runsOn.unchosenEffort",
        [("model", model), ("effort", effort)],
      ),
      None => ctx.t_args("local_hub.runsOn.unchosen", [("model", model)]),
    },
    (_, None) if held.team => ctx.t("team_hub.runsOn.placeholder"),
    (_, None) => ctx.t("local_hub.runsOn.placeholder"),
  };
  Label {
    words,
    chosen: held.on.is_some() && held.model.is_some(),
  }
}

/// What the picker shows chosen.
pub(in crate::screens::hub::profile_form) fn chosen(held: Held<'_>) -> Chosen {
  Chosen {
    connection: held.on.map_or_else(|| Arc::from(""), |item| item.id.clone()),
    model: held.model.map(Into::into),
    effort: held.effort.map(Into::into),
  }
}

/// `items` as the picker's rail takes them.
pub(in crate::screens::hub::profile_form) fn connections(ctx: &Ctx, items: &[Item], who: &Who<'_>) -> Vec<Connection> {
  items.iter().map(|item| connection(ctx, item, who)).collect()
}

/// The account's name for a sign-in this computer lists, else the connection's own.
pub(in crate::screens::hub::profile_form) fn name_of(ctx: &Ctx, target: &Target, who: &Who<'_>) -> Arc<str> {
  match target {
    Target::Account(on) => who
      .accounts
      .of(on.id)
      .map_or_else(|| Arc::from(""), |account| account_name(ctx, account).into()),
    Target::Api(api) => api.name.clone(),
    Target::Connection(choice) => choice.name.as_str().into(),
    Target::Cli(cli, _) => ctx.t(cli_key(*cli)),
  }
}

fn connection(ctx: &Ctx, item: &Item, who: &Who<'_>) -> Connection {
  let cli = ctx.t(cli_key(item.target.cli()));
  let name = name_of(ctx, &item.target, who);
  let base = Connection {
    id: item.id.clone(),
    mark: Mark::Agent(AgentMark::of(item.target.cli())),
    tip: match &item.target {
      Target::Cli(..) => cli.clone(),
      _ => ctx.t_args("local_hub.runsOn.tip", [("cli", &*cli), ("name", &*name)]),
    },
    name,
    caption: cli.clone(),
    five_hour: None,
    use_unread: false,
    limit: false,
    unavailable: None,
    detail: None,
    tip_note: None,
    typed_model: false,
    typed_effort: false,
    notice: None,
    models: models(item.target.models()),
  };
  if let Some(On::Api(_)) = item.target.unconnected().filter(|on| who.connecting == Some(*on)) {
    return Connection {
      mark: Mark::Cloud,
      detail: Some(Detail {
        text: ctx.t("local_hub.runsOn.reading"),
        tip: None,
        warns: false,
      }),
      ..base
    };
  }
  match (&item.target, item.target.host()) {
    (Target::Api(_) | Target::Connection(_), Some(host)) => Connection {
      mark: Mark::Cloud,
      caption: ctx.t_args("local_hub.runsOn.provider", [("cli", &*cli), ("host", host)]),
      typed_effort: true,
      notice: Some(Notice {
        icon: Icon::Send,
        tone: NoticeTone::Warning,
        text: ctx.t_args("local_hub.runsOn.sentTo", [("host", host)]),
      }),
      ..base
    },
    (Target::Connection(choice), _) => sign_in(ctx, (None, choice.signed_out), who, (base, &cli)),
    // An API account always has a host.
    (Target::Api(_), None) => base,
    (Target::Cli(..), _) => Connection {
      caption: ctx.t("team_hub.runsOn.caption"),
      ..base
    },
    (Target::Account(on), _) if on.connection.is_none() && who.connecting == Some(On::Account(on.id)) => Connection {
      detail: Some(Detail {
        text: ctx.t("local_hub.runsOn.reading"),
        tip: None,
        warns: false,
      }),
      ..base
    },
    (Target::Account(on), _) => {
      let signed_out = on.connection.as_ref().is_some_and(|choice| choice.signed_out);
      sign_in(ctx, (who.accounts.of(on.id), signed_out), who, (base, &cli))
    }
  }
}

fn models(listed: &[ModelChoice]) -> Vec<Model> {
  listed
    .iter()
    .map(|model| Model {
      id: model.id.as_str().into(),
      efforts: model.efforts.iter().map(|effort| effort.as_str().into()).collect(),
    })
    .collect()
}

/// A sign-in's line: its plan and the plan's 5-hour use, that its limit is reached, or that it is signed out.
fn sign_in(
  ctx: &Ctx,
  (account, signed_out): (Option<&AgentAccount>, bool),
  who: &Who<'_>,
  (base, cli): (Connection, &str),
) -> Connection {
  if signed_out {
    return Connection {
      detail: Some(Detail {
        text: ctx.t("local_hub.runsOn.notSignedIn"),
        tip: None,
        warns: true,
      }),
      ..base
    };
  }
  let plan = account
    .and_then(|account| account.last_check())
    .and_then(|check| check.known_identity())
    .and_then(|identity| identity.plan.as_deref());
  let caption = match plan {
    Some(plan) => ctx.t_args("local_hub.runsOn.who", [("cli", cli), ("plan", &plan_name(plan))]),
    None => base.caption.clone(),
  };
  let now = Timestamp::now();
  let reading = account.and_then(|account| who.readings.of_account(account.id()));
  let reached = reading.is_some_and(|reading| reading.state(now) == LimitState::Reached);
  let used = reading.and_then(|reading| five_hour_use(reading, now));
  Connection {
    caption,
    five_hour: if reached { Some(100) } else { used },
    use_unread: account.is_some() && !reached && used.is_none(),
    limit: reached,
    detail: reached.then(|| Detail {
      text: ctx.t("local_hub.runsOn.limitReached"),
      tip: None,
      warns: true,
    }),
    ..base
  }
}
