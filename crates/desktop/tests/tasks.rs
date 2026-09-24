use std::{fs, time::Duration};

use orchester_core::{
  id::{IdempotencyKey, ProjectId, ResourceId},
  money::Money,
  name::Name,
  project::{Project, ProjectKey},
  resource::{Pipeline, PromptText, RepositoryAccess, Slug, Step, StepKey, StepKind},
  store::{ResourceStore, TaskStore},
  task::{MissingDetail, Task, TaskDetails, TaskStatus},
  time::Timestamp,
};
use orchester_desktop::{
  display,
  tasks::{
    data::{ProjectTasks, published_pipelines, save_draft},
    form::{BUDGET_CURRENCY, TaskForm},
    list::{self, DraftSummary, PipelineLabel, Readiness, TaskTab},
  },
};
use orchester_store::ProjectDb;
use tempfile::TempDir;

fn form<'a>(title: &'a str, criteria: &'a [String], budget: &'a str, time_limit: &'a str) -> TaskForm<'a> {
  TaskForm {
    title,
    brief: "  Let people find a task.  ",
    criteria,
    pipeline: None,
    budget,
    time_limit,
  }
}

fn project_db() -> (TempDir, ProjectDb) {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("valid key"),
    Name::new("Kontur").expect("valid name"),
  );
  let db = ProjectDb::create(repo.path(), project).expect("create the project database");
  (repo, db)
}

fn details(title: &str) -> TaskDetails {
  TaskDetails {
    title: Name::new(title).expect("valid title"),
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: None,
  }
}

fn now() -> Timestamp {
  Timestamp::from_unix_millis(1_700_000_000_000)
}

#[test]
fn form_maps_entered_values_to_task_details() {
  let criteria = vec!["  Filters as you type. ".to_owned(), "   ".to_owned()];
  let details = form(" Add task search ", &criteria, "5.50", "30")
    .details()
    .expect("valid form");
  assert_eq!(details.title.as_str(), "Add task search");
  assert_eq!(details.brief, "Let people find a task.");
  assert_eq!(details.acceptance_criteria, vec!["Filters as you type.".to_owned()]);
  let limits = details.limits.expect("both limits entered");
  assert_eq!(limits.budget(), Money::from_micros(5_500_000, BUDGET_CURRENCY));
  assert_eq!(limits.max_duration(), Duration::from_secs(30 * 60));
}

#[test]
fn form_allows_an_incomplete_draft_and_names_what_is_missing() {
  let blank_rows = vec![String::new()];
  let incomplete = TaskForm {
    brief: "",
    ..form("Add task search", &blank_rows, "", "")
  };
  let details = incomplete
    .details()
    .expect("a draft may leave everything but the title out");
  assert_eq!(details.limits, None);
  assert!(details.acceptance_criteria.is_empty());
  assert_eq!(
    incomplete.missing(),
    vec![
      MissingDetail::Brief,
      MissingDetail::AcceptanceCriteria,
      MissingDetail::Pipeline,
      MissingDetail::Limits
    ]
  );
}

#[test]
fn form_reports_missing_details_while_the_title_is_blank() {
  let criteria = vec!["Filters as you type.".to_owned()];
  let blank_title = form("  ", &criteria, "5", "30");
  let errors = blank_title.details().expect_err("a title is required");
  assert_eq!(errors.title.map(|message| message.key), Some("errors.titleRequired"));
  assert_eq!(blank_title.missing(), vec![MissingDetail::Pipeline]);
}

#[test]
fn form_rejects_invalid_and_half_entered_limits() {
  let error_keys = |budget: &str, time_limit: &str| {
    let errors = form("Task", &[], budget, time_limit)
      .details()
      .expect_err("limits are invalid");
    (
      errors.budget.map(|message| message.key),
      errors.time_limit.map(|message| message.key),
    )
  };
  assert_eq!(error_keys("5", ""), (None, Some("errors.timeLimitRequired")));
  assert_eq!(error_keys("", "30"), (Some("errors.budgetRequired"), None));
  assert_eq!(
    error_keys("five", "half an hour"),
    (Some("errors.budgetInvalid"), Some("errors.timeLimitInvalid"))
  );
  assert_eq!(error_keys("0", "30"), (Some("errors.budgetNotPositive"), None));
  assert_eq!(error_keys("5", "0"), (None, Some("errors.timeLimitZero")));
}

#[test]
fn retrying_a_save_with_the_same_key_creates_one_task() {
  let (_repo, mut db) = project_db();
  let batch = IdempotencyKey::generate();
  let first = save_draft(&mut db, batch, details("Add task search"), now()).expect("first attempt");
  let retry = save_draft(&mut db, batch, details("Add task search"), now()).expect("retry");
  assert_eq!(retry.id(), first.id());
  assert_eq!(db.tasks().expect("read tasks").len(), 1);
}

#[test]
fn a_retry_after_editing_keeps_the_task_and_the_edits() {
  let (_repo, mut db) = project_db();
  let batch = IdempotencyKey::generate();
  // The first attempt was stored although the form saw it fail, e.g. the response was lost.
  let stored = save_draft(&mut db, batch, details("Add task serch"), now()).expect("first attempt");
  let edited = TaskDetails {
    brief: "Let people find a task.".into(),
    ..details("Add task search")
  };
  let retried = save_draft(&mut db, batch, edited.clone(), now()).expect("retry");
  assert_eq!(retried.id(), stored.id());
  let tasks = db.tasks().expect("read tasks");
  assert_eq!(tasks.len(), 1);
  assert_eq!(tasks[0].details(), &edited);
  assert_eq!(tasks[0].key().to_string(), "ORC-1");
}

#[test]
fn a_new_key_creates_the_next_task() {
  let (_repo, mut db) = project_db();
  save_draft(&mut db, IdempotencyKey::generate(), details("First"), now()).expect("first task");
  let second = save_draft(&mut db, IdempotencyKey::generate(), details("Second"), now()).expect("second task");
  assert_eq!(second.key().to_string(), "ORC-2");
}

fn research_step() -> Step {
  Step {
    key: StepKey::new("research").expect("valid key"),
    name: Name::new("Research").expect("valid name"),
    kind: StepKind::Agent {
      profile: Some(ResourceId::generate()),
      repository: RepositoryAccess::ReadOnly,
      model: None,
    },
    instructions: PromptText::new("").expect("valid prompt"),
    depends_on: Default::default(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  }
}

#[test]
fn only_published_pipelines_can_be_chosen() {
  let (repo, mut db) = project_db();
  let pipeline = Pipeline::new(vec![research_step()]).expect("valid pipeline");
  let create = |db: &mut ProjectDb, slug: &str, name: &str| {
    db.create_resource(
      Slug::new(slug).expect("valid slug"),
      Name::new(name).expect("valid name"),
      &pipeline,
      now(),
    )
    .expect("create pipeline")
  };
  let published = create(&mut db, "delivery", "Delivery");
  create(&mut db, "draft-only", "Draft only");
  db.publish_draft::<Pipeline>(published.id(), now()).expect("publish");

  let choices = published_pipelines(&db).expect("read pipelines");
  assert_eq!(choices.len(), 1);
  assert_eq!(&*choices[0].name, "Delivery");
  assert_eq!(choices[0].version.get(), 1);
  assert_eq!(
    choices[0].steps.iter().map(|step| &**step).collect::<Vec<_>>(),
    ["Research"]
  );
  assert_eq!(
    PipelineLabel::of(Some(published.id()), &choices).message().key,
    "tasks.pipeline.published"
  );
  assert_eq!(
    PipelineLabel::of(Some(ResourceId::generate()), &choices),
    PipelineLabel::Unavailable
  );

  let loaded = ProjectTasks::load(repo.path()).expect("load the project's tasks");
  assert_eq!(loaded.pipelines.len(), 1);
  assert!(loaded.tasks.is_empty());
}

fn stored_tasks() -> Vec<Task> {
  let (_repo, mut db) = project_db();
  save_draft(&mut db, IdempotencyKey::generate(), details("Incomplete"), now()).expect("draft");
  let mut queued = db
    .create_tasks(IdempotencyKey::generate(), vec![details("To queue")], now())
    .expect("create")
    .remove(0);
  // Queueing is not in the UI yet; a queued task still must not appear under Drafts.
  let complete = TaskDetails {
    brief: "Brief.".into(),
    acceptance_criteria: vec!["Works.".into()],
    pipeline: Some(ResourceId::generate()),
    limits: Some(
      orchester_core::task::TaskLimits::new(Money::from_micros(1, BUDGET_CURRENCY), Duration::from_secs(60))
        .expect("positive limits"),
    ),
    ..details("To queue")
  };
  queued = Task::from_stored(
    queued.id(),
    queued.key().clone(),
    complete,
    queued.status(),
    queued.created_at(),
  );
  queued.queue().expect("complete draft can be queued");
  db.save_task(&queued).expect("save queued task");
  db.tasks().expect("read tasks")
}

#[test]
fn tabs_filter_and_count_tasks() {
  let tasks = stored_tasks();
  assert_eq!(TaskTab::All.count(&tasks), Some(2));
  assert_eq!(TaskTab::Drafts.count(&tasks), Some(1));
  for unavailable in [TaskTab::Queue, TaskTab::NeedsReview, TaskTab::Done] {
    assert!(!unavailable.is_available());
    assert_eq!(unavailable.count(&tasks), None);
    assert!(list::rows(&tasks, &[], unavailable).is_empty());
  }
  let drafts = list::rows(&tasks, &[], TaskTab::Drafts);
  assert_eq!(drafts.len(), 1);
  assert_eq!(&*drafts[0].title, "Incomplete");
  assert_eq!(drafts[0].status, TaskStatus::Draft);
  assert_eq!(
    DraftSummary::of(&tasks),
    DraftSummary {
      ready: 0,
      incomplete: 1
    }
  );
  let recent = list::recent(&tasks, &[], 1);
  assert_eq!(&*recent[0].title, "To queue");
}

#[test]
fn readiness_names_one_missing_detail_and_counts_several() {
  assert_eq!(Readiness::Ready.message().key, "tasks.readiness.ready");
  assert_eq!(
    Readiness::Missing(vec![MissingDetail::Pipeline]).message().key,
    "tasks.readiness.missingPipeline"
  );
  let several = Readiness::Missing(vec![MissingDetail::Brief, MissingDetail::Limits]).message();
  assert_eq!(several.key, "tasks.readiness.missingCount");
  assert_eq!(several.args, vec![("count", "2".to_owned())]);
}

#[test]
fn elapsed_time_uses_the_largest_whole_unit() {
  let at = |millis: i64| Timestamp::from_unix_millis(1_700_000_000_000 + millis);
  let key = |millis: i64| display::elapsed(at(millis), at(0));
  assert_eq!(key(59_000).key, "time.justNow");
  assert_eq!(key(-5_000).key, "time.justNow");
  assert_eq!(key(4 * 60_000).args, vec![("count", "4".to_owned())]);
  assert_eq!(key(4 * 60_000).key, "time.minutesAgo");
  assert_eq!(key(2 * 3_600_000).key, "time.hoursAgo");
  assert_eq!(key(3 * 86_400_000).key, "time.daysAgo");
}

#[test]
fn form_names_one_problem_per_message_for_invalid_limits() {
  let with_limits = |budget: &'static str, time_limit: &'static str| form("Task", &[], budget, time_limit);
  let invalid = with_limits("five", "30");
  let errors = invalid.details().expect_err("the budget is invalid");
  assert!(!invalid.missing().contains(&MissingDetail::Limits));
  assert_eq!(
    errors
      .save_reasons()
      .iter()
      .map(|reason| reason.key)
      .collect::<Vec<_>>(),
    ["tasks.new.limitsBlockSave"]
  );
  let half_entered = with_limits("5", "");
  assert!(!half_entered.missing().contains(&MissingDetail::Limits));
  assert!(with_limits("", "").missing().contains(&MissingDetail::Limits));

  let blank_title = form(" ", &[], "5", "0")
    .details()
    .expect_err("title and time limit are invalid");
  assert_eq!(
    blank_title
      .save_reasons()
      .iter()
      .map(|reason| reason.key)
      .collect::<Vec<_>>(),
    ["errors.titleRequired", "tasks.new.limitsBlockSave"]
  );
}

#[test]
fn task_list_keeps_task_order_and_overview_shows_newest_first() {
  let tasks = stored_tasks();
  let titles = |rows: Vec<list::TaskRow>| rows.iter().map(|row| row.title.to_string()).collect::<Vec<_>>();
  assert_eq!(
    titles(list::rows(&tasks, &[], TaskTab::All)),
    ["Incomplete", "To queue"]
  );
  assert_eq!(titles(list::recent(&tasks, &[], 2)), ["To queue", "Incomplete"]);
}
