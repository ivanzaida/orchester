//! The three ways in of the first start and the line under them (`Ways` `NXAOo` and `Local note` `ZZ4NG` in 01
//! `H9CZ1`; signed in to Orchester Cloud, 01b `Knacr`). Each way leads to the start page's steps in place:
//!
//! - *Choose folder…* picks a folder: one that keeps a project opens it (All projects follows the same action), and a
//!   git repository Orchester has not set up, or a folder in none, opens Add project on it. Any other refusal says why
//!   under the ways, as All projects' *Open project folder…* does (`FV5pQ`; no startup frame draws it).
//! - *Join a team*: *Sign in*, or *Your teams* once signed in, opens the step on Orchester Cloud, *Use a code* on the
//!   code.
//! - *Choose project…* opens the projects of the first team of the account this computer joined, and names that team
//!   and how many of its projects are the person's (01b); before any is joined it goes through *Join a team*
//!   (`docs/design/startup.md` decision 5). The account's invites waiting show as a badge beside it.
//!
//! The line under the ways says that no account is needed, or which one is signed in.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  components::{Column, Row},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::id::ServerId;
use orchester_workspace::sharing::{
  Servers,
  cloud::{Cloud, SignIn},
  team_join::{JoinError, JoinRefusal, TeamJoin},
};
use parking_lot::Mutex;

use super::{
  super::{
    folder::{self, OpenAction, Opened},
    page::error_line,
  },
  card::{Way, card},
};
use crate::{
  navigation::Navigation,
  root::follow_sign_in,
  routes::{JoinWay, Route, RouteFolder},
  screens::{join_team::frame::note, settings::orchester::follow},
  services::Services,
  shell::sidebar::switcher::FolderRefusal,
  ui::{
    button::{ActionButton, ButtonKind},
    chip::{BadgeTone, badge},
    icon::Icon,
  },
};

/// The ways and the line under them, *Choose folder…* running `open`.
pub(super) fn ways(ctx: &mut Ctx, open: &OpenAction) -> Element {
  ctx.mount_keyed::<Ways>("welcome-ways", Props { open: open.clone() })
}

#[derive(Clone)]
struct Props {
  open: OpenAction,
}

impl PartialEq for Props {
  /// All projects asks for its folder action first in every render, so it is the same action each time.
  fn eq(&self, _: &Self) -> bool {
    true
  }
}

impl DevtoolsInspectable for Props {}

struct Ways {
  /// Whether *Choose folder…*'s end is followed already; once per page.
  followed: Mutex<bool>,
}

impl Component for Ways {
  type Props = Props;

  fn create(_: &mut Ctx) -> Self {
    Self {
      followed: Mutex::new(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let open = ctx.props::<Props>().open.clone();
    let services = Services::of(ctx);
    // In the same order in every render: lurq keeps a component's futures by the order it asks for them.
    let email = match follow_sign_in(ctx, &services.cloud) {
      SignIn::SignedIn { email, .. } => Some(email),
      SignIn::SignedOut { .. } => None,
    };
    let revision = follow(ctx, (&services.servers, &services.cloud));
    let teams = read_teams(ctx, (revision, email.is_some()), (&services.servers, &services.cloud));
    let joined = teams.joined.as_ref().map(|joined| joined.id);
    let shared = count(ctx, (revision, joined), (&services.servers, &services.cloud));
    self.follow(ctx, &open);
    let row = Row::new()
      .id("welcome-ways-row")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Lg)
      .align_items(Alignment::Start)
      .child(folder_way(ctx, &open))
      .child(join_way(ctx, email.is_some()))
      .child(shared_way(ctx, &teams, shared));
    let local = match &email {
      Some(email) => ctx.t_args("startup.start.localSignedIn", [("email", email.as_str())]),
      None => ctx.t("startup.start.local"),
    };
    Column::new()
      .id("welcome-ways")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Section)
      .child(row)
      .with_children(failure(ctx, &open))
      .child(note("welcome-local", &local, None))
  }
}

impl Ways {
  /// Opens Add project on a chosen folder that keeps no project; follows `open` once per page.
  fn follow(&self, ctx: &mut Ctx, open: &OpenAction) {
    let mut followed = self.followed.lock();
    if *followed {
      return;
    }
    *followed = true;
    let navigation = Navigation::of(ctx);
    ctx.watch(&open.state(), move |state| {
      if state.is_pending() || !state.is_fulfilled() {
        return;
      }
      if let Some(Opened::Refused(chosen, FolderRefusal::NoProject | FolderRefusal::NotRepository)) = &state.data {
        navigation.go(Route::AddProject(RouteFolder::new(chosen)));
      }
    });
  }
}

/// *Add a project from a folder* (`PpEGM`).
fn folder_way(ctx: &mut Ctx, open: &OpenAction) -> Element {
  let choose = ActionButton::new(
    "welcome-choose-folder",
    ButtonKind::Secondary,
    ctx.t("startup.start.folder.choose"),
  )
  .icon(Icon::FolderOpen)
  .large();
  let choose = if open.state().get().is_pending() {
    choose.loading()
  } else {
    let (open, title) = (open.clone(), ctx.t("startup.start.folder.chooseTitle"));
    choose.on_click(move || open.run(title.clone()))
  };
  card(Way {
    id: "welcome-folder",
    glyph: Icon::FolderPlus,
    title: ctx.t("startup.start.folder.title").to_string(),
    body: ctx.t("startup.start.folder.body").to_string(),
    meta: (Icon::HardDrive, ctx.t("startup.start.folder.meta").to_string()),
    actions: vec![choose.into()],
  })
}

/// *Join a team* (`y6yASb`): *Sign in*, or *Your teams* when signed in (01b), and *Use a code*.
fn join_way(ctx: &mut Ctx, signed_in: bool) -> Element {
  let navigation = Navigation::of(ctx);
  let (body, first) = if signed_in {
    let teams = ActionButton::new(
      "welcome-your-teams",
      ButtonKind::Secondary,
      ctx.t("startup.start.join.yourTeams"),
    )
    .icon(Icon::Users);
    ("startup.start.join.bodySignedIn", teams)
  } else {
    let sign_in = ActionButton::new(
      "welcome-sign-in",
      ButtonKind::Secondary,
      ctx.t("startup.start.join.signIn"),
    )
    .icon(Icon::LogIn);
    ("startup.start.join.body", sign_in)
  };
  let first = first.large().opens(navigation.to(Route::JoinTeam(JoinWay::Cloud)));
  let use_code = ActionButton::new(
    "welcome-use-code",
    ButtonKind::Ghost,
    ctx.t("startup.start.join.useCode"),
  )
  .icon(Icon::KeyRound)
  .large()
  .opens(navigation.to(Route::JoinTeam(JoinWay::Code)));
  card(Way {
    id: "welcome-join",
    glyph: Icon::Users,
    title: ctx.t("startup.start.join.title").to_string(),
    body: ctx.t(body).to_string(),
    meta: (Icon::Cloud, ctx.t("startup.start.join.meta").to_string()),
    actions: vec![first.into(), use_code.into()],
  })
}

/// *Open a team's shared project* (`F61bz`): the joined team, how many of its projects are the person's (`shared`, once
/// read) and the invites waiting, as `teams` has them (`F61bz` in 01b `Knacr`).
fn shared_way(ctx: &mut Ctx, teams: &CloudTeams, shared: Option<usize>) -> Element {
  let navigation = Navigation::of(ctx);
  let (route, body, meta) = match &teams.joined {
    Some(team) => {
      let name = team.name.as_str();
      let body = match shared {
        Some(0) => ctx.t_args("startup.start.shared.joined.none", [("team", name)]),
        Some(1) => ctx.t_args("startup.start.shared.joined.one", [("team", name)]),
        Some(count) => ctx.t_args(
          "startup.start.shared.joined.other",
          [("team", name.to_owned()), ("count", count.to_string())],
        ),
        None => ctx.t_args("startup.start.shared.joined.unknown", [("team", name)]),
      };
      let meta = ctx.t_args("startup.start.shared.metaJoined", [("team", name)]);
      (
        Route::JoinTeam(JoinWay::Projects(team.id)),
        body,
        (Icon::CircleCheck, meta),
      )
    }
    None => (
      Route::JoinTeam(JoinWay::Cloud),
      ctx.t("startup.start.shared.body"),
      (Icon::Users, ctx.t("startup.start.shared.meta")),
    ),
  };
  let choose = ActionButton::new(
    "welcome-choose-project",
    ButtonKind::Secondary,
    ctx.t("startup.start.shared.choose"),
  )
  .icon(Icon::ArrowRight)
  .large()
  .opens(navigation.to(route));
  let mut actions = vec![choose.into()];
  if teams.invites > 0 {
    let word = match teams.invites {
      1 => ctx.t("startup.start.shared.invites.one"),
      count => ctx.t_args("startup.start.shared.invites.other", [("count", count.to_string())]),
    };
    actions.push(badge("welcome-invites", BadgeTone::Info, &word).into());
  }
  card(Way {
    id: "welcome-shared",
    glyph: Icon::FolderGit2,
    title: ctx.t("startup.start.shared.title").to_string(),
    body: body.to_string(),
    meta: (meta.0, meta.1.to_string()),
    actions,
  })
}

/// Why the last *Choose folder…* opened nothing; nothing for a folder Add project opened on.
fn failure(ctx: &Ctx, open: &OpenAction) -> Option<Element> {
  let state = open.state().get();
  let set_up = matches!(
    state.data,
    Some(Opened::Refused(
      _,
      FolderRefusal::NoProject | FolderRefusal::NotRepository
    ))
  );
  if set_up {
    return None;
  }
  folder::failure(ctx, open).map(|failure| error_line("welcome-error", &failure))
}

/// The first team of the account this computer joined, and how many invites wait for the account.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct CloudTeams {
  joined: Option<Joined>,
  invites: usize,
}

impl DevtoolsInspectable for CloudTeams {}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Joined {
  id: ServerId,
  name: String,
}

/// The account's teams, read at `revision` while `signed_in`; none signed out, and none until they are read. A read
/// that fails is logged and shows as none: the ways then say what holds without the account's teams, and *Your teams*
/// says why they cannot be read.
fn read_teams(ctx: &mut Ctx, (revision, signed_in): (u64, bool), (servers, cloud): (&Servers, &Cloud)) -> CloudTeams {
  ctx
    .future(
      (revision, signed_in, servers.clone(), cloud.clone()),
      |(_, signed_in, servers, cloud)| async move {
        if !signed_in {
          return Ok::<_, ()>(CloudTeams::default());
        }
        let teams = TeamJoin::new(servers, cloud).teams().await.map_err(|error| {
          tracing::warn!(%error, "the account's teams could not be read for the first start");
        })?;
        let joined = teams.servers.iter().find_map(|team| {
          team.joined.map(|id| Joined {
            id,
            name: team.server.name.clone(),
          })
        });
        Ok(CloudTeams {
          joined,
          invites: teams.invites.len(),
        })
      },
    )
    .state()
    .get()
    .data
    .unwrap_or_default()
}

/// How many of a joined server's projects are the person's, if read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Shared(Option<usize>);

impl DevtoolsInspectable for Shared {}

/// How many projects of the joined server `joined` are the person's, read at `revision`; none until read, and none
/// when the server could not be asked (logged): the way then names no count rather than a wrong one.
fn count(
  ctx: &mut Ctx,
  (revision, joined): (u64, Option<ServerId>),
  (servers, cloud): (&Servers, &Cloud),
) -> Option<usize> {
  ctx
    .future(
      (revision, joined, servers.clone(), cloud.clone()),
      |(_, joined, servers, cloud)| async move {
        let Some(server) = joined else {
          return Ok::<_, ()>(Shared(None));
        };
        Ok(match TeamJoin::new(servers, cloud).projects(server).await {
          Ok(projects) => Shared(Some(projects.len())),
          Err(JoinError::Refused(JoinRefusal::NoProjects { .. })) => Shared(Some(0)),
          Err(error) => {
            tracing::info!(%error, "a joined server's projects could not be counted for the first start");
            Shared(None)
          }
        })
      },
    )
    .state()
    .get()
    .data
    .and_then(|Shared(count)| count)
}
