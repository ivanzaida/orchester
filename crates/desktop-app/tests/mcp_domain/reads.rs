//! The Orchestrator's reads (`docs/design/orchestrator-toolset.md`, "Tools" and "Links"): runs, waiting
//! questions and plans read as their cards show them; list tools answer for every project the app knows when
//! none is named, each item naming its project and a project that cannot be read listed apart; and every
//! `link` a read returns is a path the app's router opens.

use std::{fs, path::Path, process::Command};

use orchester_app::routes::Route;
use serde_json::{Value, json};

use super::Served;

/// Every `link` in `value`, however deep.
pub(super) fn links(value: &Value) -> Vec<String> {
  match value {
    Value::Object(fields) => fields
      .iter()
      .flat_map(|(name, field)| match (name.as_str(), field) {
        ("link", Value::String(link)) => vec![link.clone()],
        _ => links(field),
      })
      .collect(),
    Value::Array(items) => items.iter().flat_map(links).collect(),
    _ => Vec::new(),
  }
}

/// Fails unless the router reads every link of `value` as a route.
pub(super) fn every_link_opens(value: &Value) {
  for link in links(value) {
    assert!(Route::parse(&link).is_ok(), "the router opens {link}");
  }
}

/// Fails unless `value` has links and the router reads each as a route.
pub(super) fn opens_in_the_app(value: &Value) {
  assert!(!links(value).is_empty(), "links in {value}");
  every_link_opens(value);
}

fn list<'a>(value: &'a Value, key: &str) -> &'a Vec<Value> {
  value[key]
    .as_array()
    .unwrap_or_else(|| panic!("a list of {key}: {value}"))
}

fn link_of(item: &Value) -> &str {
  item["link"].as_str().unwrap_or_else(|| panic!("a link: {item}"))
}

/// `list_runs` of Kontur: newest first, each linking to its task's Summary, at most `limit`, and one task's.
fn runs_newest_first_with_links(served: &mut Served, project: &str) {
  let root = format!("/project/{project}");
  let kontur = json!({ "project": "Kontur" });
  let runs = served.ok("list_runs", &kontur);
  opens_in_the_app(&runs);
  let listed = list(&runs, "runs");
  assert!(!listed.is_empty(), "{runs}");
  let times: Vec<i64> = listed.iter().filter_map(|run| run["created_at"].as_i64()).collect();
  assert!(
    times.windows(2).all(|pair| pair[0] >= pair[1]),
    "newest first: {times:?}"
  );
  for run in listed {
    assert_eq!(run["project"]["id"], project, "{run}");
    let key = run["task"]["key"].as_str().expect("its task's key");
    assert_eq!(link_of(run), format!("{root}/tasks/{key}/summary"));
  }
  let newest = served.ok("list_runs", &json!({ "project": "Kontur", "limit": 1 }));
  assert_eq!(list(&newest, "runs").len(), 1, "{newest}");
  assert_eq!(newest["more"], listed.len() > 1, "{newest}");
  assert_eq!(list(&newest, "runs")[0]["id"], listed[0]["id"], "the newest: {newest}");
  for limit in [0, 201] {
    let refused = served.refused("list_runs", &json!({ "project": "Kontur", "limit": limit }));
    assert_eq!(refused["code"], "invalid_arguments", "limit {limit}: {refused}");
  }
  let key = listed[0]["task"]["key"].as_str().expect("a key").to_owned();
  let of_task = served.ok("list_runs", &json!({ "project": "Kontur", "task": key }));
  assert!(
    list(&of_task, "runs").iter().all(|run| run["task"]["key"] == key),
    "{of_task}"
  );
}

#[test]
fn domain_reads_runs_questions_and_plans_with_links_to_their_cards() {
  let (mut served, seeded) = Served::seeded("cards");
  let kontur = json!({ "project": "Kontur" });

  runs_newest_first_with_links(&mut served, &seeded.project.to_string());

  let questions = served.ok("list_questions", &kontur);
  opens_in_the_app(&questions);
  let waiting = list(&questions, "questions");
  assert!(!waiting.is_empty(), "the cards wait on questions: {questions}");
  for question in waiting {
    assert!(link_of(question).ends_with("/conversation"), "{question}");
    assert!(question["run"]["number"].is_u64(), "{question}");
  }
  let asked: Vec<i64> = waiting
    .iter()
    .filter_map(|question| question["asked_at"].as_i64())
    .collect();
  assert!(
    asked.windows(2).all(|pair| pair[0] <= pair[1]),
    "oldest first: {asked:?}"
  );
  let asking = waiting[0]["run"]["number"].as_u64().expect("its run");
  let run = served.ok("get_run", &json!({ "project": "Kontur", "run": asking.to_string() }));
  opens_in_the_app(&run);
  assert!(link_of(&run).ends_with("/summary"), "{run}");
  let approved = served.ok("get_plan", &json!({ "project": "Kontur", "run": asking.to_string() }));
  let approved = &approved["plan"];
  assert_eq!(approved["waiting"], false, "a run at work passed its plan: {approved}");
  assert_eq!(approved["decision"]["decision"], "approved", "{approved}");

  let approvals = served.ok("list_approvals", &kontur);
  opens_in_the_app(&approvals);
  let plans = list(&approvals, "plans");
  let revised = plans
    .iter()
    .find(|plan| plan["version"] == 2)
    .unwrap_or_else(|| panic!("a plan revised after changes were asked for: {approvals}"));
  let number = revised["run"]["number"].as_u64().expect("its run").to_string();
  let plan = served.ok("get_plan", &json!({ "project": "Kontur", "run": number }));
  opens_in_the_app(&plan);
  let plan = &plan["plan"];
  assert_eq!(plan["version"], 2, "the step's latest: {plan}");
  assert_eq!(plan["waiting"], true, "{plan}");
  assert!(plan["text"].as_str().is_some_and(|text| !text.is_empty()), "{plan}");
  assert_eq!(
    plan["versions"][0]["decision"]["decision"], "changes_requested",
    "{plan}"
  );
  assert!(link_of(plan).ends_with("/conversation"), "{plan}");
  let first = served.ok("get_plan", &json!({ "project": "Kontur", "run": number, "version": 1 }));
  assert_eq!(first["plan"]["waiting"], false, "{first}");
  let missing = served.refused("get_plan", &json!({ "project": "Kontur", "run": number, "version": 9 }));
  assert_eq!(missing["code"], "plan_not_found", "{missing}");
}

/// A request auto mode held by a hard rule says why in the card's words, in `list_approvals` and in `get_run`; one it
/// did not look at says nothing of it.
#[test]
fn domain_reads_say_why_auto_mode_left_a_request_to_the_person() {
  const CAUSE: &str =
    "Auto mode didn’t review this: the step’s network is off, and this command may reach it or install.";
  let (mut served, _) = Served::seeded("cards");
  let approvals = served.ok("list_approvals", &json!({ "project": "Kontur" }));
  let listed = list(&approvals, "approvals");
  let held = listed
    .iter()
    .find(|approval| approval["auto_mode"]["outcome"] == "held")
    .unwrap_or_else(|| panic!("the request auto mode held: {approvals}"));
  assert_eq!(held["auto_mode"]["hold"], "network_off", "{held}");
  assert_eq!(held["hold_cause"], CAUSE, "{held}");
  assert!(
    listed
      .iter()
      .filter(|approval| approval["id"] != held["id"])
      .all(|approval| approval["hold_cause"].is_null()),
    "{approvals}"
  );
  let number = held["run"]["number"].as_u64().expect("its run").to_string();
  let run = served.ok("get_run", &json!({ "project": "Kontur", "run": number }));
  let waiting = list(&run["awaiting"], "approvals");
  let same = waiting
    .iter()
    .find(|approval| approval["id"] == held["id"])
    .unwrap_or_else(|| panic!("the run's waiting request: {run}"));
  assert_eq!(same["hold_cause"], CAUSE, "{same}");
}

/// A request auto mode's reviewer is still judging is marked `reviewing` in `list_approvals` and in `get_run`; those
/// its review left to the person are not.
#[test]
fn domain_reads_mark_the_requests_auto_mode_is_reviewing() {
  let (mut served, _) = Served::seeded("cards-auto-mode");
  let approvals = served.ok("list_approvals", &json!({ "project": "Kontur" }));
  let marked: Vec<(&str, bool)> = list(&approvals, "approvals")
    .iter()
    .map(|approval| {
      let action = approval["action"].as_str().expect("its command");
      (action, approval["reviewing"].as_bool().expect("reviewing or not"))
    })
    .collect();
  assert_eq!(
    marked,
    [
      ("tail -n 3 CHANGELOG.md", true),
      ("cargo update -p reqwest", false),
      ("cargo tree -d", false),
    ],
    "{approvals}"
  );
  let reviewing = list(&approvals, "approvals")
    .iter()
    .find(|approval| approval["reviewing"] == true)
    .expect("the request under review");
  let number = reviewing["run"]["number"].as_u64().expect("its run").to_string();
  let run = served.ok("get_run", &json!({ "project": "Kontur", "run": number }));
  let waiting = list(&run["awaiting"], "approvals");
  assert_eq!(waiting.len(), 1, "{run}");
  assert_eq!(waiting[0]["reviewing"], true, "{run}");
}

#[test]
fn domain_reads_existing_results_carry_links_the_router_opens() {
  let (mut served, seeded) = Served::seeded("cards");
  let kontur = json!({ "project": "Kontur" });
  opens_in_the_app(&served.ok("list_projects", &json!({})));
  every_link_opens(&served.ok("list_accounts", &json!({})));
  for tool in [
    "get_project",
    "list_tasks",
    "list_pipelines",
    "list_profiles",
    "list_tools",
  ] {
    let read = served.ok(tool, &kontur);
    opens_in_the_app(&read);
    assert!(read.to_string().contains(&seeded.project.to_string()), "{tool}: {read}");
  }
  every_link_opens(&served.ok("list_instructions", &kontur));
  let tasks = served.ok("list_tasks", &kontur);
  let listed = &list(&tasks, "tasks")[0];
  let key = listed["key"].clone();
  let page = format!("/project/{}/tasks/{}", seeded.project, key.as_str().expect("a key"));
  assert_eq!(
    link_of(listed),
    format!("{page}/summary"),
    "a task opens on its Summary"
  );
  let task = served.ok("get_task", &json!({ "project": "Kontur", "key": key }));
  assert_eq!(link_of(&task), format!("{page}/summary"), "a task opens on its Summary");
  for run in list(&task, "runs") {
    assert_eq!(
      link_of(run),
      format!("{page}/summary"),
      "a run opens the Summary: {run}"
    );
  }
  let profiles = served.ok("list_profiles", &kontur);
  let role = list(&profiles, "profiles")[0]["role"].clone();
  let profile = served.ok("get_profile", &json!({ "project": "Kontur", "profile": role }));
  assert_eq!(
    link_of(&profile["profile"]),
    format!(
      "/project/{}/agents/{}",
      seeded.project,
      profile["profile"]["id"].as_str().expect("an id")
    )
  );
  let pipelines = served.ok("list_pipelines", &kontur);
  let name = list(&pipelines, "pipelines")[0]["name"].clone();
  opens_in_the_app(&served.ok("get_pipeline", &json!({ "project": "Kontur", "pipeline": name })));
}

/// A git repository with one commit in a new folder `name` of `parent`.
fn repository(parent: &Path, name: &str) -> String {
  let repo = parent.join(name);
  fs::create_dir_all(&repo).expect("the repository's folder");
  let git = |args: &[&str]| {
    let output = Command::new("git")
      .arg("-C")
      .arg(&repo)
      .args(args)
      .output()
      .expect("run git");
    assert!(
      output.status.success(),
      "git {args:?}: {}",
      String::from_utf8_lossy(&output.stderr)
    );
  };
  git(&["init", "--quiet", "--initial-branch=main"]);
  fs::write(repo.join("README.md"), "hi\n").expect("the readme");
  git(&["add", "README.md"]);
  git(&[
    "-c",
    "user.name=Orchester Test",
    "-c",
    "user.email=test@orchester.invalid",
    "-c",
    "commit.gpgsign=false",
    "commit",
    "--quiet",
    "-m",
    "initial",
  ]);
  repo.display().to_string()
}

#[test]
fn domain_reads_answer_for_every_project_and_list_the_unreadable_apart() {
  let (mut served, seeded) = Served::seeded("empty");
  let parent = served.folder.path().to_owned();
  let ledger = repository(&parent, "ledger");
  served.ok("add_project", &json!({ "name": "Ledger", "folder": ledger }));
  let gone = repository(&parent, "gone");
  served.ok("add_project", &json!({ "name": "Gone", "folder": gone }));
  fs::rename(&gone, parent.join("moved")).expect("the folder moves away");
  served.ok(
    "create_task",
    &json!({ "project": "Kontur", "title": "Count the coins" }),
  );
  served.ok(
    "create_task",
    &json!({ "project": "Ledger", "title": "Balance the books" }),
  );

  let everywhere = served.ok("list_tasks", &json!({}));
  opens_in_the_app(&everywhere);
  let projects: Vec<&str> = list(&everywhere, "tasks")
    .iter()
    .map(|task| task["project"]["name"].as_str().expect("its project"))
    .collect();
  assert!(
    projects.contains(&"Kontur") && projects.contains(&"Ledger"),
    "{everywhere}"
  );
  let kontur_task = list(&everywhere, "tasks")
    .iter()
    .find(|task| task["project"]["name"] == "Kontur")
    .expect("Kontur's task");
  assert!(link_of(kontur_task).starts_with(&format!("/project/{}/tasks/", seeded.project)));
  let unreadable = list(&everywhere, "unreadable_projects");
  assert_eq!(unreadable.len(), 1, "{everywhere}");
  assert_eq!(unreadable[0]["name"], "Gone");
  assert_eq!(unreadable[0]["error"]["code"], "project_folder_missing", "{everywhere}");

  let one = served.ok("list_tasks", &json!({ "project": "Ledger" }));
  assert_eq!(list(&one, "tasks").len(), 1, "{one}");
  assert!(list(&one, "unreadable_projects").is_empty(), "{one}");
  let named_gone = served.refused("list_tasks", &json!({ "project": "Gone" }));
  assert_eq!(named_gone["code"], "project_folder_missing", "{named_gone}");

  for tool in [
    "list_runs",
    "list_questions",
    "list_approvals",
    "list_pipelines",
    "list_profiles",
    "list_instructions",
    "list_skills",
    "list_tools",
    "list_secrets",
    "list_connections",
  ] {
    let read = served.ok(tool, &json!({}));
    assert_eq!(list(&read, "unreadable_projects").len(), 1, "{tool}: {read}");
    let listed = served.tools();
    let schema = &listed
      .iter()
      .find(|listed| listed["name"] == tool)
      .unwrap_or_else(|| panic!("{tool} is listed"))["inputSchema"];
    let required = schema["required"].as_array().cloned().unwrap_or_default();
    assert!(
      !required.contains(&json!("project")),
      "{tool} takes no project: {schema}"
    );
  }
  for (tool, argument) in [
    ("list_runs", "task"),
    ("list_questions", "run"),
    ("list_approvals", "run"),
  ] {
    let refused = served.refused(tool, &json!({ argument: "1" }));
    assert_eq!(refused["code"], "invalid_arguments", "{tool}: {refused}");
  }
}

/// The project database of the repository at `repo`.
fn database(repo: &str) -> std::path::PathBuf {
  Path::new(repo).join(".orchester").join("orchester.db")
}

#[test]
fn domain_reads_leave_a_project_this_version_has_not_opened_untouched() {
  let (mut served, _) = Served::seeded("empty");
  let parent = served.folder.path().to_owned();
  let old = repository(&parent, "old");
  served.ok("add_project", &json!({ "name": "Old", "folder": old }));
  // As a project an older build last opened: its data waits for this version's last migration, all of it in
  // the database file itself.
  let file = database(&old);
  let connection = rusqlite::Connection::open(&file).expect("the project's data");
  let version: u32 = connection
    .pragma_query_value(None, "user_version", |row| row.get(0))
    .expect("its schema version");
  connection
    .pragma_update(None, "user_version", version - 1)
    .expect("an older schema version");
  connection
    .pragma_update(None, "wal_checkpoint", "TRUNCATE")
    .expect("the log written into the file");
  drop(connection);
  let before = fs::read(&file).expect("the database file");

  for tool in [
    "list_tasks",
    "list_runs",
    "list_questions",
    "list_approvals",
    "list_pipelines",
    "list_profiles",
    "list_instructions",
    "list_skills",
    "list_tools",
    "list_secrets",
    "list_connections",
  ] {
    let read = served.ok(tool, &json!({}));
    let unreadable = list(&read, "unreadable_projects");
    assert_eq!(unreadable.len(), 1, "{tool}: {read}");
    assert_eq!(unreadable[0]["name"], "Old", "{tool}: {read}");
    assert_eq!(
      unreadable[0]["error"]["code"], "project_not_opened_yet",
      "{tool}: {read}"
    );
    let named = served.refused(tool, &json!({ "project": "Old" }));
    assert_eq!(named["code"], "project_not_opened_yet", "{tool}: {named}");
  }
  assert_eq!(
    fs::read(&file).expect("the database file"),
    before,
    "no read changed the project's data"
  );
  let after = rusqlite::Connection::open(&file).expect("the project's data");
  let kept: u32 = after
    .pragma_query_value(None, "user_version", |row| row.get(0))
    .expect("its schema version");
  assert_eq!(kept, version - 1, "nothing migrated it");
}
