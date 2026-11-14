//! What the shared compact picker is given of the project (`ui::runs_on_picker`): each connection the Orchestrator can
//! run on as the rail and the line under it show it (`05 · Account and model` `v6mWp8`, `05b · On DeepSeek` `AZp3I`,
//! `Runs-on picker · states` `t7cdi` in `Orchestrator · redesign` `hujxu`), and what is chosen.
//!
//! An account the project has no connection on yet stands on the rail as a connected one does, with no models until
//! choosing it made its connection. A Claude Code or Codex sign-in is named by its account; the line under its name
//! says *CLI · plan*, and its plan's 5-hour use; at its limit the caption says until when, in `warning`, with *limit
//! reached* under it. An API provider says its CLI and host, the tokens its agents spent today and *no plan limit* (its
//! key's secret is that line's tooltip), takes a model and an effort of the person's own, and says where its data goes.
//! A sign-in to another connection or account than the conversation's session lives on says in its tooltip that
//! choosing it starts a new session.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{
  connection::AgentProvider,
  id::ConnectionId,
  orchestrator::{ChatSession, OrchestratorSettings},
  time::Timestamp,
  tokens::compact_tokens,
};
use orchester_workspace::{display, orchestrator::runs_on::RunsOn};

use super::{
  picking::efforts_for,
  sources::{Account, Entry, Open, SignedIn, Sources, cli_name, tip, tip_open},
};
use crate::{
  services::{accounts::plan_name, plan_limits::five_hour_use},
  ui::{
    agent_mark::AgentMark,
    icon::Icon,
    runs_on_picker::{Chosen, Connection, Detail, Mark, Model, Notice, NoticeTone},
  },
};

/// What the picker shows chosen: the connection, model and effort the Orchestrator runs on; nothing while none is.
pub(super) fn chosen(settings: Option<&OrchestratorSettings>) -> Chosen {
  Chosen {
    connection: settings.map_or_else(|| Arc::from(""), |settings| settings.connection().to_string().into()),
    model: settings.map(|settings| settings.model().as_str().into()),
    effort: settings.and_then(|settings| settings.effort().map(Into::into)),
  }
}

/// Choosing `connection` starts a new session where the conversation has `session`: the session lives under another
/// connection, or another account.
pub(super) fn starts_new_session(sources: &Sources<'_>, connection: ConnectionId, session: &ChatSession) -> bool {
  sources
    .setup
    .account(connection)
    .is_some_and(|account| session.change(connection, account).is_some())
}

/// The rail for the picker: the connections of `entries` and the accounts of `open`, in the rail's groups; `chosen`
/// what the Orchestrator runs on now, `session` the account the conversation's session is on, and `connecting` the
/// account whose connection is being made.
pub(super) fn connections(
  ctx: &mut Ctx,
  sources: &Sources<'_>,
  (entries, open): (&[Entry], &[Open]),
  (chosen, session): (Option<&OrchestratorSettings>, Option<&ChatSession>),
  connecting: Option<Account>,
) -> Vec<Connection> {
  let mut rail = Vec::with_capacity(entries.len() + open.len());
  for entry in entries {
    let on = chosen.filter(|chosen| chosen.connection() == entry.id);
    rail.push((entry.group, connection(ctx, sources, entry, (on, session))));
  }
  for account in open {
    let shown = unconnected(ctx, sources, account, connecting == Some(account.account));
    rail.push((account.group, shown));
  }
  // Stable: within a group the connections stand before the accounts the project has none on.
  rail.sort_by_key(|(group, _)| *group);
  rail.into_iter().map(|(_, shown)| shown).collect()
}

/// An account the project has no connection on, as the rail shows it: named and captioned as a connected one is, with
/// no models until its connection is made, which its line says while it is (`connecting`).
fn unconnected(ctx: &mut Ctx, sources: &Sources<'_>, open: &Open, connecting: bool) -> Connection {
  let tip_words = tip_open(ctx, open);
  let base = Connection {
    id: open.id().into(),
    mark: Mark::Agent(AgentMark::of(open.provider)),
    name: open.name.clone(),
    caption: Arc::from(""),
    tip: tip_words.clone(),
    five_hour: None,
    use_unread: false,
    limit: false,
    unavailable: (!open.choosable).then_some(tip_words),
    detail: None,
    tip_note: None,
    typed_model: false,
    typed_effort: false,
    notice: None,
    models: Vec::new(),
  };
  let shown = match (&open.host, open.account) {
    (Some(host), _) => Connection {
      mark: Mark::Cloud,
      caption: ctx.t_args(
        "orchestrator.runsOn.providerWho",
        [("cli", &*cli_name(ctx, open.provider)), ("host", &**host)],
      ),
      ..base
    },
    (None, Account::SignIn(id)) => sign_in(ctx, (sources.who(sources.accounts.of(id)), open.provider), base),
    (None, Account::Api(_)) => base,
  };
  if connecting {
    return Connection {
      detail: Some(Detail {
        text: ctx.t("orchestrator.runsOn.connecting"),
        tip: None,
        warns: false,
      }),
      ..shown
    };
  }
  shown
}

fn connection(
  ctx: &mut Ctx,
  sources: &Sources<'_>,
  entry: &Entry,
  (on, session): (Option<&OrchestratorSettings>, Option<&ChatSession>),
) -> Connection {
  let tip_words = tip(ctx, entry);
  let models = models(sources, entry, on);
  let base = Connection {
    id: entry.id.to_string().into(),
    mark: Mark::Agent(AgentMark::of(entry.provider)),
    name: entry.name.clone(),
    caption: Arc::from(""),
    tip: tip_words.clone(),
    five_hour: None,
    use_unread: false,
    limit: entry.limit,
    unavailable: (!entry.choosable).then_some(tip_words),
    detail: None,
    tip_note: session
      .filter(|session| starts_new_session(sources, entry.id, session))
      .map(|_| ctx.t("orchestrator.runsOn.newSession")),
    typed_model: false,
    typed_effort: false,
    notice: None,
    models,
  };
  match &entry.runs_on {
    RunsOn::ApiProvider { base_url, .. } => provider(ctx, sources, (entry, base_url.host()), base),
    RunsOn::SignIn(_) | RunsOn::Native => {
      let who = sources.signed_in(&entry.runs_on, entry.provider);
      sign_in(ctx, (who, entry.provider), base)
    }
  }
}

/// A sign-in's caption, plan use and limit.
fn sign_in(ctx: &mut Ctx, (who, provider): (SignedIn<'_>, AgentProvider), base: Connection) -> Connection {
  let cli = cli_name(ctx, provider);
  if !who.signed_in {
    let caption = ctx.t_args("orchestrator.runsOn.signedOut", [("cli", &*cli)]);
    return Connection { caption, ..base };
  }
  let now = Timestamp::now();
  let reached = who.reached(now);
  let plan = who
    .account
    .and_then(|account| account.last_check())
    .and_then(|check| check.known_identity())
    .and_then(|identity| identity.plan.as_deref());
  let until = who.reading.and_then(|reading| reading.resets_at(now));
  let caption = match (reached, until, plan) {
    (true, Some(until), _) => ctx.t_args(
      "orchestrator.runsOn.limitUntil",
      [("cli", &*cli), ("time", &display::clock(until))],
    ),
    (_, _, Some(plan)) => ctx.t_args("orchestrator.runsOn.who", [("cli", &*cli), ("plan", &plan_name(plan))]),
    _ => ctx.t_args("orchestrator.runsOn.whoNoPlan", [("cli", &*cli)]),
  };
  let used = who.reading.and_then(|reading| five_hour_use(reading, now));
  Connection {
    caption,
    five_hour: if reached { Some(100) } else { used },
    use_unread: !reached && used.is_none(),
    limit: reached,
    detail: reached.then(|| Detail {
      text: ctx.t("orchestrator.runsOn.limitReached"),
      tip: None,
      warns: true,
    }),
    ..base
  }
}

/// An API provider: its CLI and host, its tokens today and no plan limit, what it takes of the person's own, and where
/// its data goes.
fn provider(ctx: &mut Ctx, sources: &Sources<'_>, (entry, host): (&Entry, &str), base: Connection) -> Connection {
  let used = sources.uses.iter().find(|used| used.connection == entry.id);
  let tokens = used.map_or(0, |used| used.tokens);
  let tip = match used.and_then(|used| used.key.as_deref()) {
    Some(key) => ctx.t_args("orchestrator.runsOn.key", [("secret", key)]),
    None => ctx.t("orchestrator.runsOn.keyGone"),
  };
  Connection {
    mark: Mark::Cloud,
    caption: ctx.t_args(
      "orchestrator.runsOn.providerWho",
      [("cli", &*cli_name(ctx, entry.provider)), ("host", host)],
    ),
    detail: Some(Detail {
      text: ctx.t_args("orchestrator.runsOn.providerUse", [("tokens", compact_tokens(tokens))]),
      tip: Some(tip),
      warns: false,
    }),
    typed_model: true,
    typed_effort: true,
    notice: Some(Notice {
      icon: Icon::Send,
      tone: NoticeTone::Warning,
      text: ctx.t_args("orchestrator.runsOn.sentTo", [("host", host)]),
    }),
    ..base
  }
}

/// The models `entry` lists, with the one chosen on it among them, each with the efforts it takes: none on an API
/// provider, whose effort is typed.
fn models(sources: &Sources<'_>, entry: &Entry, on: Option<&OrchestratorSettings>) -> Vec<Model> {
  let provider = matches!(entry.runs_on, RunsOn::ApiProvider { .. });
  sources
    .setup
    .models(entry.id, on.map(OrchestratorSettings::model))
    .into_iter()
    .map(|option| {
      let efforts = if provider {
        Vec::new()
      } else {
        efforts_for(sources.setup, entry.id, &option.id)
      };
      Model {
        id: option.id.as_str().into(),
        efforts: efforts.into_iter().map(Arc::from).collect(),
      }
    })
    .collect()
}
