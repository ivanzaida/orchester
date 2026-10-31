//! The tabs each QA state shows, after the board's frames: which runs own which pages, the tab shown, and what else
//! the runner reports (an agent's action, a dialog, a request for a file, a `<select>` list).

use orchester_core::{id::RunId, time::Timestamp};
use orchester_ipc::browser::{
  AgentAction, BrowserDialog, BrowserEvent, BrowserMode, BrowserState as Reported, BrowserTab, BrowserTabId,
  DialogKind, ElementBox, FileChooser, FileChooserId, SelectId, SelectList, SelectOption, TabOwner,
};

use super::BrowserState;

/// A page of the fixture: its title and address.
type Page = (&'static str, &'static str);

const ORDERS: Page = (
  "orders-sync · Kontur Admin",
  "http://localhost:5173/endpoints/orders-sync",
);
const REQWEST: Page = (
  "Client in reqwest - Rust",
  "https://docs.rs/reqwest/latest/reqwest/struct.Client.html",
);
const RETRY_AFTER: Page = (
  "Retry-After - HTTP | MDN",
  "https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Retry-After",
);
const DELIVERIES: Page = ("Deliveries · Kontur Admin", "http://localhost:5173/deliveries");
const PULLS: Page = ("Pull requests · GitHub", "https://github.com/kontur/kontur/pulls");
/// More pages, for the crowded and the overflowing strips.
const MORE: [Page; 12] = [
  (
    "Rate limits · Kontur API reference",
    "https://docs.kontur.test/api/rate-limits",
  ),
  ("Pricing · Stripe Docs", "https://docs.stripe.com/pricing"),
  ("Sign in · Kontur staging", "https://staging.kontur.test/sign-in"),
  (
    "Session cookies - HTTP | MDN",
    "https://developer.mozilla.org/en-US/docs/Web/HTTP/Cookies",
  ),
  (
    "Login redirect loop after session refresh",
    "https://github.com/kontur/kontur/issues/412",
  ),
  ("Settings · Kontur Admin", "http://localhost:5173/settings"),
  (
    "tokio::time::sleep - Rust",
    "https://docs.rs/tokio/latest/tokio/time/fn.sleep.html",
  ),
  ("Timeouts · Kontur Admin", "http://localhost:5173/settings/timeouts"),
  ("Dashboard · Kontur Admin", "http://localhost:5173/dashboard"),
  ("Charts · Kontur Admin", "http://localhost:5173/charts"),
  ("Chart.js | Open source HTML5 Charts", "https://www.chartjs.org/"),
  ("Colors · Kontur design", "https://design.kontur.test/colors"),
];

/// The page's `Max attempts` field and its `Retry policy` select, in the page's CSS pixels (`browser_page.html`).
pub const MAX_ATTEMPTS: ElementBox = ElementBox {
  x: 55,
  y: 377,
  width: 268,
  height: 34,
};
pub const RETRY_POLICY: ElementBox = ElementBox {
  x: 55,
  y: 303,
  width: 552,
  height: 34,
};

/// When an agent's wait ends if the person's last input is now, as the runner reports it.
pub fn until_after_input() -> Timestamp {
  Timestamp::from_unix_millis(Timestamp::now().unix_millis() + 10_000)
}

/// What a QA state reports.
pub struct Scene {
  pub reported: Reported,
  /// Reported once after the state, and again while the window is open.
  pub events: Vec<BrowserEvent>,
  /// What the page's `Max attempts` field holds.
  pub attempts: u32,
  /// The page's *Signing certificate* field, shown only where a state asks for a file: the file chosen in it, empty
  /// for none (`03b` `TWpyp`, `03c` `x0N8d`).
  pub certificate: Option<&'static str>,
}

impl Scene {
  /// The sample page's query (`browser_page.html`): what its fields hold.
  pub fn query(&self) -> String {
    match self.certificate {
      Some(file) => format!("attempts={}&certificate={file}", self.attempts),
      None => format!("attempts={}", self.attempts),
    }
  }
}

/// The file the person chose for the page's *Signing certificate* field (`03c` `x0N8d`).
const CERTIFICATE: &str = "kontur-hooks.pem";

/// The scene of `state`, its agents' tabs owned by `runs` in turn. The window has looked at no agent's tab but the
/// one it shows, so the others are *new* until the person shows them.
pub fn scene(state: BrowserState, runs: &[RunId]) -> Scene {
  let run = |index: usize| TabOwner::Run {
    run: runs[index % runs.len().max(1)],
  };
  let mut tabs = Tabs::default();
  if runs.is_empty() || matches!(state, BrowserState::Empty | BrowserState::NotFound) {
    return tabs.scene(None, 5);
  }
  let shown = tabs.add(run(0), ORDERS);
  tabs.add(run(0), REQWEST);
  if state != BrowserState::Acting {
    tabs.add(run(0), RETRY_AFTER);
  }
  let deliveries = tabs.add(run(1), DELIVERIES);
  // `01c` `fPkxD` has nine tabs, crowded but all in view; `01d` `fWhIr` twenty.
  let extra = match state {
    BrowserState::Many => 3,
    BrowserState::Overflow => 14,
    _ => 0,
  };
  let more: Vec<BrowserTabId> = (0..extra)
    .map(|index| tabs.add(run(2 + index / 3), MORE[index % MORE.len()]))
    .collect();
  let yours = tabs.add(TabOwner::Person, ORDERS);
  tabs.add(TabOwner::Person, PULLS);
  let acting = |tab: BrowserTabId, summary: &str, element: Option<ElementBox>| BrowserEvent::AgentActed {
    run: runs[0],
    tab,
    action: AgentAction {
      tool: "browser_type".to_owned(),
      summary: summary.to_owned(),
      element,
    },
  };
  let (shown, events, attempts) = match state {
    BrowserState::Acting => (
      shown.clone(),
      vec![acting(shown, "typed 8 in Max attempts", Some(MAX_ATTEMPTS))],
      8,
    ),
    BrowserState::Waiting => (
      shown.clone(),
      vec![BrowserEvent::AgentWaiting {
        run: runs[0],
        tab: shown,
        until: until_after_input(),
      }],
      8,
    ),
    BrowserState::Dialog => (yours.clone(), vec![confirm(yours)], 5),
    BrowserState::FileChooser => (yours.clone(), vec![signing_certificate(yours)], 5),
    BrowserState::FileChosen => (yours, Vec::new(), 5),
    BrowserState::Select => (yours.clone(), vec![retry_policy(yours)], 5),
    // A tab in the middle of the strip, so it scrolls to show it (`01e` `cmofl`).
    BrowserState::Overflow => (
      more[more.len() / 2].clone(),
      vec![acting(deliveries, "clicked Retry", None)],
      5,
    ),
    _ => (shown, vec![acting(deliveries, "clicked Retry", None)], 5),
  };
  let mut scene = tabs.scene(Some(shown), attempts);
  if state == BrowserState::Headed {
    scene.reported.mode = BrowserMode::Headed;
  }
  scene.certificate = match state {
    BrowserState::FileChooser => Some(""),
    BrowserState::FileChosen => Some(CERTIFICATE),
    _ => None,
  };
  scene.events = events;
  scene
}

#[derive(Default)]
struct Tabs(Vec<BrowserTab>);

impl Tabs {
  fn add(&mut self, owner: TabOwner, (title, url): Page) -> BrowserTabId {
    let id = BrowserTabId::new(format!("qa-tab-{}", self.0.len() + 1));
    self.0.push(BrowserTab {
      id: id.clone(),
      owner,
      title: title.to_owned(),
      url: url.to_owned(),
      loading: false,
    });
    id
  }

  fn scene(self, shown: Option<BrowserTabId>, attempts: u32) -> Scene {
    Scene {
      reported: Reported {
        tabs: self.0,
        shown,
        mode: BrowserMode::Headless,
      },
      events: Vec::new(),
      attempts,
      certificate: None,
    }
  }
}

/// The page's confirm before deleting the endpoint (`03` `t08Ur`).
fn confirm(tab: BrowserTabId) -> BrowserEvent {
  BrowserEvent::DialogOpened {
    tab,
    dialog: BrowserDialog {
      kind: DialogKind::Confirm,
      message: "Delete orders-sync? Deliveries to this endpoint stop at once.".to_owned(),
      default_prompt: String::new(),
    },
  }
}

/// The page's *Signing certificate* field asks for one PEM file (`03b` `TWpyp`).
fn signing_certificate(tab: BrowserTabId) -> BrowserEvent {
  BrowserEvent::FileChooserOpened {
    tab,
    chooser: FileChooser {
      id: FileChooserId::new(1),
      multiple: false,
      accept: vec![".pem".to_owned()],
      label: Some("Signing certificate".to_owned()),
    },
  }
}

/// The page's `Retry policy` list, open (`04` `r31HX`).
fn retry_policy(tab: BrowserTabId) -> BrowserEvent {
  let options = ["None", "Fixed delay", "Exponential", "Exponential with jitter"]
    .into_iter()
    .map(|label| SelectOption {
      label: label.to_owned(),
      selected: label == "Exponential with jitter",
      disabled: false,
    })
    .collect();
  BrowserEvent::SelectOpened {
    tab,
    list: SelectList {
      select: SelectId::new(41),
      options,
      element: RETRY_POLICY,
    },
  }
}
