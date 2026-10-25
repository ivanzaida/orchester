//! What a run delivers (ORC-93) reaches every step's prompt, new and resumed sessions alike, so an investigation writes
//! a report instead of inventing commits and work done through MCP tools is reported where it was done.

use orchester_core::{
  resource::{Deliverable, InjectionEvent},
  run::{last_report, run_delivers},
};

use crate::{
  fixture::{Logged, task},
  prompt::{Prompt, builder, step_with},
};

fn finish(prompt: &Prompt, event: InjectionEvent) -> String {
  let text = prompt.input(&[], &[], true).assemble(event).expect("prompt").text;
  let (_, finish) = text.split_once("# When you finish\n").expect("finish part");
  finish.to_owned()
}

#[test]
fn delivers_prompt_says_what_the_run_delivers_at_start_and_resume() {
  let mut prompt = Prompt::new(builder("Role."), step_with("Step."), task("Brief.", &[]));
  for (delivers, words) in [
    (Deliverable::Changes, "This run delivers repository changes"),
    (Deliverable::Report, "do not make commits to have something to show"),
    (Deliverable::Outside, "Say in your summary what you changed and where"),
    (Deliverable::ChangesOrReport, "Commit only changes the task needs"),
  ] {
    prompt.delivers = delivers;
    for event in [InjectionEvent::Start, InjectionEvent::Resume] {
      let finish = finish(&prompt, event);
      assert!(finish.contains(words), "{delivers} at {event:?}: {finish}");
    }
  }
}

#[test]
fn delivers_prompt_a_report_run_is_told_its_last_summary_is_its_report() {
  let mut prompt = Prompt::new(builder("Role."), step_with("Step."), task("Brief.", &[]));
  prompt.delivers = Deliverable::Report;
  let finish = finish(&prompt, InjectionEvent::Start);
  assert!(
    finish.contains("This run delivers a report, not repository changes"),
    "{finish}"
  );
  assert!(finish.contains("kept as its report"), "{finish}");
  assert!(!finish.contains("its commits are merged"), "{finish}");
}

#[test]
fn delivers_prompt_a_task_says_otherwise_than_its_pipeline() {
  let logged = Logged::started();
  let pipeline = logged.run.snapshot().body().delivers();
  assert_eq!(
    pipeline,
    Deliverable::Changes,
    "pipelines saved before ORC-93 deliver changes"
  );
  let plain = task("Brief.", &[]);
  assert_eq!(run_delivers(&plain, &logged.run), Deliverable::Changes);
  let organization = plain.organization().clone().with_delivers(Some(Deliverable::Outside));
  let chosen = plain.with_organization(organization);
  assert_eq!(run_delivers(&chosen, &logged.run), Deliverable::Outside);
}

#[test]
fn delivers_prompt_a_run_without_outputs_has_no_report() {
  let logged = Logged::started();
  assert_eq!(last_report(&logged.run, &[], &[]), None);
}
