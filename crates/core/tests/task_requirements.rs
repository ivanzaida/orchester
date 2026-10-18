//! Which versions of a task's requirements the Details can show and which one by default: the creation
//! version when it was kept, else the latest run's start version when that was kept, else the current one;
//! a version that was not kept is never offered, and standing in for a missing creation version is said.
//!
//! `cargo test -p orchester-core --test task_requirements <filter>`

use std::num::NonZeroU32;

use orchester_core::{
  id::RunId,
  name::Name,
  run::RunNumber,
  task::{
    TaskDetails, TaskLimits,
    requirements::{CurrentStanding, Fallback, LatestRun, RequirementsSource, RunStart, TaskRequirements},
  },
};

fn details(title: &str, criteria: &[&str]) -> TaskDetails {
  TaskDetails {
    title: Name::new(title).expect("a title"),
    brief: "Retry failed deliveries.".into(),
    acceptance_criteria: criteria.iter().map(|criterion| (*criterion).to_owned()).collect(),
    pipeline: None,
    limits: TaskLimits::NONE,
  }
}

fn run(number: u32, kept: Option<TaskDetails>) -> (RunId, LatestRun) {
  let run = RunId::generate();
  let start = RunStart {
    run,
    number: RunNumber::new(NonZeroU32::new(number).expect("a number")),
    details: kept,
  };
  (run, LatestRun::Run(start))
}

#[test]
fn requirements_a_new_task_opens_on_its_creation_version() {
  let created = details("Retry webhooks", &["Backs off."]);
  let now = details("Retry webhooks", &["Backs off.", "Can be canceled."]);
  let (id, latest) = run(3, Some(created.clone()));
  let requirements = TaskRequirements::new(now.clone(), Some(created.clone()), latest);
  assert_eq!(
    requirements.sources(),
    vec![
      RequirementsSource::Creation,
      RequirementsSource::RunStart(id),
      RequirementsSource::Current
    ]
  );
  assert_eq!(requirements.shown(None), RequirementsSource::Creation);
  assert_eq!(requirements.details(RequirementsSource::Creation), Some(&created));
  assert_eq!(requirements.details(RequirementsSource::Current), Some(&now));
  assert_eq!(requirements.fallback(RequirementsSource::Creation), None);
  assert_eq!(
    requirements.current_standing(),
    CurrentStanding::EditedAfter(RunNumber::new(NonZeroU32::new(3).expect("3")))
  );
}

#[test]
fn requirements_an_older_task_falls_back_to_its_run_start_and_says_so() {
  let started = details("Retry webhooks", &["Backs off."]);
  let (id, latest) = run(3, Some(started.clone()));
  let requirements = TaskRequirements::new(started, None, latest);
  assert_eq!(
    requirements.sources(),
    vec![RequirementsSource::RunStart(id), RequirementsSource::Current],
    "no creation version is offered"
  );
  assert_eq!(
    requirements.details(RequirementsSource::Creation),
    None,
    "never made up"
  );
  let shown = requirements.shown(None);
  assert_eq!(shown, RequirementsSource::RunStart(id));
  assert_eq!(requirements.fallback(shown), Some(Fallback::RunStart));
  assert_eq!(
    requirements.fallback(RequirementsSource::Current),
    None,
    "a version picked over the default stands in for nothing"
  );
  assert_eq!(
    requirements.current_standing(),
    CurrentStanding::UnchangedSince(RunNumber::new(NonZeroU32::new(3).expect("3")))
  );
}

#[test]
fn requirements_without_any_kept_version_show_the_current_one_honestly() {
  let now = details("Retry webhooks", &["Backs off."]);
  let never_ran = TaskRequirements::new(now.clone(), None, LatestRun::None);
  assert_eq!(never_ran.sources(), vec![RequirementsSource::Current]);
  assert_eq!(never_ran.fallback(never_ran.shown(None)), Some(Fallback::Current));
  assert_eq!(never_ran.current_standing(), CurrentStanding::NoRun);
  let (id, latest) = run(2, None);
  let unrecorded = TaskRequirements::new(now.clone(), None, latest);
  assert_eq!(unrecorded.sources(), vec![RequirementsSource::Current]);
  assert_eq!(unrecorded.details(RequirementsSource::RunStart(id)), None);
  assert_eq!(
    unrecorded.current_standing(),
    CurrentStanding::RunStartNotRecorded(RunNumber::new(NonZeroU32::new(2).expect("2")))
  );
  let unreadable = TaskRequirements::new(now, None, LatestRun::Unknown);
  assert_eq!(unreadable.current_standing(), CurrentStanding::RunUnknown);
}

#[test]
fn requirements_a_chosen_version_stays_while_offered_and_gives_way_when_not() {
  let created = details("Retry webhooks", &["Backs off."]);
  let (first, latest) = run(3, Some(created.clone()));
  let requirements = TaskRequirements::new(created.clone(), Some(created.clone()), latest);
  assert_eq!(
    requirements.shown(Some(RequirementsSource::Current)),
    RequirementsSource::Current
  );
  assert_eq!(
    requirements.shown(Some(RequirementsSource::RunStart(first))),
    RequirementsSource::RunStart(first)
  );
  // The task ran again: the earlier run's version is no longer offered.
  let (_, next) = run(4, Some(created.clone()));
  let after = TaskRequirements::new(created.clone(), Some(created), next);
  assert_eq!(
    after.shown(Some(RequirementsSource::RunStart(first))),
    RequirementsSource::Creation
  );
}
