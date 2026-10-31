//! *Choose a team* (`04 · Your teams` `drCQd`): signed in to Orchester Cloud, the account the step goes on with and
//! *Sign out*, the account's team servers under *YOUR TEAMS* and the invites waiting for it under *INVITES*, read
//! again whenever a server's connection or the account changes. *Join* gives this computer its own sign-in on a team's
//! server ([`TeamJoin::join_team`]), *Accept* accepts an invite and joins its server ([`TeamJoin::join_invite`]); what
//! that came to shows under its section, in place. A server this computer joined shows how many of its projects are
//! the person's and *Open* instead of *Join*; no count is shown for one it has not joined. Under the sections, a team
//! not listed leads to the code.
//!
//! *Decline* and *Sign out* ask first, in Settings › Orchester's questions in place; an invite that needs two-factor
//! authentication is accepted there too. Joining and accepting are a person's: synthetic input is refused, and the
//! page says so.

use std::sync::Arc;

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction},
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Text},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_api::cloud::{CloudInvite, CloudServer};
use orchester_core::id::{CloudAccountId, ServerId};
use orchester_workspace::sharing::{
  Servers,
  cloud::Cloud,
  team_join::{JoinError, JoinRefusal, TeamJoin, TeamServer, Teams},
};

use super::{
  cloud::account_row,
  frame::{StartStep, StepFrame, note, step_frame},
  outcome::{self, Outcome},
  rows::{self, Line},
};
use crate::{
  navigation::Navigation,
  routes::{JoinWay, OrchesterAsk, OrchesterPanel, OrchesterView, Route},
  screens::settings::orchester::{
    PersonOnly,
    connect::{Shown, join_error},
    follow,
  },
  services::Services,
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::Icon,
    pressable::{Focus, opening},
    state::loading,
  },
};

const TEAMS: &str = "join-team-teams";
const INVITES: &str = "join-team-invites";

/// What *Join* or *Accept* asked for.
#[derive(Clone)]
enum Ask {
  Team(CloudServer),
  Invite(CloudInvite),
}

/// An ask and the id of the button it was pressed with.
#[derive(Clone)]
struct Asked {
  button: String,
  ask: Ask,
}

/// What an ask came to, for the button it was pressed with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Answered<T> {
  button: String,
  answer: T,
}

impl<T> DevtoolsInspectable for Answered<T> {}

/// The page's joins. Clones share them.
#[derive(Clone)]
pub(super) struct TeamJoins {
  run: FutureAction<Asked, Answered<Outcome>, Answered<Shown>>,
  /// The button pressed last: its row shows the join under way, its section what it came to.
  pressed: Signal<Option<String>>,
  person: PersonOnly,
}

impl TeamJoins {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    let team_join = Services::of(ctx).team_join;
    Self {
      run: ctx.future_action(move |asked: Asked| join(team_join.clone(), asked)),
      pressed: ctx.signal(None),
      person: PersonOnly::new(ctx),
    }
  }

  /// Runs `ask` for the button `button`, a person's press only; nothing while another join is under way.
  fn asker(&self, button: &str, ask: Ask) -> impl Fn() + Send + Sync + 'static {
    let (run, pressed, person) = (self.run.clone(), self.pressed.clone(), self.person.clone());
    let button = button.to_owned();
    move || {
      if run.state().get_untracked().is_pending() {
        return;
      }
      pressed.set(Some(button.clone()));
      if person.allows(&button) {
        run.run_if_idle(Asked {
          button: button.clone(),
          ask: ask.clone(),
        });
      }
    }
  }

  /// Whether the join of the button `button` is under way.
  fn joining(&self, button: &str) -> bool {
    self.run.state().get().is_pending() && self.pressed.get().as_deref() == Some(button)
  }
}

/// Joins what `asked` names.
async fn join(team_join: TeamJoin, Asked { button, ask }: Asked) -> Result<Answered<Outcome>, Answered<Shown>> {
  let (joined, server) = match &ask {
    Ask::Team(server) => (team_join.join_team(server).await, server.name.as_str()),
    Ask::Invite(invite) => (team_join.join_invite(invite).await, invite.server.name.as_str()),
  };
  match joined {
    Ok(outcome) => Ok(Answered {
      answer: Outcome::of(outcome, server),
      button,
    }),
    Err(error) => {
      tracing::info!(%error, "Join a team did not join");
      Err(Answered {
        answer: join_error(&error),
        button,
      })
    }
  }
}

/// The page, signed in to the account `account` of `email`.
pub(super) fn page(ctx: &mut Ctx, joins: &TeamJoins, (email, account): (&str, CloudAccountId)) -> Element {
  let services = Services::of(ctx);
  let revision = follow(ctx, (&services.servers, &services.cloud));
  let sign_out = ActionButton::new(
    "join-team-sign-out",
    ButtonKind::Ghost,
    ctx.t("startup.join.teams.signOut"),
  )
  .icon(Icon::LogOut)
  .opens(Navigation::of(ctx).to(Route::Orchester(OrchesterView {
    ask: Some(OrchesterAsk::SignOut),
    ..OrchesterView::default()
  })));
  let mut body = vec![account_row(ctx, (email, account), Some(sign_out.into()))];
  match read(ctx, revision, (&services.servers, &services.cloud)) {
    Ok(Some(teams)) => {
      let joined = teams.servers.iter().filter_map(|team| team.joined).collect();
      let counts = counts(ctx, revision, joined, (&services.servers, &services.cloud));
      body.push(teams_section(ctx, joins, (&teams.servers, &counts), email));
      body.push(invites_section(ctx, joins, &teams.invites, email));
    }
    Ok(None) => body.push(loading("join-team-reading", &ctx.t("startup.join.teams.reading")).into()),
    Err(shown) => body.push(field_error("join-team-read-failed", &shown.text(ctx)).into()),
  }
  let label = ctx.t("startup.join.teams.useCode");
  let use_code = Focus::Link
    .apply(opening(Navigation::of(ctx).to(Route::JoinTeam(JoinWay::Code))))
    .id("join-team-use-code")
    .describe("a11y_name", label.to_string())
    .child(Text::new(&label).variant(TypographyStyle::Label));
  let not_listed = ctx.t_args("startup.join.teams.notListed", [("email", email)]);
  let frame = StepFrame {
    id: "join_team",
    ancestors: Vec::new(),
    crumb: ctx.t("startup.join.crumb").to_string(),
    step: StartStep::Join,
    title: ctx.t("startup.join.teams.title").to_string(),
    lead: ctx.t("startup.join.teams.lead").to_string(),
    body: Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Xl)
      .with_children(body)
      .into(),
    note: Some(note("join-team-not-listed", &not_listed, Some(use_code.into()))),
  };
  step_frame(ctx, frame)
}

/// *YOUR TEAMS*: a row per team server, with how many projects are the person's on the joined ones (`counts`).
fn teams_section(
  ctx: &mut Ctx,
  joins: &TeamJoins,
  (servers, counts): (&[TeamServer], &Counts),
  email: &str,
) -> Element {
  let lines = servers
    .iter()
    .enumerate()
    .map(|(index, team)| {
      let id = format!("{TEAMS}-{}", team.server.id);
      let meta = team
        .joined
        .and_then(|joined| counts.of(joined))
        .map(|count| match count {
          1 => ctx.t("startup.join.teams.projects.one").to_string(),
          count => ctx
            .t_args("startup.join.teams.projects.other", [("count", count.to_string())])
            .to_string(),
        });
      Line {
        title: rows::team_title(ctx, &id, team),
        sub: rows::team_address(&id, team),
        actions: vec![team_action(ctx, joins, (&id, team), index == 0)],
        glyph: Icon::Server,
        meta,
        id,
      }
    })
    .collect();
  let (heading, none) = (ctx.t("startup.join.teams.section"), ctx.t("startup.join.teams.none"));
  let below = answered(ctx, joins, TEAMS, email);
  rows::section(TEAMS, (&heading, &none), lines, below)
}

/// *Join*, primary on the first team as drawn, or *Open* on a server this computer joined.
fn team_action(ctx: &mut Ctx, joins: &TeamJoins, (id, team): (&str, &TeamServer), first: bool) -> Element {
  if let Some(joined) = team.joined {
    return ActionButton::new(
      format!("{id}-open"),
      ButtonKind::Secondary,
      ctx.t("startup.join.teams.open"),
    )
    .icon(Icon::ArrowRight)
    .opens(Navigation::of(ctx).to(Route::Orchester(OrchesterView::server(joined))))
    .into();
  }
  let button = format!("{id}-join");
  let kind = if first {
    ButtonKind::Primary
  } else {
    ButtonKind::Secondary
  };
  if joins.joining(&button) {
    return ActionButton::new(button, kind, ctx.t("startup.join.teams.joining"))
      .loading()
      .into();
  }
  let ask = joins.asker(&button, Ask::Team(team.server.clone()));
  ActionButton::new(button, kind, ctx.t("startup.join.teams.join"))
    .icon(Icon::ArrowRight)
    .on_click(ask)
    .into()
}

/// *INVITES*: a row per invite waiting for the account.
fn invites_section(ctx: &mut Ctx, joins: &TeamJoins, invites: &[CloudInvite], email: &str) -> Element {
  let lines = invites
    .iter()
    .map(|invite| {
      let id = format!("{INVITES}-{}", invite.id);
      Line {
        title: rows::invite_title(ctx, &id, invite),
        sub: rows::invite_sub(ctx, &id, invite),
        actions: invite_actions(ctx, joins, &id, invite),
        glyph: Icon::Mail,
        meta: None,
        id,
      }
    })
    .collect();
  let (heading, none) = (
    ctx.t("startup.join.invites.section"),
    ctx.t("startup.join.invites.none"),
  );
  let below = answered(ctx, joins, INVITES, email);
  rows::section(INVITES, (&heading, &none), lines, below)
}

/// *Accept* and *Decline*. An invite that needs two-factor authentication first is accepted in Settings ›
/// Orchester › Invites, where that step is; *Decline* asks there.
fn invite_actions(ctx: &mut Ctx, joins: &TeamJoins, id: &str, invite: &CloudInvite) -> Vec<Element> {
  let navigation = Navigation::of(ctx);
  let invites = OrchesterView {
    panel: OrchesterPanel::Invites,
    ..OrchesterView::default()
  };
  let button = format!("{id}-accept");
  let accept = if joins.joining(&button) {
    ActionButton::new(button, ButtonKind::Secondary, ctx.t("startup.join.teams.joining")).loading()
  } else if invite.needs_two_factor {
    ActionButton::new(button, ButtonKind::Secondary, ctx.t("startup.join.invites.accept"))
      .icon(Icon::Check)
      .opens(navigation.to(Route::Orchester(invites)))
  } else {
    let ask = joins.asker(&button, Ask::Invite(invite.clone()));
    ActionButton::new(button, ButtonKind::Secondary, ctx.t("startup.join.invites.accept"))
      .icon(Icon::Check)
      .on_click(ask)
  };
  let decline = ActionButton::new(
    format!("{id}-decline"),
    ButtonKind::Ghost,
    ctx.t("startup.join.invites.decline"),
  )
  .opens(navigation.to(Route::Orchester(OrchesterView {
    ask: Some(OrchesterAsk::Decline(invite.id)),
    ..invites
  })));
  vec![accept.into(), decline.into()]
}

/// Under the section whose buttons start with `section`: why synthetic input did nothing, and what the last join of
/// one of its buttons came to, once it ended.
fn answered(ctx: &mut Ctx, joins: &TeamJoins, section: &str, email: &str) -> Vec<Element> {
  let mut parts = Vec::new();
  if let Some(button) = joins.pressed.get().filter(|button| button.starts_with(section)) {
    parts.extend(joins.person.reason(ctx, &button));
  }
  let state = joins.run.state().get();
  if state.is_pending() {
    return parts;
  }
  let fulfilled = state.is_fulfilled();
  if let Some(answered) = state
    .data
    .filter(|answered| fulfilled && answered.button.starts_with(section))
  {
    parts.push(outcome::callout(
      ctx,
      &format!("{section}-outcome"),
      &answered.answer,
      Some(email),
    ));
  }
  if let Some(failed) = state.error.filter(|failed| failed.button.starts_with(section)) {
    parts.push(field_error(&format!("{section}-failed"), &failed.answer.text(ctx)).into());
  }
  parts
}

/// The account's teams and invites as read.
#[derive(Clone, Debug)]
struct Read(Arc<Teams>);

impl PartialEq for Read {
  /// The same teams, as this computer joined them, and the same invites, by id.
  fn eq(&self, other: &Self) -> bool {
    self.0.servers == other.0.servers
      && self.0.invites.len() == other.0.invites.len()
      && self
        .0
        .invites
        .iter()
        .zip(&other.0.invites)
        .all(|(one, two)| one.id == two.id && one.needs_two_factor == two.needs_two_factor)
  }
}

impl DevtoolsInspectable for Read {}

/// The account's teams and invites, read at `revision`; `None` until the first read finished.
fn read(ctx: &mut Ctx, revision: u64, (servers, cloud): (&Servers, &Cloud)) -> Result<Option<Arc<Teams>>, Shown> {
  let state = ctx
    .future(
      (revision, servers.clone(), cloud.clone()),
      |(_, servers, cloud)| async move {
        TeamJoin::new(servers, cloud)
          .teams()
          .await
          .map(|teams| Read(Arc::new(teams)))
          .map_err(|error| {
            tracing::warn!(%error, "the account's teams could not be read");
            join_error(&error)
          })
      },
    )
    .state()
    .get();
  match (state.data, state.error) {
    (Some(Read(teams)), _) => Ok(Some(teams)),
    (None, Some(shown)) => Err(shown),
    (None, None) => Ok(None),
  }
}

/// How many projects are the person's on each joined server that has some.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Counts(Vec<(ServerId, usize)>);

impl DevtoolsInspectable for Counts {}

impl Counts {
  fn of(&self, server: ServerId) -> Option<usize> {
    self.0.iter().find(|(id, _)| *id == server).map(|(_, count)| *count)
  }
}

/// The project counts of the servers `joined`, read at `revision`; none until they are read, and none for a server
/// that could not be asked (logged): the row then shows no count rather than a wrong one.
fn counts(ctx: &mut Ctx, revision: u64, joined: Vec<ServerId>, (servers, cloud): (&Servers, &Cloud)) -> Counts {
  ctx
    .future(
      (revision, joined, servers.clone(), cloud.clone()),
      |(_, joined, servers, cloud)| async move {
        let team_join = TeamJoin::new(servers, cloud);
        let mut counts = Vec::new();
        for server in joined {
          match team_join.projects(server).await {
            Ok(projects) => counts.push((server, projects.len())),
            Err(JoinError::Refused(JoinRefusal::NoProjects { .. })) => {}
            Err(error) => tracing::info!(%error, "a joined server's projects could not be counted"),
          }
        }
        Ok::<_, ()>(Counts(counts))
      },
    )
    .state()
    .get()
    .data
    .unwrap_or_default()
}
