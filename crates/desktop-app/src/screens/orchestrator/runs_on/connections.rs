//! What the shared compact picker is given of the project (`ui::runs_on_picker`): each connection the Orchestrator can
//! run on as the rail and the line under it show it (`05 · Account and model` `v6mWp8`, `05b · On DeepSeek` `AZp3I`,
//! `Runs-on picker · states` `t7cdi` in `Orchestrator · redesign` `hujxu`), and what is chosen.
//!
//! A Claude Code or Codex sign-in is named by its account; the line under its name says *CLI · plan*, and its plan's
//! 5-hour use; at its limit the caption says until when, in `warning`, with *limit reached* under it. An API provider
//! says its CLI and host, the tokens its agents spent today and *no plan limit* (its key's secret is that line's
//! tooltip), takes a model and an effort of the person's own, and says where its data goes. A sign-in to another
//! connection or account than the conversation's session lives on says in its tooltip that choosing it starts a new
//! session.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{
  id::ConnectionId,
  orchestrator::{ChatSession, OrchestratorSettings},
  time::Timestamp,
  tokens::compact_tokens,
};
use orchester_workspace::{display, orchestrator::runs_on::RunsOn};

use super::{
  picking::efforts_for,
  sources::{Entry, Sources, cli_name, tip},
};
use crate::{
  services::accounts::plan_name,
  ui::{
    agent_mark::AgentMark,
    icon::Icon,
    runs_on_picker::{Chosen, Connection, Detail, Mark, Model, Notice, NoticeTone},
  },
};

/// The 5-hour window, in minutes.
const FIVE_HOURS: u64 = 300;

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

/// The connections of `entries` for the picker, `chosen` what the Orchestrator runs on now and `session` the account
/// the conversation's session is on.
pub(super) fn connections(
  ctx: &mut Ctx,
  sources: &Sources<'_>,
  entries: &[Entry],
  (chosen, session): (Option<&OrchestratorSettings>, Option<&ChatSession>),
) -> Vec<Connection> {
  entries
    .iter()
    .map(|entry| {
      let on = chosen.filter(|chosen| chosen.connection() == entry.id);
      connection(ctx, sources, entry, (on, session))
    })
    .collect()
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
    RunsOn::SignIn(_) | RunsOn::Native => sign_in(ctx, sources, entry, base),
  }
}

/// A sign-in's caption, plan use and limit.
fn sign_in(ctx: &mut Ctx, sources: &Sources<'_>, entry: &Entry, base: Connection) -> Connection {
  let cli = cli_name(ctx, entry.provider);
  let who = sources.signed_in(&entry.runs_on, entry.provider);
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
  let used = who.reading.and_then(|reading| {
    reading
      .limits
      .windows
      .iter()
      .find(|window| window.minutes() == FIVE_HOURS && !window.expired(now))
      .and_then(|window| window.percent())
  });
  Connection {
    caption,
    five_hour: if reached { Some(100) } else { used.map(percent) },
    use_unread: !reached && used.is_none(),
    detail: reached.then(|| Detail {
      text: ctx.t("orchestrator.runsOn.limitReached"),
      tip: None,
      warns: true,
    }),
    ..base
  }
}

/// A share of at most a hundred, whole.
fn percent(used: f64) -> u8 {
  // Clamped to 0..=100 first: the cast keeps every bit.
  used.round().clamp(0.0, 100.0) as u8
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
