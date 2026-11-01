//! A team server the account joined without this computer, in the browser (`docs/architecture/server.md`, Invites ›
//! *What the desktop sees afterwards*): Orchester Cloud lists it (`GET /servers`) while this computer has no entry of
//! it. Its row under *Team servers* says *Joined · this computer isn't connected* and has *Connect*, which joins it
//! with the account as a new computer does (row 44: an identity token without an invite), so this computer gets its
//! own device token; with join approval on, the join waits for an admin. Once joined, the server has its own row, and
//! its page lists its projects, each with *Link a checkout…*, which opens the join review. No frame draws this row;
//! it follows the other server rows (`z9Zzt`).

use std::sync::Arc;

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction},
    theme::PaletteColor,
  },
  core::Signal,
  node::Element,
};
use orchester_api::{client::ServerAddress, cloud::CloudServer};
use orchester_core::id::TeamServerId;
use orchester_store::StoredCloudAccount;
use orchester_workspace::sharing::{
  ServerEntry,
  cloud::Cloud,
  team_join::{JoinOutcome, JoinedServer, TeamJoin},
};

use super::{
  PersonOnly,
  connect::{Shown, join_error},
};
use crate::{
  navigation::Navigation,
  routes::{OrchesterView, Route},
  services::Services,
  theme::IconSize,
  ui::{
    button::{ActionButton, ButtonKind},
    callout::{Callout, CalloutTone},
    field::field_error,
    icon::{Icon, icon},
    setting_rows::{SettingRow, row_tile},
  },
};

/// Where the answers to *Connect* show, under the group.
const ANSWER: &str = "orchester-joined-answer";

/// The account's team servers, as Orchester Cloud lists them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Listed(Arc<Vec<CloudServer>>);

impl DevtoolsInspectable for Listed {}

impl Listed {
  /// The listed servers this computer has no entry of, each at its address.
  pub(super) fn not_here(&self, entries: &[ServerEntry]) -> Vec<(ServerAddress, CloudServer)> {
    self
      .0
      .iter()
      .filter_map(|server| {
        // A server the cloud names at no address can't be connected to: no row offers it.
        let address = ServerAddress::parse(&server.address).ok()?;
        (!entries.iter().any(|entry| entry.address == address)).then(|| (address, server.clone()))
      })
      .collect()
  }
}

/// The team servers of `account`, read at `revision`; none while signed out or when the cloud could not say (logged).
pub(super) fn listed(ctx: &mut Ctx, revision: u64, cloud: &Cloud, account: Option<&StoredCloudAccount>) -> Listed {
  let signed_in = account.map(|account| account.account);
  ctx
    .future(
      (revision, signed_in, cloud.clone()),
      |(_, signed_in, cloud)| async move {
        if signed_in.is_none() {
          return Ok::<_, ()>(Listed::default());
        }
        let listed = cloud
          .team_servers()
          .await
          .map_err(|error| tracing::warn!(%error, "the account's team servers could not be read"))?;
        Ok(Listed(Arc::new(
          listed.into_iter().map(|listed| listed.server).collect(),
        )))
      },
    )
    .state()
    .get()
    .data
    .unwrap_or_default()
}

/// What *Connect* came to.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Came {
  Joined(JoinedServer),
  /// Join approval holds the join until an admin approves this computer.
  Waiting,
  /// The server has no account of this identity and no invite for its email.
  NotInvited,
}

/// An answer about the server *Connect* was pressed for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Answered<T> {
  server: CloudServer,
  answer: T,
}

impl<T> DevtoolsInspectable for Answered<T> {}

/// The server whose *Connect* was pressed last.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Pressed(Option<TeamServerId>);

impl DevtoolsInspectable for Pressed {}

/// The column's *Connect*s. Clones share them.
#[derive(Clone)]
pub(super) struct Connects {
  run: FutureAction<CloudServer, Answered<Came>, Answered<Shown>>,
  pressed: Signal<Pressed>,
  /// Joining is a person's (review 1, M1).
  person: PersonOnly,
}

impl Connects {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    let team_join = Services::of(ctx).team_join;
    Self {
      run: ctx.future_action(move |server: CloudServer| connect(team_join.clone(), server)),
      pressed: ctx.signal(Pressed(None)),
      person: PersonOnly::new(ctx),
    }
  }

  fn connecting(&self, server: TeamServerId) -> bool {
    self.run.state().get().is_pending() && self.pressed.get().0 == Some(server)
  }
}

async fn connect(team_join: TeamJoin, server: CloudServer) -> Result<Answered<Came>, Answered<Shown>> {
  match team_join.join_team(&server).await {
    Ok(outcome) => {
      let answer = match outcome {
        JoinOutcome::Joined(joined) => Came::Joined(joined),
        JoinOutcome::Waiting(_) => Came::Waiting,
        JoinOutcome::NotInvited => Came::NotInvited,
      };
      Ok(Answered { server, answer })
    }
    Err(error) => {
      tracing::info!(%error, "Connect did not join a server joined in the browser");
      Err(Answered {
        answer: join_error(&error),
        server,
      })
    }
  }
}

/// The row of `server`, at `address`, with *Connect*.
pub(super) fn row(ctx: &mut Ctx, connects: &Connects, (address, server): &(ServerAddress, CloudServer)) -> Element {
  let row_id = format!("orchester-joined-{}", server.id);
  let button = format!("{row_id}-connect");
  let connect = if connects.connecting(server.id) {
    ActionButton::new(button, ButtonKind::Secondary, ctx.t("cloud.list.browser.connecting")).loading()
  } else {
    let (run, pressed, person) = (connects.run.clone(), connects.pressed.clone(), connects.person.clone());
    let (pressed_id, server) = (button.clone(), server.clone());
    ActionButton::new(button, ButtonKind::Secondary, ctx.t("cloud.list.browser.connect"))
      .icon(Icon::LogIn)
      .on_click(move || {
        if run.state().get_untracked().is_pending() {
          return;
        }
        pressed.set(Pressed(Some(server.id)));
        if person.allows(&pressed_id) {
          run.run(server.clone());
        }
      })
  };
  SettingRow::new(row_id, address.authority())
    .lead(row_tile(icon(Icon::Server, IconSize::Md, PaletteColor::TextSecondary)))
    .hint(ctx.t("cloud.list.browser.line"))
    .control(connect)
    .render(ctx)
}

/// Under the group: why synthetic input did nothing, and what the last *Connect* came to once it ended.
pub(super) fn answers(ctx: &mut Ctx, connects: &Connects) -> Vec<Element> {
  let mut parts = Vec::new();
  if let Pressed(Some(server)) = connects.pressed.get() {
    parts.extend(
      connects
        .person
        .reason(ctx, &format!("orchester-joined-{server}-connect")),
    );
  }
  let state = connects.run.state().get();
  if state.is_pending() {
    return parts;
  }
  let fulfilled = state.is_fulfilled();
  if let Some(answered) = state.data.filter(|_| fulfilled) {
    parts.push(came(ctx, &answered));
  }
  if let Some(failed) = state.error {
    parts.push(field_error(&format!("{ANSWER}-failed"), &failed.answer.text(ctx)).into());
  }
  parts
}

fn came(ctx: &mut Ctx, Answered { server, answer }: &Answered<Came>) -> Element {
  let name = server.name.as_str();
  match answer {
    Came::Joined(JoinedServer { id, name, username }) => {
      let open = ActionButton::new(
        format!("{ANSWER}-open"),
        ButtonKind::Secondary,
        ctx.t("cloud.list.browser.joined.open"),
      )
      .icon(Icon::ArrowRight)
      .opens(Navigation::of(ctx).to(Route::Orchester(OrchesterView::server(*id))));
      Callout::new(
        format!("{ANSWER}-joined"),
        CalloutTone::Success,
        ctx.t_args(
          "cloud.list.browser.joined.title",
          [("server", name.as_str()), ("username", username.as_str())],
        ),
      )
      .body(ctx.t("cloud.list.browser.joined.body"))
      .action(open)
      .into()
    }
    Came::Waiting => Callout::new(
      format!("{ANSWER}-waiting"),
      CalloutTone::Warning,
      ctx.t("cloud.invites.approval.title"),
    )
    .icon(Icon::Hourglass)
    .body(ctx.t_args("cloud.list.browser.waiting", [("server", name)]))
    .into(),
    Came::NotInvited => {
      let email = Services::of(ctx)
        .cloud
        .account()
        .ok()
        .flatten()
        .map(|account| account.email)
        .unwrap_or_default();
      Callout::new(
        format!("{ANSWER}-not-invited"),
        CalloutTone::Warning,
        ctx.t_args(
          "sharing.connect.notInvitedTitle",
          [("server", name), ("email", email.as_str())],
        ),
      )
      .icon(Icon::UserX)
      .body(ctx.t("sharing.connect.notInvitedBody"))
      .into()
    }
  }
}
