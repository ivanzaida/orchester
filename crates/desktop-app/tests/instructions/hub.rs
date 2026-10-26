//! Instructions in the Agents hub: one list in the order prompts assemble them, and a new rule that chooses its
//! scope on its page.
use orchester_core::{resource::InstructionScope, store::ResourceStore};
use orchester_store::ProjectDb;

use super::{harness::Instructions, preview::seed_preview};

#[test]
fn instructions_list_follows_the_prompts_order() {
  let mut screen = Instructions::new();
  let pipeline = seed_preview(&screen);
  let db = ProjectDb::open(&screen.root).expect("private fixture");
  let step = db
    .latest_version::<orchester_core::resource::Pipeline>(pipeline)
    .expect("pipeline")
    .expect("published")
    .body()
    .steps()[0]
    .key
    .clone();
  // Made in the reverse of the order prompts take them.
  screen.rule("Step rule", "Step text", InstructionScope::Step { pipeline, step }, true);
  screen.rule("Pipeline rule", "Pipeline text", InstructionScope::Pipeline { pipeline }, true);
  screen.rule("Project rule", "Project text", InstructionScope::Project, true);
  screen.list();
  screen.until("listed", |s| s.text("instruction-library").contains("Step rule"));
  let list = screen.text("instruction-library");
  let at = |name: &str| list.find(name).unwrap_or_else(|| panic!("{name} in {list}"));
  assert!(at("Project rule") < at("Pipeline rule"), "{list}");
  assert!(at("Pipeline rule") < at("Step rule"), "{list}");
  assert!(screen.exists("instructions-order-note"));
}

#[test]
fn instructions_a_new_rule_chooses_its_scope_on_its_page() {
  let mut screen = Instructions::new();
  let pipeline = seed_preview(&screen);
  screen.list();
  screen.click("instructions-new");
  screen.until("the scope choice", |s| s.exists("instruction-scope-project"));
  assert!(screen.exists("instruction-scope-role") && screen.exists("instruction-scope-step"));
  screen.click("instruction-scope-pipeline");
  screen.until("the pipeline select", |s| s.exists("instruction-scope-pipeline-select"));
  screen.choose("instruction-scope-pipeline-select", "Feature delivery");
  screen.set("instruction-name", "Delivery rule");
  screen.set("instruction-prompt", "Deliver in small steps.");
  screen.click("instruction-create");
  screen.until("created in the chosen scope", |s| {
    s.listed()
      .entries
      .iter()
      .any(|e| &*e.name == "Delivery rule" && *e.body.scope() == InstructionScope::Pipeline { pipeline })
  });
}
