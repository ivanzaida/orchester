//! What auto mode's reviewer spent, in the desktop: Usage's *Where tokens go* gives it rows of their own
//! among the profiles (*Auto mode reviewer* and the model in mono) and its totals count it, the invocation
//! panel of the Conversation's Details has a line for it, and `get_run` on Orchester's MCP server counts
//! it in the run's tokens with its part apart. At 960 a reviewer row's model stays whole beside its
//! figures, which stay in their column, and in the table the two reviewers' rows still read apart.
//! Seeded: a run whose agent reported a turn and two of whose requests reviewers of two models approved,
//! all today.
//! `cargo test -p orchester-desktop --test form_keyboard usage_reviewer`.

use lurq::{
  app::{App, Tree, component::Component, ctx::Ctx},
  components::{Column, Text},
  node::Element,
};
use orchester_core::{
  connection::{AgentProvider, stream::AgentEvent},
  id::RunId,
  resource::ModelId,
  run::{
    PendingToolApproval, TokenUsage,
    review::{ReviewEnd, ReviewRecord},
  },
  store::ActivityStore,
  time::Timestamp,
};
use orchester_desktop::theme::{self, ExtraTypography};
use orchester_store::ProjectDb;
use serde_json::json;

use crate::{
  app_mcp::{agent, ok},
  github::{contains_text, shows},
  harness::Desktop,
  narrow_answers::{details, open_conversation},
  native_run::native_run,
};

/// The two reviewers' models: the second spends more, so it is ranked first.
const MODELS: [&str; 2] = ["claude-haiku-4-5", "claude-sonnet-4-5"];
/// Layout positions are fractional; two equal ones may differ by rounding.
const SAME: f32 = 0.5;

fn usage(input: u64, cached_input: u64, output: u64) -> TokenUsage {
  TokenUsage {
    input,
    cached_input,
    output,
  }
}

/// A run whose agent spent 3,500 counted tokens today, and whose reviewers spent 1,050 (Haiku) and 2,100
/// (Sonnet) on two of its requests.
fn reviewed_run() -> (Desktop, RunId) {
  let mut seeded = None;
  let desktop = Desktop::with_seeded_project_and_app_mcp(|repo| {
    let (run, mut invocation) = native_run(repo);
    let mut db = ProjectDb::open(repo).expect("open");
    let now = Timestamp::now();
    invocation.usage = usage(4_000, 1_000, 500);
    db.save_invocation(&invocation).expect("the agent's usage");
    let turn = AgentEvent::TurnCompleted {
      usage: invocation.usage,
    };
    db.append_activity(&invocation, &[(now, turn)]).expect("its turn");
    let spent = [usage(1_200, 200, 50), usage(2_000, 0, 100)];
    for (number, (model, spent)) in MODELS.iter().zip(spent).enumerate() {
      let request = PendingToolApproval {
        id: format!("{}:call_{number}", invocation.id),
        run: run.id(),
        execution: invocation.execution,
        invocation: invocation.id,
        agent: "Builder".to_owned(),
        command: true,
        scope: None,
        server: "command".to_owned(),
        action: "cargo build".to_owned(),
        target: None,
        requested_at: now,
        native: None,
      };
      db.request_tool_approval(&request).expect("request");
      let review = ReviewRecord {
        run: run.id(),
        execution: invocation.execution,
        invocation: invocation.id,
        approval: request.id,
        provider: AgentProvider::ClaudeCodeCli,
        model: ModelId::new(model).expect("model"),
        started_at: now,
        ended_at: now,
        usage: Some(spent),
        end: ReviewEnd::Approved,
      };
      db.record_reviewer_usage(&review).expect("the review's cost");
    }
    seeded = Some(run.id());
  });
  (desktop, seeded.expect("seeded"))
}

fn has(desktop: &mut Desktop, id: &str) -> bool {
  desktop.tree.get_element_by_id(id).is_some()
}

#[test]
fn usage_reviewer_has_a_row_of_its_own_and_counts_in_the_totals_and_the_run() {
  let (mut desktop, run) = reviewed_run();
  open_conversation(&mut desktop);
  details(&mut desktop, true);
  desktop.wait_until("the reviewer's line", |desktop| {
    desktop
      .tree
      .get_element_by_id("activity-invocation")
      .is_some_and(|element| contains_text(element, "Auto mode reviewer"))
  });
  assert!(shows(&mut desktop, "activity-invocation", "3.2k tokens"));

  desktop.press("nav-usage");
  desktop.wait_until("the reviewers' rows", |desktop| {
    MODELS
      .iter()
      .all(|model| has(desktop, &format!("usage-breakdown-reviewer-{model}")))
  });
  assert!(shows(&mut desktop, "usage-breakdown", "Auto mode reviewer"));
  assert!(
    shows(
      &mut desktop,
      "usage-breakdown",
      "the auto mode reviewer’s rows only read"
    ),
    "the note does not offer to filter by them"
  );
  assert!(
    shows(&mut desktop, "usage-summary-spent", "6.7k"),
    "3,500 of the agent and 3,150 of the reviewers"
  );

  let mut client = agent(&desktop);
  let read = ok(
    &mut client,
    "get_run",
    &json!({"project": "Kontur", "run": run.to_string()}),
  );
  let tokens = &read["tokens"];
  assert_eq!(
    (&tokens["input"], &tokens["cached_input"], &tokens["output"]),
    (&json!(7_200), &json!(1_200), &json!(650)),
    "{tokens}"
  );
  assert_eq!(
    tokens["reviewer"],
    json!({"reviews": 2, "unknown": 0, "input": 3_200, "cached_input": 200, "output": 150})
  );
}

/// How wide each model's name is in mono when nothing constrains it.
fn natural_widths() -> Vec<f32> {
  struct Natural;

  impl Component for Natural {
    type Props = ();

    fn create(_: &mut Ctx) -> Self {
      Self
    }

    fn render(&self, _: &mut Ctx) -> impl Into<Element> {
      Column::new().with_children(MODELS.iter().enumerate().map(|(index, model)| {
        Text::new(model)
          .id(format!("natural-{index}"))
          .variant(ExtraTypography::MonoValue.role())
          .nowrap()
      }))
    }
  }

  let mut app = App::new();
  theme::install(&mut app);
  let mut tree = Tree::new();
  tree.resize(2000, 400);
  tree.mount_root::<Natural>(&mut app, ());
  tree.pass_headless(&mut app);
  (0..MODELS.len())
    .map(|index| {
      let id = format!("natural-{index}");
      tree
        .get_element_by_id_mut(&id)
        .and_then(|element| element.bounds())
        .expect(&id)
        .width
    })
    .collect()
}

/// How much of each reviewer's model, at `prefix`, must show inside the profiles' column.
#[derive(Clone, Copy, Debug)]
enum Shown {
  Whole,
  /// Past what the models share ("claude-"), a glyph that tells them apart and the ellipsis.
  Apart,
}

fn assert_models(desktop: &mut Desktop, prefix: &str, shown: Shown) {
  let shared = MODELS[0]
    .chars()
    .zip(MODELS[1].chars())
    .take_while(|(left, right)| left == right)
    .count();
  let column = desktop.bounds("usage-breakdown-column-0");
  for (model, natural) in MODELS.iter().zip(natural_widths()) {
    let name = desktop.bounds(&format!("{prefix}-{model}"));
    let glyph = natural / model.chars().count() as f32;
    let needed = match shown {
      Shown::Whole => natural,
      Shown::Apart => glyph * (shared + 2) as f32,
    };
    assert!(
      name.width + SAME >= needed,
      "{prefix} {model} shows {name:?}, {shown:?} needs {needed}"
    );
    assert!(
      name.x + name.width <= column.x + column.width + SAME,
      "{prefix} {model} at {name:?} ends past its column {column:?}"
    );
  }
}

#[test]
fn usage_reviewer_rows_keep_their_model_and_figures_in_a_narrow_window() {
  let (mut desktop, _) = reviewed_run();
  desktop.tree.resize(960, 960);
  desktop.pass();
  desktop.wait_for("nav-usage");
  desktop.press("nav-usage");
  desktop.wait_until("the reviewers' rows", |desktop| {
    has(desktop, &format!("usage-breakdown-reviewer-{}", MODELS[0]))
  });
  assert_models(&mut desktop, "usage-breakdown-reviewer", Shown::Whole);
  let column = desktop.bounds("usage-breakdown-column-0");
  for model in MODELS {
    let name = desktop.bounds(&format!("usage-breakdown-reviewer-{model}"));
    let spent = desktop.bounds(&format!("usage-breakdown-reviewer-{model}-spent"));
    let share = desktop.bounds(&format!("usage-breakdown-reviewer-{model}-share"));
    assert!(
      name.x + name.width <= spent.x + SAME,
      "{model}: {name:?} runs into {spent:?}"
    );
    assert!(
      share.x + share.width <= column.x + column.width + SAME,
      "{model}: {share:?} ends past its column {column:?}"
    );
  }

  // Below the fold: clicked where it is, as the page would be scrolled to it.
  desktop.click("usage-breakdown-view-1");
  desktop.wait_for("usage-breakdown-table");
  assert_models(&mut desktop, "usage-breakdown-table-reviewer", Shown::Apart);
  desktop.tree.resize(1440, 960);
  desktop.pass();
  assert_models(&mut desktop, "usage-breakdown-table-reviewer", Shown::Whole);
}
