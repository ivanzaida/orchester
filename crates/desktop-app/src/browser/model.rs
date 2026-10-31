//! What the tab strip shows of a project's browser (`docs/design/browser.md`, "Tabs"; notes `RggGn`): one flat
//! strip, agents' tabs first in the order they opened with the tabs of one task next to each other, then the person's
//! own; and the one dot on each agent's tab, by precedence *waits for you* (amber) › *acting* (blue) › *new* (white).
//! Pure: the window hands in what it knows and the time.

use std::{
  collections::{HashMap, HashSet},
  sync::Arc,
  time::{Duration, Instant},
};

use orchester_core::{id::RunId, run::RunNumber, time::Timestamp};
use orchester_ipc::browser::{BrowserTab, BrowserTabId, TabOwner};
use orchester_workspace::browser::{BrowserView, RunOwner};

/// How long the window does not follow an agent to its new tab after the person used the window.
pub const LEFT_ALONE_AFTER: Duration = Duration::from_secs(10);
/// How long an agent's last action on a tab keeps it *acting*.
pub const ACTING_FOR: Duration = Duration::from_secs(10);

/// The dot on an agent's tab, strongest first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Indicator {
  /// Amber: its run waits for the person, or its agent waits because the person uses the tab, as the runner says.
  Waits,
  /// Blue: its agent acted on it a moment ago.
  Acting,
  /// White: an agent opened it and the person has not looked at it yet.
  New,
}

impl Indicator {
  pub const fn name(self) -> &'static str {
    match self {
      Self::Waits => "waits",
      Self::Acting => "acting",
      Self::New => "new",
    }
  }
}

/// Whose a tab of the strip is, as its label and the address line name it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Owner {
  /// A run's, by its task key and number; `None` until the run is read.
  Run {
    run: RunId,
    known: Option<KnownRun>,
  },
  Person,
}

/// A run as the window names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnownRun {
  pub key: Arc<str>,
  pub number: RunNumber,
  pub agent: Option<Arc<str>>,
}

/// One tab as the strip shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StripTab {
  pub id: BrowserTabId,
  pub owner: Owner,
  pub title: String,
  pub url: String,
  pub loading: bool,
  pub indicator: Option<Indicator>,
}

impl StripTab {
  pub const fn is_person(&self) -> bool {
    matches!(self.owner, Owner::Person)
  }

  /// Its task key, for an agent's tab whose run is known.
  pub fn key(&self) -> Option<&Arc<str>> {
    match &self.owner {
      Owner::Run { known: Some(known), .. } => Some(&known.key),
      _ => None,
    }
  }
}

/// What the window keeps of the person's own use of a project's tabs: the tabs looked at, and the last input on each.
/// Whether an agent waits on a tab is the runner's to say ([`BrowserView::waiting`]), not judged from these.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Marks {
  seen: HashSet<BrowserTabId>,
  input: HashMap<BrowserTabId, Instant>,
}

impl Marks {
  /// The person looked at `tab`: it is no longer *new*.
  pub fn seen(&mut self, tab: &BrowserTabId) {
    self.seen.insert(tab.clone());
  }

  /// The person's input reached `tab` `at`.
  pub fn used(&mut self, tab: &BrowserTabId, at: Instant) {
    self.input.insert(tab.clone(), at);
  }

  /// When the person last used any tab, to decide whether the window follows an agent to its new tab.
  pub fn last_input(&self) -> Option<Instant> {
    self.input.values().max().copied()
  }
}

/// How many whole seconds are left of an agent's wait until `until`, at `now`, as the 02b line says it: rounded up, and
/// at least one, since the wait goes on until the runner says it ended.
pub fn seconds_left(until: Timestamp, now: Timestamp) -> u64 {
  let left = u64::try_from(until.unix_millis().saturating_sub(now.unix_millis())).unwrap_or(0);
  left.div_ceil(1000).max(1)
}

/// The strip of `view`: agents' tabs, then the person's. `owners` names the runs read so far.
pub fn strip(view: &BrowserView, owners: &HashMap<RunId, RunOwner>, marks: &Marks, now: Instant) -> Strip {
  let mut agents: Vec<(Option<Arc<str>>, StripTab)> = Vec::new();
  let mut person = Vec::new();
  for tab in view.tabs() {
    let shown = strip_tab(view, tab, owners, marks, now);
    match tab.owner {
      TabOwner::Person => person.push(shown),
      TabOwner::Run { .. } => agents.push((shown.key().cloned(), shown)),
    }
  }
  Strip {
    agents: grouped(agents),
    person,
  }
}

/// The strip's tabs in order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Strip {
  pub agents: Vec<StripTab>,
  pub person: Vec<StripTab>,
}

impl Strip {
  /// Every tab, agents' first.
  pub fn all(&self) -> impl Iterator<Item = &StripTab> {
    self.agents.iter().chain(&self.person)
  }

  pub fn len(&self) -> usize {
    self.agents.len() + self.person.len()
  }

  pub fn is_empty(&self) -> bool {
    self.len() == 0
  }

  pub fn find(&self, tab: &BrowserTabId) -> Option<&StripTab> {
    self.all().find(|shown| &shown.id == tab)
  }

  /// The strongest dot among the tabs at `indexes` of [`Self::all`], for the overflow button.
  pub fn strongest(&self, indexes: &[usize]) -> Option<Indicator> {
    self
      .all()
      .enumerate()
      .filter(|(index, _)| indexes.contains(index))
      .filter_map(|(_, tab)| tab.indicator)
      .min()
  }
}

fn strip_tab(
  view: &BrowserView,
  tab: &BrowserTab,
  owners: &HashMap<RunId, RunOwner>,
  marks: &Marks,
  now: Instant,
) -> StripTab {
  let owner = match tab.owner {
    TabOwner::Person => Owner::Person,
    TabOwner::Run { run } => Owner::Run {
      run,
      known: owners.get(&run).map(|owner| KnownRun {
        key: owner.key.clone(),
        number: owner.number,
        agent: owner.agent.clone(),
      }),
    },
  };
  let indicator = match tab.owner {
    TabOwner::Person => None,
    TabOwner::Run { run } => indicator(view, tab, owners.get(&run), marks, now),
  };
  StripTab {
    id: tab.id.clone(),
    owner,
    title: tab.title.clone(),
    url: tab.url.clone(),
    loading: tab.loading,
    indicator,
  }
}

/// The dot of an agent's `tab`, by precedence.
fn indicator(
  view: &BrowserView,
  tab: &BrowserTab,
  owner: Option<&RunOwner>,
  marks: &Marks,
  now: Instant,
) -> Option<Indicator> {
  if owner.is_some_and(|owner| owner.waits) || view.waiting(&tab.id).is_some() {
    return Some(Indicator::Waits);
  }
  let acting = view
    .acting(&tab.id)
    .is_some_and(|acting| now.saturating_duration_since(acting.at) < ACTING_FOR);
  if acting {
    return Some(Indicator::Acting);
  }
  (!marks.seen.contains(&tab.id) && view.shown() != Some(&tab.id)).then_some(Indicator::New)
}

/// Agents' tabs in the order they opened, each task's tabs moved up beside its first one. A tab whose run is not read
/// yet keeps its own place.
fn grouped(tabs: Vec<(Option<Arc<str>>, StripTab)>) -> Vec<StripTab> {
  let groups: Vec<usize> = tabs
    .iter()
    .enumerate()
    .map(|(index, (key, _))| match key {
      Some(key) => tabs
        .iter()
        .position(|(other, _)| other.as_ref() == Some(key))
        .unwrap_or(index),
      None => index,
    })
    .collect();
  let mut ranked: Vec<(usize, usize, StripTab)> = tabs
    .into_iter()
    .zip(groups)
    .enumerate()
    .map(|(index, ((_, tab), group))| (group, index, tab))
    .collect();
  ranked.sort_by_key(|(group, index, _)| (*group, *index));
  ranked.into_iter().map(|(_, _, tab)| tab).collect()
}

/// The tab the window shows: `wanted`, asked for by the person or a request, while it is open; else the tab `shown`,
/// unless an agent of its run just `opened` a tab, which the window follows only while the person has not used it for
/// [`LEFT_ALONE_AFTER`]; else the tab shown `last`; else the first.
pub fn tab_to_show(
  strip: &Strip,
  (wanted, shown, last): (Option<&BrowserTabId>, Option<&BrowserTabId>, Option<&BrowserTabId>),
  (opened, marks, now): (&[BrowserTabId], &Marks, Instant),
) -> Option<BrowserTabId> {
  let open = |tab: Option<&BrowserTabId>| tab.filter(|tab| strip.find(tab).is_some()).cloned();
  if let Some(wanted) = open(wanted) {
    return Some(wanted);
  }
  if let Some(shown) = open(shown) {
    let idle = marks
      .last_input()
      .is_none_or(|at| now.saturating_duration_since(at) >= LEFT_ALONE_AFTER);
    let run = match strip.find(&shown).map(|tab| &tab.owner) {
      Some(Owner::Run { run, .. }) => Some(*run),
      _ => None,
    };
    let followed = run.filter(|_| idle).and_then(|run| {
      strip
        .agents
        .iter()
        .rev()
        .find(|tab| opened.contains(&tab.id) && matches!(tab.owner, Owner::Run { run: of, .. } if of == run))
    });
    return Some(followed.map_or(shown, |tab| tab.id.clone()));
  }
  open(last).or_else(|| strip.all().next().map(|tab| tab.id.clone()))
}

/// What a run's *Browser* line on its Summary says (`10 · Run · Browser line` `RGeKO`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunTabs {
  pub count: usize,
  /// Their hosts, each once.
  pub hosts: Vec<String>,
  /// The run's first tab, which *Show* opens the window on.
  pub first: BrowserTabId,
}

/// The tabs of `run` among `view`'s; `None` while it has none, and the line is not shown.
pub fn run_tabs(view: &BrowserView, run: RunId) -> Option<RunTabs> {
  let tabs: Vec<&BrowserTab> = view
    .tabs()
    .iter()
    .filter(|tab| tab.owner == TabOwner::Run { run })
    .collect();
  let first = tabs.first()?.id.clone();
  Some(RunTabs {
    count: tabs.len(),
    hosts: hosts(tabs.iter().map(|tab| tab.url.as_str())),
    first,
  })
}

/// The hosts of `tabs`' addresses, each once, in their order: the run line's *localhost:5173, docs.rs*.
pub fn hosts<'a>(tabs: impl IntoIterator<Item = &'a str>) -> Vec<String> {
  let mut hosts: Vec<String> = Vec::new();
  for address in tabs {
    let host = host(address);
    if !host.is_empty() && !hosts.contains(&host) {
      hosts.push(host);
    }
  }
  hosts
}

/// The host and port of `address`, as the address line and the run line show it; the address itself when it has no
/// host (`about:blank`).
pub fn host(address: &str) -> String {
  match url::Url::parse(address) {
    Ok(url) => match (url.host_str(), url.port()) {
      (Some(host), Some(port)) => format!("{host}:{port}"),
      (Some(host), None) => host.to_owned(),
      (None, _) => address.to_owned(),
    },
    Err(_) => address.to_owned(),
  }
}

/// `address` as the address line shows it: without its scheme for a web page (`localhost:5173/endpoints`).
pub fn shown_address(address: &str) -> &str {
  address
    .strip_prefix("https://")
    .or_else(|| address.strip_prefix("http://"))
    .unwrap_or(address)
}

/// What the person typed in the address line as an address to open: as typed with a scheme, else `http://` for a
/// local host and `https://` for any other.
pub fn typed_address(typed: &str) -> Option<String> {
  let typed = typed.trim();
  if typed.is_empty() {
    return None;
  }
  if typed.contains("://") || typed.starts_with("about:") {
    return Some(typed.to_owned());
  }
  // An IPv6 address is bracketed and holds colons of its own (`[::1]:8080`).
  let host = if typed.starts_with('[') {
    typed.split_inclusive(']').next().unwrap_or_default()
  } else {
    typed.split(['/', ':']).next().unwrap_or_default()
  };
  let local = host == "localhost" || host == "127.0.0.1" || host == "[::1]" || host.ends_with(".localhost");
  Some(format!("{}://{typed}", if local { "http" } else { "https" }))
}

/// The tab after `shown` in the strip's order (before it, unless `forward`), around the ends: Ctrl+Tab and
/// Ctrl+Shift+Tab. The first (or last) when none is shown; `None` with no tab.
pub fn cycled(strip: &Strip, shown: Option<&BrowserTabId>, forward: bool) -> Option<BrowserTabId> {
  let tabs: Vec<&StripTab> = strip.all().collect();
  let count = tabs.len();
  let at = shown.and_then(|shown| tabs.iter().position(|tab| &tab.id == shown));
  let next = match (at, forward) {
    (Some(at), true) => (at + 1) % count,
    (Some(at), false) => (at + count - 1) % count,
    (None, true) => 0,
    (None, false) => count.checked_sub(1)?,
  };
  tabs.get(next).map(|tab| tab.id.clone())
}
