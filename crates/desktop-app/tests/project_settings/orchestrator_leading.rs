//! Orchestrator › *Leading to the end* (`S01 · Project settings · Orchestrator · leading defaults` `pK4S5`, board
//! `Orchestrator autonomy · frames to approve`; OA-10): the ceilings a new subscription that leads its target to the
//! end takes, four rows under *Watching*, saved as the person leaves a box and read back from the project; a value
//! out of bounds says why and saves nothing; only a person's input saves.
//! `cargo test -p orchester-app --features test-support --test project_settings orchestrator_leading`.
use lurq::app::events::with_synthetic_input;
use orchester_core::{orchestrator::Ceilings, tokens::TokenBudget};
use orchester_workspace::tasks::outside::Outside;

use super::harness::Settings;

const GROUP: &str = "settings-orchestrator-leading";
const RUNS: &str = "settings-orchestrator-leading-runs-field";
const RETURNS: &str = "settings-orchestrator-leading-returns-field";
const RAISES: &str = "settings-orchestrator-leading-raises-field";
const PERCENT: &str = "settings-orchestrator-leading-percent-field";
const BUDGET: &str = "settings-orchestrator-leading-budget-field";

/// Project settings on its Orchestrator tab, *Leading to the end* read.
fn leading() -> Settings {
  let mut page = Settings::new(false);
  page.tab("orchestrator", GROUP);
  page.until("the ceilings read", |p| p.exists(RUNS));
  page
}

fn stored(page: &Settings) -> Ceilings {
  page.db().lead_defaults().expect("the leading defaults")
}

/// Moves the focus to `id`, as clicking another box does.
fn focus(page: &mut Settings, id: &str) {
  page
    .window
    .tree
    .get_element_by_id_mut(id)
    .unwrap_or_else(|| panic!("{id} is shown"))
    .focus();
  page.window.pass();
}

fn error(row: &str) -> String {
  format!("{GROUP}-{row}-error")
}

#[test]
fn project_settings_orchestrator_leading_shows_four_ceilings_with_their_bounds() {
  let mut page = leading();
  assert!(page.text(&format!("{GROUP}-head")).contains("Leading to the end"));
  assert!(
    page
      .text(&format!("{GROUP}-where"))
      .contains("What a new leading subscription takes · /watching changes it for one")
  );
  for (row, label, hint) in [
    (
      "runs",
      "Runs of a task",
      "How many times it may run one task again before the task is parked for you · 1 to 5",
    ),
    (
      "returns",
      "Returns per run",
      "Returns to an earlier step inside one run · 0 to 10",
    ),
    (
      "raises",
      "Raises of a limit",
      "How often it may raise a run's token or time limit, and by how much · 0 to 3, up to 100 %",
    ),
    (
      "budget",
      "Budget",
      "Tokens for the whole target; nothing new starts once it is used · empty: no budget",
    ),
  ] {
    assert_eq!(page.text(&format!("{GROUP}-{row}-label")), label);
    assert_eq!(page.text(&format!("{GROUP}-{row}-hint")), hint);
  }
  let rows = page.text(GROUP);
  for unit in ["runs", "returns", "by", "%", "tokens", "None"] {
    assert!(rows.contains(unit), "{unit}: {rows}");
  }
  assert!(
    page
      .text(&format!("{GROUP}-foot"))
      .contains("A new project starts with 3 runs, 4 returns, 1 raise by 50 % and no budget."),
    "{}",
    page.text(&format!("{GROUP}-foot"))
  );
  // A new project: the built-in ceilings, and no budget.
  assert_eq!(
    [RUNS, RETURNS, RAISES, PERCENT, BUDGET].map(|id| page.value(id)),
    ["3", "4", "1", "50", ""]
  );
  assert_eq!(stored(&page), Ceilings::default());
}

#[test]
fn project_settings_orchestrator_leading_saves_as_a_box_is_left_and_refuses_what_is_out_of_bounds() {
  let mut page = leading();
  page.set(RUNS, "5");
  focus(&mut page, RETURNS);
  page.until("the runs saved", |p| stored(p).runs_of_task() == 5);

  // Out of bounds says why under its row and saves nothing; leaving puts the saved value back.
  page.set(RUNS, "8");
  page.until("why", |p| p.exists(&error("runs")));
  assert_eq!(page.text(&error("runs")), "A whole number, 1 to 5");
  focus(&mut page, RETURNS);
  page.until("the saved runs back", |p| p.value(RUNS) == "5");
  assert!(!page.exists(&error("runs")));
  assert_eq!(stored(&page).runs_of_task(), 5);

  page.set(PERCENT, "150");
  page.until("why", |p| p.exists(&error("raises")));
  assert_eq!(page.text(&error("raises")), "A whole number of percent, 1 to 100");
  focus(&mut page, RAISES);
  page.until("the saved share back", |p| p.value(PERCENT) == "50");

  page.set(BUDGET, "lots");
  page.until("why", |p| p.exists(&error("budget")));
  assert!(page.text(&error("budget")).contains("A whole number of tokens above 0"));
  focus(&mut page, RUNS);
  page.until("no budget back", |p| p.value(BUDGET).is_empty());
  assert_eq!(stored(&page).budget(), None);

  // The other three and a budget, typed as an absolute number and grouped as it is typed.
  page.set(RETURNS, "0");
  page.set(RAISES, "3");
  page.set(PERCENT, "100");
  page.set(BUDGET, "40000000");
  focus(&mut page, RUNS);
  let budget = TokenBudget::new(40_000_000).expect("a budget");
  let asked = Ceilings::new((5, 0), (3, 100), Some(budget)).expect("ceilings");
  page.until("all of them saved", |p| stored(p) == asked);
  assert_eq!(page.value(BUDGET), "40,000,000");
  page.saved();

  // Emptied, the budget is none again.
  page.set(BUDGET, "");
  focus(&mut page, RUNS);
  page.until("no budget saved", |p| stored(p).budget().is_none());
}

#[test]
fn project_settings_orchestrator_leading_loads_what_the_project_holds_and_takes_no_synthetic_input() {
  let mut page = leading();
  let set = Ceilings::new((2, 7), (0, 25), TokenBudget::new(1_500_000).ok()).expect("ceilings");
  page.db().save_lead_defaults(&set).expect("saved");
  // A change made elsewhere reads the ceilings again.
  page.outside(Outside::Orchestrator);
  page.until("the stored ceilings", |p| p.value(RUNS) == "2");
  assert_eq!(
    [RETURNS, RAISES, PERCENT, BUDGET].map(|id| page.value(id)),
    ["7", "0", "25", "1,500,000"]
  );

  // The ceilings are a person's: automation that reaches a box saves nothing.
  with_synthetic_input(|| {
    page.set(RUNS, "4");
    focus(&mut page, RETURNS);
  });
  for _ in 0..5 {
    page.window.tree.tick_futures();
    page.window.pass();
  }
  assert_eq!(stored(&page), set, "synthetic input saved nothing");
  // The person's own leave saves what the box holds.
  focus(&mut page, RUNS);
  focus(&mut page, RETURNS);
  page.until("the person's save", |p| stored(p).runs_of_task() == 4);
}
