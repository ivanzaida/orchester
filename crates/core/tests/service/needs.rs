use orchester_core::service::{NeedsError, Stack, StackName, StartPlan};

use super::{name, service};

fn steps(plan: &StartPlan) -> Vec<Vec<&str>> {
  plan
    .steps()
    .iter()
    .map(|step| step.iter().map(|service| service.as_str()).collect())
    .collect()
}

/// The design's Orchester Platform: a database, the cloud API on it, and the apps on the API.
fn platform() -> Vec<orchester_core::service::Service> {
  vec![
    service("team-web", &["team-server"]),
    service("postgres", &[]),
    service("cloud-api", &["postgres"]),
    service("cloud-web", &["cloud-api"]),
    service("team-server", &["cloud-api"]),
    service("desktop-qa", &["cloud-api"]),
  ]
}

#[test]
fn needs_resolve_a_service_after_everything_it_needs() {
  let plan = StartPlan::for_service(&platform(), &name("team-web")).expect("resolves");
  assert_eq!(
    steps(&plan),
    [
      vec!["postgres"],
      vec!["cloud-api"],
      vec!["team-server"],
      vec!["team-web"]
    ]
  );
}

#[test]
fn needs_resolve_independent_services_onto_one_step() {
  let full = ["cloud-web", "team-server", "desktop-qa", "team-web"];
  let stack = Stack::new(
    StackName::new("full").expect("kebab-case"),
    full.iter().map(|service| name(service)).collect(),
  )
  .expect("services listed once");
  let plan = StartPlan::for_stack(&platform(), &stack).expect("resolves");
  assert_eq!(
    steps(&plan),
    [
      vec!["postgres"],
      vec!["cloud-api"],
      vec!["cloud-web", "team-server", "desktop-qa"],
      vec!["team-web"]
    ]
  );
}

#[test]
fn needs_start_a_shared_need_once_on_the_latest_step_it_fits() {
  let services = [
    service("app", &["cache", "api"]),
    service("api", &["db", "cache"]),
    service("cache", &[]),
    service("db", &[]),
  ];
  let plan = StartPlan::for_service(&services, &name("app")).expect("resolves");
  assert_eq!(steps(&plan), [vec!["cache", "db"], vec!["api"], vec!["app"]]);
}

#[test]
fn needs_name_the_services_of_a_cycle() {
  let services = [
    service("web", &["api"]),
    service("api", &["worker"]),
    service("worker", &["queue"]),
    service("queue", &["api"]),
  ];
  let cycle = StartPlan::for_service(&services, &name("web")).expect_err("a cycle");
  assert_eq!(
    cycle,
    NeedsError::Cycle(vec![name("api"), name("worker"), name("queue"), name("api")])
  );
  assert_eq!(cycle.to_string(), "the needs form a cycle: api → worker → queue → api");
}

#[test]
fn needs_name_a_missing_service_and_who_needs_it() {
  let services = [service("web", &["api"]), service("api", &["db"])];
  let missing = StartPlan::for_service(&services, &name("web")).expect_err("db is missing");
  assert_eq!(
    missing,
    NeedsError::Missing {
      name: name("db"),
      needed_by: Some(name("api"))
    }
  );
  assert_eq!(missing.to_string(), "`api` needs `db`, which is not defined");
  let unknown = StartPlan::for_service(&services, &name("docs")).expect_err("docs is missing");
  assert_eq!(
    unknown,
    NeedsError::Missing {
      name: name("docs"),
      needed_by: None
    }
  );
  let stack = Stack::new(
    StackName::new("all").expect("kebab-case"),
    vec![name("api"), name("cache")],
  )
  .expect("once");
  assert!(matches!(
    StartPlan::for_stack(&services, &stack),
    Err(NeedsError::Missing { needed_by: Some(_), .. })
  ));
}

#[test]
fn needs_refuse_a_service_defined_twice() {
  let services = [service("api", &[]), service("api", &[])];
  assert_eq!(
    StartPlan::for_service(&services, &name("api")),
    Err(NeedsError::DuplicateService(name("api")))
  );
}
