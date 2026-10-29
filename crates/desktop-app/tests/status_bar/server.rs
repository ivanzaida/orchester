//! The team server's item (`X39PHh`): the project's server, or the one in use, or how many and the worst state; a
//! press opens the server's entry in Settings › Orchester (or its list for several).
use std::time::Instant;

use orchester_api::client::ServerAddress;
use orchester_app::{
  navigation::press::OpenIn,
  routes::{OrchesterView, Route},
  shell::status_bar::{BarAction, BarFacts, ServerFacts, ServerState},
};
use orchester_core::{
  id::{ServerId, UserId},
  time::Timestamp,
};
use orchester_store::{ServerAccount, StoredServer};
use orchester_workspace::sharing::{Connection, ServerEntry};

use super::{Bar, facts};

fn entry(name: &str, connection: Option<Connection>) -> ServerEntry {
  let url = format!("https://{}.example", name.to_lowercase().replace(' ', "-"));
  ServerEntry {
    stored: StoredServer {
      id: ServerId::generate(),
      url: url.clone(),
      name: name.to_owned(),
      plain_http_allowed: false,
      certificate_pin: None,
      added_at: Timestamp::from_unix_millis(1),
      joined_with: None,
      account: connection.map(|_| ServerAccount {
        user: UserId::generate(),
        username: "ivan".into(),
        display_name: "Ivan Zaida".into(),
        admin: false,
        signed_in_at: Timestamp::from_unix_millis(1),
      }),
    },
    address: ServerAddress::parse(&url).expect("address"),
    connection,
    last_reached: None,
    certificate_changed: false,
  }
}

fn shown(servers: ServerFacts) -> String {
  let mut bar = Bar::of(BarFacts { servers, ..facts(None) });
  bar
    .texts("status-server")
    .map(|texts| texts.join(" "))
    .unwrap_or_default()
}

#[test]
fn status_bar_server_item_follows_the_connection() {
  let offline = Connection::Offline {
    since: Timestamp::from_unix_millis(1),
    next: Instant::now(),
  };
  let reconnecting = Connection::Reconnecting {
    attempt: 3,
    next: Instant::now(),
  };
  for (connection, words, state) in [
    (Some(Connection::Connected), "", ServerState::Connected),
    (Some(reconnecting), "Kontur team · connecting", ServerState::Connecting),
    (Some(offline), "Kontur team offline", ServerState::Offline),
    (
      Some(Connection::SignedOut),
      "Kontur team · signed out",
      ServerState::SignedOut,
    ),
  ] {
    let servers = ServerFacts::of(&[entry("Kontur team", connection)], None);
    assert!(matches!(servers, ServerFacts::One { state: shown_state, .. } if shown_state == state));
    assert_eq!(shown(servers), words, "connected is quiet");
  }
  assert_eq!(shown(ServerFacts::of(&[], None)), "", "no server, no item");
}

#[test]
fn status_bar_server_item_names_the_projects_server_or_counts_two() {
  let kontur = entry("Kontur team", Some(Connection::SignedOut));
  let lab = entry("Lab", Some(Connection::Connected));
  let both = ServerFacts::of(&[kontur.clone(), lab.clone()], None);
  assert_eq!(
    both,
    ServerFacts::Several {
      count: 2,
      worst: ServerState::SignedOut
    }
  );
  assert_eq!(shown(both), "2 servers · one signed out");
  let linked = ServerFacts::of(&[kontur.clone(), lab], Some(kontur.stored.id));
  assert_eq!(shown(linked.clone()), "Kontur team · signed out");
  let mut bar = Bar::of(BarFacts {
    servers: linked,
    ..facts(None)
  });
  bar.press("status-server");
  assert_eq!(
    bar.asked(),
    vec![BarAction::Go(
      Route::Orchester(OrchesterView::server(kontur.stored.id)),
      OpenIn::ActiveTab
    )]
  );
}
