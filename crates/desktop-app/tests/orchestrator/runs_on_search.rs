//! The runs-on picker's Search and keys (`Many models` `pGj8M`, `Searching` `Ul1ay` and the notes `Hxq6v` in
//! `Runs-on picker · states` `t7cdi`): Search filters the chosen connection's models and says when none matches; more
//! than four scroll inside their list, the chosen one scrolled into view when the picker opens, and the picker keeps its
//! height; Tab moves the keys between the rail, the models and the efforts, ← → along the rail and the efforts, ↑ ↓
//! within the models (keeping the place in view), Enter chooses and applies at once, a key typed elsewhere in the
//! picker goes to Search, and Esc closes the picker.

use orchester_app::routes::OrchestratorChat;
use orchester_core::{
  connection::{AgentProvider, CatalogModel, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn},
  id::ConnectionId,
  orchestrator::OrchestratorSettings,
  resource::ModelId,
  store::ConnectionStore,
  time::Timestamp,
};

use super::{
  connections,
  harness::{Orchestrator, Setup},
};

/// The picker's element ids start with this.
const MENU: &str = "orchestrator-runs-on-menu";

/// The models of the many-models connection (`pGj8M`), each with the same efforts.
const MODELS: [&str; 12] = [
  "gpt-6-astra",
  "gpt-6.1-sol",
  "gpt-6.1-terra",
  "gpt-6-sol",
  "gpt-6-luna",
  "gpt-5.6-sol",
  "gpt-5.6-terra",
  "gpt-5.6-luna",
  "gpt-5.5",
  "gpt-5.5-mini",
  "gpt-5.4",
  "gpt-5.4-mini",
];

/// A Codex connection that lists the twelve [`MODELS`], published as what the Orchestrator runs on with the last.
fn many_models(screen: &Orchestrator) -> ConnectionId {
  let program = std::env::current_exe().expect("a program");
  let mut connection = connections::cli(AgentProvider::CodexCli, "Codex · Many", &program);
  let efforts = || ["low", "medium", "high"].map(str::to_owned).to_vec();
  let models = MODELS
    .iter()
    .map(|id| CatalogModel::new(ModelId::new(id).expect("model"), efforts()))
    .collect();
  connection
    .record_check(ConnectionCheck {
      at: Timestamp::now(),
      result: CheckResult::Available {
        version: "0.130.0".into(),
        sign_in: SignIn::SignedIn,
        catalog: ModelCatalog::new(CatalogSource::Provider, models),
      },
    })
    .expect("its check");
  let mut db = screen.db();
  db.create_connection(&connection).expect("created");
  let settings = OrchestratorSettings::new(connection.id(), ModelId::new(MODELS[11]).expect("model"), None);
  db.publish_orchestrator(&settings, Timestamp::now()).expect("published");
  connection.id()
}

/// The models the list shows, in order.
fn listed(screen: &mut Orchestrator) -> Vec<String> {
  (0..)
    .map(|index| format!("{MENU}-model-{index}"))
    .map_while(|id| screen.exists(&id).then(|| screen.text(&id)))
    .collect()
}

fn published(screen: &Orchestrator) -> Option<(u32, OrchestratorSettings)> {
  screen.db().orchestrator_published().expect("read")
}

fn open(screen: &mut Orchestrator) {
  screen.go(OrchestratorChat::New);
  screen.click("orchestrator-runs-on-chip");
  screen.until("the picker", |screen| screen.exists(&format!("{MENU}-search")));
}

/// The place `index` of the models' list shows whole inside it.
fn in_view(screen: &mut Orchestrator, index: usize) -> bool {
  let (list, row) = (
    screen.bounds(&format!("{MENU}-model-scroll")),
    screen.bounds(&format!("{MENU}-model-{index}")),
  );
  row.y >= list.y - 0.5 && row.y + row.height <= list.y + list.height + 0.5
}

#[test]
fn orchestrator_picker_scrolls_many_models_in_their_list_and_search_filters_them() {
  let mut screen = Orchestrator::new(Setup::Ready);
  many_models(&screen);
  open(&mut screen);
  // Every model is in the list, which is four rows high (`BYfVi`) and scrolled to the chosen, the last.
  assert_eq!(listed(&mut screen), MODELS.to_vec());
  assert!(!screen.exists(&format!("{MENU}-show-all")));
  let list = screen.bounds(&format!("{MENU}-model-scroll"));
  assert!((list.height - 112.0).abs() < 0.5, "four rows high: {list:?}");
  screen.until("the chosen model in view", |screen| in_view(screen, 11));
  let menu = screen.bounds(MENU);
  // ↑ shows the keys on the chosen, then moves them to the first, which scrolls into view; the picker keeps its height.
  for _ in 0..12 {
    screen.key(("ArrowUp", "ArrowUp"), false);
  }
  screen.until("the first model in view", |screen| in_view(screen, 0));
  assert!(!in_view(&mut screen, 11), "the list scrolled");
  let after = screen.bounds(MENU);
  assert!(
    (after.height - menu.height).abs() < 0.5 && (after.y - menu.y).abs() < 0.5,
    "the picker stays: {menu:?} {after:?}"
  );

  screen.set(&format!("{MENU}-search"), "sol");
  screen.until("the models found", |screen| listed(screen).len() == 3);
  assert_eq!(listed(&mut screen), ["gpt-6.1-sol", "gpt-6-sol", "gpt-5.6-sol"]);
  let found = screen.bounds(MENU);
  assert!(
    (found.height - menu.height).abs() < 0.5,
    "the list keeps its height while it finds fewer: {menu:?} {found:?}"
  );
  screen.set(&format!("{MENU}-search"), "zzz");
  screen.until("nothing found", |screen| screen.exists(&format!("{MENU}-no-models")));
  assert_eq!(screen.text(&format!("{MENU}-no-models")), "No model matches");
  assert!(
    !screen.exists(&format!("{MENU}-typed")),
    "a CLI takes only the models it lists"
  );
}

#[test]
fn orchestrator_picker_keys_move_between_areas_and_choose() {
  let mut screen = Orchestrator::new(Setup::Ready);
  open(&mut screen);
  // Search has the focus: ↓ shows where the keys are, on the chosen model, ↓ again the next, Enter takes it.
  screen.key(("ArrowDown", "ArrowDown"), false);
  screen.key(("ArrowDown", "ArrowDown"), false);
  screen.key(("Enter", "Enter"), false);
  let opus = OrchestratorSettings::new(screen.claude, ModelId::new("opus").expect("model"), None);
  screen.until("opus published", |screen| published(screen) == Some((2, opus.clone())));
  screen.until("opus's efforts", |screen| {
    screen.exists(&format!("{MENU}-effort-low"))
  });
  // Tab to the efforts, on Default; → to Low; Enter.
  screen.key(("Tab", "Tab"), false);
  screen.key(("ArrowRight", "ArrowRight"), false);
  screen.key(("Enter", "Enter"), false);
  let low = OrchestratorSettings::new(
    screen.claude,
    ModelId::new("opus").expect("model"),
    Some("low".to_owned()),
  );
  screen.until("low published", |screen| published(screen) == Some((3, low.clone())));
  // Shift+Tab twice to the rail, on the connection shown; → to Codex; Enter runs on it.
  screen.shift_key(("Tab", "Tab"));
  screen.shift_key(("Tab", "Tab"));
  screen.key(("ArrowRight", "ArrowRight"), false);
  screen.key(("Enter", "Enter"), false);
  let codex = screen.codex;
  screen.until("Codex published", |screen| {
    published(screen).is_some_and(|(version, settings)| version == 4 && settings.connection() == codex)
  });
  screen.until("Codex's models", |screen| {
    screen.text(&format!("{MENU}-model-0")) == "gpt-5.5"
  });
  // Esc closes the picker.
  screen.key(("Escape", "Escape"), false);
  screen.until("the picker closed", |screen| !screen.exists(MENU));
}

#[test]
fn orchestrator_picker_filters_as_a_key_is_typed_anywhere_in_it() {
  let mut screen = Orchestrator::new(Setup::Ready);
  open(&mut screen);
  // Tab to the efforts, where the focus leaves Search; a letter typed there goes to Search and filters the models.
  screen.key(("Tab", "Tab"), false);
  screen.key(("o", "KeyO"), false);
  screen.until("the models found", |screen| {
    let found = listed(screen);
    !found.is_empty() && found.iter().all(|model| model.contains('o'))
  });
  assert!(listed(&mut screen).contains(&"opus".to_owned()), "opus holds it");
}
