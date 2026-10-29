//! The status bar's server item (`X39PHh` in the *Settings* layer, after old 10.43 `hYres`): for a project linked to a
//! team server, that server with its state; otherwise the one server this computer is signed in to, or how many and
//! the worst state among them. Connected is quiet; connecting is muted; offline and signed out are warnings; a changed
//! certificate is danger. A press opens the server's entry in Settings › Orchester (or its list for several).

use std::path::PathBuf;

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, StreamEmitter},
    theme::PaletteColor,
  },
  node::Element,
};
use orchester_core::id::ServerId;
use orchester_store::ProjectDb;
use orchester_workspace::{
  background,
  sharing::{Connection, ServerEntry, Servers},
};

use super::{
  facts::{Dispatch, go},
  item::{glyph, label, pressed},
};
use crate::{
  routes::{OrchesterView, Route},
  ui::icon::Icon,
};

/// A server's state as the item shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ServerState {
  Connected,
  Connecting,
  Offline,
  SignedOut,
  CertificateChanged,
}

impl ServerState {
  pub fn of(entry: &ServerEntry) -> Self {
    if entry.certificate_changed {
      return Self::CertificateChanged;
    }
    match entry.connection {
      Some(Connection::Connected) => Self::Connected,
      Some(Connection::Checking | Connection::Reconnecting { .. }) => Self::Connecting,
      Some(Connection::Offline { .. }) => Self::Offline,
      None | Some(Connection::SignedOut) => Self::SignedOut,
    }
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Connected => "connected",
      Self::Connecting => "connecting",
      Self::Offline => "offline",
      Self::SignedOut => "signed-out",
      Self::CertificateChanged => "certificate-changed",
    }
  }
}

/// What the item shows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ServerFacts {
  /// No server is in use from this computer: no item.
  #[default]
  None,
  /// The project's server, or the one server in use.
  One {
    id: ServerId,
    name: String,
    state: ServerState,
  },
  /// Several servers in use and none linked to the project: how many, and the worst state among them.
  Several { count: usize, worst: ServerState },
}

impl DevtoolsInspectable for ServerFacts {}

impl ServerFacts {
  /// The item for `entries` (the servers in the list) when the shown project is linked to `linked`.
  pub fn of(entries: &[ServerEntry], linked: Option<ServerId>) -> Self {
    let one = |entry: &ServerEntry| Self::One {
      id: entry.stored.id,
      name: entry.stored.name.clone(),
      state: ServerState::of(entry),
    };
    if let Some(entry) = linked.and_then(|linked| entries.iter().find(|entry| entry.stored.id == linked)) {
      return one(entry);
    }
    let used: Vec<&ServerEntry> = entries.iter().filter(|entry| entry.in_use()).collect();
    match used.as_slice() {
      [] => Self::None,
      [entry] => one(entry),
      several => Self::Several {
        count: several.len(),
        worst: several
          .iter()
          .map(|entry| ServerState::of(entry))
          .max()
          .unwrap_or(ServerState::Connected),
      },
    }
  }
}

/// The servers as the item shows them for the project at `project_root`, read again whenever a connection changes.
pub fn server_facts(ctx: &mut Ctx, servers: &Servers, project_root: Option<PathBuf>) -> ServerFacts {
  let revision = ctx
    .stream(servers.clone(), |servers, emitter: StreamEmitter<u64, ()>| async move {
      let mut changes = servers.subscribe();
      loop {
        let count = *changes.borrow_and_update();
        if !emitter.emit(count) || changes.changed().await.is_err() {
          return;
        }
      }
    })
    .state()
    .get()
    .data
    .unwrap_or_default();
  ctx
    .future(
      (revision, project_root, servers.clone()),
      |(_, root, servers)| async move {
        background::run(move || {
          let entries = servers.entries().map_err(|error| {
            tracing::warn!(%error, "the status bar could not read the servers");
          })?;
          // A project whose database cannot be read names no server here; its own screens say why.
          let linked = root
            .and_then(|root| ProjectDb::open(&root).and_then(|db| db.sharing()).ok().flatten())
            .map(|link| link.server);
          Ok::<_, ()>(ServerFacts::of(&entries, linked))
        })
        .await
        .map_err(drop)?
      },
    )
    .state()
    .get()
    .data
    .unwrap_or_default()
}

/// The item, or nothing when no server is in use or it is connected: connected is quiet, the health dot's card says
/// it (`13 · Team server offline or signed out` `BfW9V`).
pub fn item(ctx: &Ctx, facts: &ServerFacts, dispatch: &Dispatch) -> Option<Element> {
  let (id, state) = match facts {
    ServerFacts::None => return None,
    ServerFacts::One { id, state, .. } => (Some(*id), *state),
    ServerFacts::Several { worst, .. } => (None, *worst),
  };
  let (icon, color) = match state {
    ServerState::Connected => return None,
    ServerState::Connecting => (Icon::LoaderCircle, PaletteColor::TextMuted),
    ServerState::Offline => (Icon::WifiOff, PaletteColor::Warning),
    ServerState::SignedOut => (Icon::LogOut, PaletteColor::Warning),
    ServerState::CertificateChanged => (Icon::ShieldAlert, PaletteColor::Danger),
  };
  let words = words(ctx, facts)?;
  let route = Route::Orchester(id.map_or_else(OrchesterView::default, OrchesterView::server));
  Some(
    pressed("status-server", go(dispatch, route))
      .describe("state", state.name())
      .describe("a11y_name", words.clone())
      .child(glyph(icon, color.clone()))
      .child(label(&words, color))
      .into(),
  )
}

/// What the item, or the health dot's card, says of the servers; nothing when none is in use.
pub(super) fn words(ctx: &Ctx, facts: &ServerFacts) -> Option<String> {
  match facts {
    ServerFacts::None => None,
    ServerFacts::One { name, state, .. } => Some(one_words(ctx, name, *state)),
    ServerFacts::Several { count, worst } => Some(several_words(ctx, *count, *worst)),
  }
}

fn one_words(ctx: &Ctx, name: &str, state: ServerState) -> String {
  let key = match state {
    ServerState::Connected => "shell.status.server.connected",
    ServerState::Connecting => "shell.status.server.connecting",
    ServerState::Offline => "shell.status.server.offline",
    ServerState::SignedOut => "shell.status.server.signedOut",
    ServerState::CertificateChanged => "shell.status.server.certificateChanged",
  };
  ctx.t_args(key, [("server", name)]).to_string()
}

fn several_words(ctx: &Ctx, count: usize, worst: ServerState) -> String {
  let key = match worst {
    ServerState::Connected => "shell.status.server.several.connected",
    ServerState::Connecting => "shell.status.server.several.connecting",
    ServerState::Offline => "shell.status.server.several.offline",
    ServerState::SignedOut => "shell.status.server.several.signedOut",
    ServerState::CertificateChanged => "shell.status.server.several.certificateChanged",
  };
  ctx.t_args(key, [("count", count.to_string())]).to_string()
}
