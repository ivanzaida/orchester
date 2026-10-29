//! The strip's tabs (`docs/design/browser.md`, "Tabs"): agents' tabs first in the order they opened with one task's
//! tabs together, then the person's; one dot by precedence *waits for you* › *acting* › *new*; the tab the window
//! shows, which follows an agent to its new tab only while the person leaves the window alone; and the words the
//! address line and the run line take from the tabs' addresses.

use std::{
  collections::HashMap,
  num::NonZeroU32,
  path::Path,
  sync::Arc,
  time::{Duration, Instant},
};

use orchester_app::browser::model::{
  Indicator, Marks, Owner, PERSON_HOLD, host, hosts, shown_address, strip, tab_to_show, typed_address,
};
use orchester_core::{id::RunId, run::RunNumber};
use orchester_ipc::browser::{
  AgentAction, BrowserEvent, BrowserMode, BrowserState, BrowserTab, BrowserTabId, TabOwner,
};
use orchester_workspace::browser::{BrowserView, BrowserWindow, RunOwner};

const ROOT: &str = "/p/kontur";

fn tab(id: &str, owner: TabOwner) -> BrowserTab {
  BrowserTab {
    id: BrowserTabId::new(id),
    owner,
    title: format!("Page {id}"),
    url: format!("http://localhost:5173/{id}"),
    loading: false,
  }
}

fn id(tab: &str) -> BrowserTabId {
  BrowserTabId::new(tab)
}

/// The view of a project whose browser holds `tabs`, `shown` shown, after `events`.
fn view(tabs: Vec<BrowserTab>, shown: Option<&str>, events: Vec<BrowserEvent>) -> BrowserView {
  let window = BrowserWindow::default();
  let root = Path::new(ROOT);
  window.receive_for_test(
    root,
    "Kontur",
    BrowserEvent::Changed(BrowserState {
      tabs,
      shown: shown.map(id),
      mode: BrowserMode::Headless,
    }),
  );
  for event in events {
    window.receive_for_test(root, "Kontur", event);
  }
  window.view(root).expect("the project's browser")
}

fn owner(key: &str, number: u32, waits: bool) -> RunOwner {
  RunOwner {
    key: Arc::from(key),
    number: RunNumber::new(NonZeroU32::new(number).expect("a run number")),
    agent: Some(Arc::from("Builder")),
    waits,
  }
}

#[test]
fn browser_strip_puts_agents_tabs_first_with_each_tasks_tabs_together() {
  let (orc_107, orc_104, other_107) = (RunId::generate(), RunId::generate(), RunId::generate());
  let tabs = vec![
    tab("A", TabOwner::Person),
    tab("B", TabOwner::Run { run: orc_107 }),
    tab("C", TabOwner::Run { run: orc_104 }),
    tab("D", TabOwner::Run { run: other_107 }),
    tab("E", TabOwner::Person),
  ];
  let owners = HashMap::from([
    (orc_107, owner("ORC-107", 102, false)),
    (orc_104, owner("ORC-104", 98, false)),
    (other_107, owner("ORC-107", 103, false)),
  ]);
  let shown = strip(
    &view(tabs, Some("B"), Vec::new()),
    &owners,
    &Marks::default(),
    Instant::now(),
  );
  let order: Vec<&str> = shown.all().map(|tab| tab.id.as_str()).collect();
  assert_eq!(
    order,
    ["B", "D", "C", "A", "E"],
    "ORC-107's tabs together, the person's last"
  );
  assert_eq!(shown.agents.len(), 3);
  assert_eq!(shown.agents[0].key().map(|key| key.as_ref()), Some("ORC-107"));
  assert!(matches!(shown.person[0].owner, Owner::Person));
}

#[test]
fn browser_dots_go_by_waits_then_acting_then_new() {
  let (waiting, acting, quiet) = (RunId::generate(), RunId::generate(), RunId::generate());
  let tabs = vec![
    tab("W", TabOwner::Run { run: waiting }),
    tab("A", TabOwner::Run { run: acting }),
    tab("N", TabOwner::Run { run: quiet }),
    tab("S", TabOwner::Run { run: quiet }),
    tab("P", TabOwner::Person),
  ];
  let acted = BrowserEvent::AgentActed {
    run: waiting,
    tab: id("W"),
    action: AgentAction {
      tool: "browser_click".to_owned(),
      summary: "clicked Save".to_owned(),
      element: None,
    },
  };
  let acted_too = BrowserEvent::AgentActed {
    run: acting,
    tab: id("A"),
    action: AgentAction {
      tool: "browser_type".to_owned(),
      summary: "typed 8 in Max attempts".to_owned(),
      element: None,
    },
  };
  let owners = HashMap::from([
    (waiting, owner("ORC-1", 1, true)),
    (acting, owner("ORC-2", 2, false)),
    (quiet, owner("ORC-3", 3, false)),
  ]);
  let mut marks = Marks::default();
  marks.seen(&id("S"));
  let view = view(tabs, Some("P"), vec![acted, acted_too]);
  let dots: Vec<Option<Indicator>> = strip(&view, &owners, &marks, Instant::now())
    .all()
    .map(|tab| tab.indicator)
    .collect();
  assert_eq!(
    dots,
    [
      Some(Indicator::Waits),
      Some(Indicator::Acting),
      Some(Indicator::New),
      None,
      None
    ],
    "waits over acting; new until looked at; the person's tab has none"
  );
  let later = Instant::now() + Duration::from_secs(60);
  let quiet_dots: Vec<Option<Indicator>> = strip(&view, &owners, &marks, later)
    .all()
    .map(|tab| tab.indicator)
    .collect();
  assert_eq!(
    quiet_dots[1],
    Some(Indicator::New),
    "acting ends a while after the action: A was never looked at"
  );
}

#[test]
fn browser_person_input_makes_the_tab_wait_for_a_while() {
  let run = RunId::generate();
  let view = view(vec![tab("T", TabOwner::Run { run })], Some("T"), Vec::new());
  let owners = HashMap::from([(run, owner("ORC-9", 9, false))]);
  let now = Instant::now();
  let mut marks = Marks::default();
  marks.seen(&id("T"));
  marks.used(&id("T"), now);
  assert!(marks.holds(&id("T"), now));
  assert_eq!(
    strip(&view, &owners, &marks, now).agents[0].indicator,
    Some(Indicator::Waits)
  );
  let after = now + PERSON_HOLD + Duration::from_millis(1);
  assert!(!marks.holds(&id("T"), after));
  assert_eq!(strip(&view, &owners, &marks, after).agents[0].indicator, None);
}

#[test]
fn browser_shows_the_tab_asked_for_and_follows_an_agent_only_while_left_alone() {
  let run = RunId::generate();
  let tabs = vec![
    tab("A", TabOwner::Run { run }),
    tab("B", TabOwner::Run { run }),
    tab("P", TabOwner::Person),
  ];
  let view = view(tabs, Some("A"), Vec::new());
  let owners = HashMap::from([(run, owner("ORC-5", 5, false))]);
  let now = Instant::now();
  let mut marks = Marks::default();
  marks.seen(&id("A"));
  let shown = strip(&view, &owners, &marks, now);
  let opened = [id("B")];
  assert_eq!(
    tab_to_show(&shown, (Some(&id("P")), Some(&id("A")), None), (&opened, &marks, now)),
    Some(id("P")),
    "a tab asked for wins"
  );
  assert_eq!(
    tab_to_show(&shown, (None, Some(&id("A")), None), (&opened, &marks, now)),
    Some(id("B")),
    "left alone, the window follows the agent to the tab its run just opened"
  );
  assert_eq!(
    tab_to_show(&shown, (None, Some(&id("A")), None), (&[], &marks, now)),
    Some(id("A")),
    "a tab that was there before, looked at or not, does not pull the window"
  );
  marks.used(&id("A"), now);
  let shown = strip(&view, &owners, &marks, now);
  assert_eq!(
    tab_to_show(&shown, (None, Some(&id("A")), None), (&opened, &marks, now)),
    Some(id("A")),
    "while the person uses the window it stays"
  );
  assert_eq!(
    tab_to_show(&shown, (Some(&id("gone")), None, Some(&id("B"))), (&[], &marks, now)),
    Some(id("B")),
    "a tab that closed is not shown; the one shown last is"
  );
}

#[test]
fn browser_addresses_read_as_the_address_line_and_the_run_line_say_them() {
  assert_eq!(host("http://localhost:5173/endpoints/orders-sync"), "localhost:5173");
  assert_eq!(host("https://docs.rs/reqwest"), "docs.rs");
  assert_eq!(host("about:blank"), "about:blank");
  assert_eq!(
    hosts([
      "http://localhost:5173/a",
      "https://docs.rs/x",
      "http://localhost:5173/b"
    ]),
    ["localhost:5173", "docs.rs"]
  );
  assert_eq!(
    shown_address("http://localhost:5173/endpoints"),
    "localhost:5173/endpoints"
  );
  assert_eq!(
    typed_address("localhost:5173/x").as_deref(),
    Some("http://localhost:5173/x")
  );
  assert_eq!(typed_address("docs.rs").as_deref(), Some("https://docs.rs"));
  assert_eq!(typed_address("https://a.test").as_deref(), Some("https://a.test"));
  assert_eq!(typed_address("   "), None);
}
