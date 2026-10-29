use std::collections::BTreeMap;

use orchester_core::{
  resource::{CommandArg, CommandError, VariableName},
  secret::SecretName,
  service::{
    HttpPath, LogPattern, OwnedPath, ReadyCheck, ReadyTimeout, RelativeFolder, RunData, RunFolder, ServiceError,
    ServiceName, ServiceValue, ServiceVariables, Stack, StackName, UiDriver,
  },
};

use super::{name, service};

#[test]
fn service_names_are_kebab_case() {
  assert_eq!(name("team-server").as_str(), "team-server");
  assert_eq!(StackName::new("full").expect("kebab-case").as_str(), "full");
  for invalid in [
    "",
    "Team",
    "team_server",
    "team server",
    "-web",
    "web-",
    "team--web",
    &"a".repeat(65),
  ] {
    assert_eq!(ServiceName::new(invalid), Err(ServiceError::Name(invalid.to_owned())));
    assert!(StackName::new(invalid).is_err(), "{invalid} should be rejected");
  }
}

#[test]
fn service_starts_in_the_worktree_without_anything_else() {
  let web = service("web", &[]);
  assert!(web.arguments().is_empty() && web.needs().is_empty() && web.owned_paths().is_empty());
  assert!(web.working_folder().is_root() && web.variables().is_empty());
  assert_eq!(web.run_data(), &RunData::None);
  assert_eq!(web.ui_driver(), UiDriver::None);
  assert_eq!(web.readiness().check(), &ReadyCheck::PortOpen);
  assert_eq!(web.readiness().timeout().secs(), 30);
}

#[test]
fn service_arguments_keep_placeholders_as_text() {
  let args = ["run", "dev", "--port", "{{port}}", "--name=QA team"];
  let arguments: Vec<_> = args
    .iter()
    .map(|arg| CommandArg::new(arg).expect("an argument"))
    .collect();
  let web = service("web", &[]).with_arguments(arguments);
  let written: Vec<_> = web.arguments().iter().map(CommandArg::as_str).collect();
  assert_eq!(written, args);
  assert_eq!(CommandArg::new(""), Err(CommandError::EmptyArgument));
  assert!(matches!(
    CommandArg::new("a\nb"),
    Err(CommandError::ControlCharacter(_))
  ));
}

#[test]
fn service_working_folder_stays_inside_the_worktree() {
  assert_eq!(RelativeFolder::new(" web/team ").expect("inside").as_str(), "web/team");
  assert!(RelativeFolder::new(".").expect("the worktree").is_root());
  assert!(RelativeFolder::new("").expect("the worktree").is_root());
  assert!(RelativeFolder::new(r"web\{{run}}").is_ok());
  for outside in [
    "../web",
    "web/../../x",
    r"web\..\..",
    "/srv/web",
    r"\srv",
    r"\\server\share",
    "C:/web",
    "C:web",
  ] {
    assert_eq!(
      RelativeFolder::new(outside),
      Err(ServiceError::OutsideWorktree(outside.to_owned()))
    );
  }
  assert!(RelativeFolder::new("web\ntools").is_err());
}

#[test]
fn service_run_folder_is_a_folder_of_its_own() {
  let folder = RunFolder::new("runs/{{run}}/team-web").expect("below the worktree");
  assert_eq!(folder.as_str(), "runs/{{run}}/team-web");
  let web = service("web", &[]).with_run_data(RunData::RunFolder(folder));
  assert!(matches!(web.run_data(), RunData::RunFolder(_)));
  assert_eq!(RunFolder::new("."), Err(ServiceError::RunFolderIsRoot));
  assert_eq!(RunFolder::new(""), Err(ServiceError::RunFolderIsRoot));
  assert!(matches!(
    RunFolder::new("../runs"),
    Err(ServiceError::OutsideWorktree(_))
  ));
  let database = service("db", &[]).with_run_data(RunData::DatabaseContainer);
  assert_eq!(database.run_data(), &RunData::DatabaseContainer);
}

#[test]
fn service_variables_are_named_once_without_regard_to_case() {
  let variable = |value: &str| VariableName::new(value).expect("a variable name");
  let secret = ServiceValue::Secret(SecretName::new("TEAM_SESSION_SECRET").expect("a secret name"));
  let variables = BTreeMap::from([
    (variable("PORT"), ServiceValue::Plain("{{port}}".to_owned())),
    (variable("SESSION_SECRET"), secret.clone()),
  ]);
  let variables = ServiceVariables::new(variables).expect("named once");
  let web = service("web", &[]).with_variables(variables);
  let names: Vec<_> = web.variables().iter().map(|(name, _)| name.as_str()).collect();
  assert_eq!(names, ["PORT", "SESSION_SECRET"]);
  let twice = BTreeMap::from([(variable("Port"), secret.clone()), (variable("PORT"), secret)]);
  assert_eq!(
    ServiceVariables::new(twice),
    Err(ServiceError::DuplicateVariable("Port".to_owned()))
  );
  let nul = BTreeMap::from([(variable("PORT"), ServiceValue::Plain("4\0".to_owned()))]);
  assert_eq!(
    ServiceVariables::new(nul),
    Err(ServiceError::VariableValue("PORT".to_owned()))
  );
}

#[test]
fn service_debug_leaves_out_plain_values() {
  let token = "sk-live-0123456789";
  let variables = BTreeMap::from([(
    VariableName::new("API_TOKEN").expect("a variable name"),
    ServiceValue::Plain(token.to_owned()),
  )]);
  let web = service("web", &[]).with_variables(ServiceVariables::new(variables).expect("named once"));
  let debug = format!("{web:?}");
  assert!(!debug.contains(token), "{debug}");
  assert!(
    debug.contains("API_TOKEN") && debug.contains("<18 characters>"),
    "{debug}"
  );
  let secret = ServiceValue::Secret(SecretName::new("API_TOKEN").expect("a secret name"));
  assert!(format!("{secret:?}").contains("API_TOKEN"));
}

#[test]
fn service_needs_other_services_once_each() {
  let web = service("web", &["api", "db"]);
  assert_eq!(web.needs(), [name("api"), name("db")]);
  assert_eq!(
    service("web", &[]).with_needs(vec![name("web")]),
    Err(ServiceError::NeedsItself(name("web")))
  );
  assert_eq!(
    service("web", &[]).with_needs(vec![name("api"), name("db"), name("api")]),
    Err(ServiceError::DuplicateNeed(name("api")))
  );
}

#[test]
fn service_readiness_checks_are_valid() {
  assert_eq!(
    HttpPath::new("/healthz?full=1").expect("a path").as_str(),
    "/healthz?full=1"
  );
  for invalid in ["", "healthz", "/health z", "/a\nb"] {
    assert_eq!(HttpPath::new(invalid), Err(ServiceError::HttpPath(invalid.to_owned())));
  }
  assert_eq!(
    LogPattern::new(r"ready in \d+ ms").expect("a regex").as_str(),
    r"ready in \d+ ms"
  );
  assert!(matches!(
    LogPattern::new("ready ("),
    Err(ServiceError::LogPattern { .. })
  ));
  assert!(matches!(LogPattern::new(""), Err(ServiceError::LogPattern { .. })));
  assert_eq!(LogPattern::new(&"a".repeat(1025)), Err(ServiceError::LogPatternTooLong));
  assert_eq!(ReadyTimeout::from_secs(90).expect("in range").secs(), 90);
  assert_eq!(ReadyTimeout::from_secs(0), Err(ServiceError::ReadyTimeout));
  assert_eq!(ReadyTimeout::from_secs(3601), Err(ServiceError::ReadyTimeout));
}

#[test]
fn service_ui_driver_is_none_native_or_mcp() {
  for driver in [UiDriver::None, UiDriver::Native, UiDriver::Mcp] {
    assert_eq!(service("app", &[]).with_ui_driver(driver).ui_driver(), driver);
  }
}

#[test]
fn service_owned_paths_are_repository_patterns_listed_once() {
  let path = |value: &str| OwnedPath::new(value).expect("a pattern");
  let web = service("web", &[])
    .with_owned_paths(vec![path("web/team/**"), path("packages/team-ui/**")])
    .expect("listed once");
  assert_eq!(web.owned_paths()[0].as_str(), "web/team/**");
  for invalid in ["", ".", "../shared/**", "/web/**", "C:/web/**"] {
    assert_eq!(
      OwnedPath::new(invalid),
      Err(ServiceError::OwnedPath(invalid.to_owned()))
    );
  }
  assert_eq!(
    service("web", &[]).with_owned_paths(vec![path("web/**"), path("web/**")]),
    Err(ServiceError::DuplicateOwnedPath("web/**".to_owned()))
  );
}

#[test]
fn stack_lists_services_once() {
  let full = StackName::new("full").expect("kebab-case");
  let stack = Stack::new(full.clone(), vec![name("api"), name("web")]).expect("listed once");
  assert_eq!(stack.name(), &full);
  assert_eq!(stack.services(), [name("api"), name("web")]);
  assert_eq!(
    Stack::new(full.clone(), Vec::new()),
    Err(ServiceError::EmptyStack(full.clone()))
  );
  assert_eq!(
    Stack::new(full.clone(), vec![name("api"), name("web"), name("api")]),
    Err(ServiceError::DuplicateStackService {
      stack: full,
      service: name("api")
    })
  );
}
