use std::collections::BTreeMap;

use orchester_core::{
  id::RunId,
  resource::{CommandArg, CommandProgram, FieldName, HttpMethod, VariableName},
  secret::SecretName,
  service::{
    Capture, CapturePath, HttpPath, Placeholder, PlaceholderError, PlaceholderPlace, Readiness, ReadyCheck,
    ReadyTimeout, RunValues, Seed, SeedHeader, SeedRequest, SeedRequests, Service, ServiceError, ServiceValue,
    ServiceVariables, UiStart, check_placeholders,
  },
};

use super::{name, service};

const RUN: &str = "0190f2a4-7b3c-7d4e-8f00-1234567890ab";

fn field(value: &str) -> FieldName {
  FieldName::new(value).expect("a field name")
}

fn variable(value: &str) -> VariableName {
  VariableName::new(value).expect("a variable name")
}

fn plain(value: &str) -> ServiceValue {
  ServiceValue::Plain(value.to_owned())
}

fn secret(value: &str) -> ServiceValue {
  ServiceValue::Secret(SecretName::new(value).expect("a secret name"))
}

fn arguments(values: &[&str]) -> Vec<CommandArg> {
  values
    .iter()
    .map(|value| CommandArg::new(value).expect("an argument"))
    .collect()
}

fn request(url: &str, body: Option<&str>, headers: Vec<SeedHeader>, captures: &[(&str, &str)]) -> SeedRequest {
  let captures = captures
    .iter()
    .map(|(name, path)| Capture::new(field(name), CapturePath::new(path).expect("a JSON path")))
    .collect();
  SeedRequest::new(HttpMethod::Post, url, body.map(str::to_owned), headers, captures).expect("a request")
}

fn seeded(requests: Vec<SeedRequest>) -> Service {
  let requests = SeedRequests::new(requests).expect("captures named once");
  service("web", &["api"]).with_seed(Seed::Requests(requests))
}

fn http_ready(path: &str) -> Service {
  let check = ReadyCheck::Http(HttpPath::new(path).expect("a path"));
  let readiness = Readiness::new(check, ReadyTimeout::from_secs(30).expect("in range"));
  Service::new(name("web"), CommandProgram::new("npm").expect("a program"), readiness)
}

/// The project: `web` needs `api`; `db` is defined but `web` does not need it.
fn project() -> Vec<Service> {
  vec![service("api", &[]), service("db", &[]), service("web", &["api"])]
}

fn values() -> RunValues {
  RunValues::new(
    RUN.parse::<RunId>().expect("a run id"),
    41_234,
    "http://127.0.0.1:41234",
  )
  .with_service(name("api"), 41_235, "http://127.0.0.1:41235")
}

#[test]
fn placeholder_kinds_are_parsed_in_order() {
  let text = "{{port}} {{url}} {{run}} {{services.team-server.url}} {{services.team-server.port}} {{seed.team_id}}";
  let found = Placeholder::find_all(text, &PlaceholderPlace::Argument(0)).expect("known placeholders");
  assert_eq!(
    found,
    [
      Placeholder::Port,
      Placeholder::Url,
      Placeholder::Run,
      Placeholder::ServiceUrl(name("team-server")),
      Placeholder::ServicePort(name("team-server")),
      Placeholder::Seed(field("team_id")),
    ]
  );
  let written: Vec<_> = found.iter().map(ToString::to_string).collect();
  assert_eq!(written.join(" "), text);
}

#[test]
fn placeholder_braces_of_json_are_text() {
  let place = PlaceholderPlace::SeedBody { request: 0 };
  assert_eq!(Placeholder::find_all("no placeholders", &place), Ok(Vec::new()));
  assert_eq!(
    Placeholder::find_all(r#"{"team": {"port": {{port}}}}"#, &place),
    Ok(vec![Placeholder::Port])
  );
}

#[test]
fn placeholder_malformed_braces_name_their_place() {
  let place = PlaceholderPlace::Variable(variable("API_URL"));
  for unclosed in ["{{url", "{{url}", "{{url}}/{{run"] {
    assert_eq!(
      Placeholder::find_all(unclosed, &place),
      Err(PlaceholderError::Unclosed { place: place.clone() }),
      "{unclosed}"
    );
  }
  for malformed in ["{{}}", "{{{port}}", "{{po{{rt}}", "{{a}b}}"] {
    assert_eq!(
      Placeholder::find_all(malformed, &place),
      Err(PlaceholderError::Malformed { place: place.clone() }),
      "{malformed}"
    );
  }
  let error = Placeholder::find_all("{{password=hunter2", &place).expect_err("unclosed");
  assert_eq!(
    error.to_string(),
    "the variable `API_URL` opens a placeholder with `{{` but never closes it with `}}`"
  );
}

#[test]
fn placeholder_unknown_fields_name_their_place() {
  let place = PlaceholderPlace::ReadinessPath;
  for unknown in [
    "{{host}}",
    "{{ port }}",
    "{{PORT}}",
    "{{services.api.host}}",
    "{{services.api}}",
    "{{seed}}",
  ] {
    assert_eq!(
      Placeholder::find_all(unknown, &place),
      Err(PlaceholderError::UnknownField {
        place: place.clone(),
        placeholder: unknown.to_owned()
      }),
      "{unknown}"
    );
  }
  assert_eq!(
    Placeholder::find_all("{{services.Team_Server.url}}", &place),
    Err(PlaceholderError::UnknownService {
      place: place.clone(),
      service: "Team_Server".to_owned()
    })
  );
  assert_eq!(
    Placeholder::find_all("{{seed.Team-Id}}", &place),
    Err(PlaceholderError::UnknownCapture {
      place,
      capture: "Team-Id".to_owned()
    })
  );
}

#[test]
fn placeholder_places_read_as_parts_of_the_service() {
  let places = [
    (PlaceholderPlace::Argument(1), "argument 2"),
    (PlaceholderPlace::Variable(variable("PORT")), "the variable `PORT`"),
    (PlaceholderPlace::ReadinessPath, "the readiness path"),
    (PlaceholderPlace::SeedUrl { request: 0 }, "the URL of seed request 1"),
    (PlaceholderPlace::SeedBody { request: 2 }, "the body of seed request 3"),
    (
      PlaceholderPlace::SeedHeader {
        request: 0,
        header: "X-Team".to_owned(),
      },
      "the header `X-Team` of seed request 1",
    ),
    (PlaceholderPlace::SeedArgument(0), "argument 1 of the seed command"),
    (PlaceholderPlace::UiStart, "the place `ui_open` starts"),
  ];
  for (place, words) in places {
    assert_eq!(place.to_string(), words);
  }
}

#[test]
fn placeholders_of_a_service_check_against_the_project() {
  let ready = http_ready("/healthz/{{run}}")
    .with_needs(vec![name("api")])
    .expect("needs once")
    .with_arguments(arguments(&["--port={{port}}", "--api={{services.api.url}}"]))
    .with_variables(
      ServiceVariables::new(BTreeMap::from([
        (variable("API_PORT"), plain("{{services.api.port}}")),
        (variable("TOKEN"), secret("TEAM_TOKEN")),
      ]))
      .expect("named once"),
    )
    .with_ui_start(UiStart::new("{{url}}/teams").expect("a place"));
  assert_eq!(check_placeholders(&ready, &project()), Ok(()));
  let seeded = seeded_project_seed().with_ui_start(UiStart::new("{{url}}/teams/{{seed.team_id}}").expect("a place"));
  assert_eq!(check_placeholders(&seeded, &project()), Ok(()));
}

fn seeded_project_seed() -> Service {
  let auth = SeedHeader::new("X-Team", plain("{{seed.team_id}}")).expect("a header");
  seeded(vec![
    request(
      "{{services.api.url}}/teams",
      Some(r#"{"run": "{{run}}"}"#),
      Vec::new(),
      &[("team_id", "$.id")],
    ),
    request(
      "{{services.api.url}}/teams/{{seed.team_id}}/invites",
      None,
      vec![auth],
      &[],
    ),
  ])
}

#[test]
fn placeholders_name_services_that_are_defined_and_needed() {
  let unknown = service("web", &["api"]).with_arguments(arguments(&["--x", "{{services.cache.url}}"]));
  assert_eq!(
    check_placeholders(&unknown, &project()),
    Err(PlaceholderError::UnknownService {
      place: PlaceholderPlace::Argument(1),
      service: "cache".to_owned()
    })
  );
  let header = SeedHeader::new("X-Db", plain("{{services.db.port}}")).expect("a header");
  let not_needed = seeded(vec![request("{{url}}/a", None, vec![header], &[])]);
  let error = check_placeholders(&not_needed, &project()).expect_err("db is not needed");
  assert_eq!(
    error,
    PlaceholderError::NotNeeded {
      place: PlaceholderPlace::SeedHeader {
        request: 0,
        header: "X-Db".to_owned()
      },
      service: name("db")
    }
  );
  assert_eq!(
    error.to_string(),
    "the header `X-Db` of seed request 1 uses the service `db`, which this service does not need"
  );
  let itself = http_ready("/{{services.web.port}}");
  assert_eq!(
    check_placeholders(&itself, &project()),
    Err(PlaceholderError::NotNeeded {
      place: PlaceholderPlace::ReadinessPath,
      service: name("web")
    })
  );
}

#[test]
fn placeholders_use_only_what_earlier_seed_requests_captured() {
  let early = seeded(vec![request(
    "{{url}}/a",
    Some(r#"{"team": "{{seed.team_id}}"}"#),
    Vec::new(),
    &[("team_id", "$.id")],
  )]);
  assert_eq!(
    check_placeholders(&early, &project()),
    Err(PlaceholderError::UnknownCapture {
      place: PlaceholderPlace::SeedBody { request: 0 },
      capture: "team_id".to_owned()
    })
  );
  let late = seeded(vec![
    request("{{url}}/a", None, Vec::new(), &[]),
    request("{{url}}/b/{{seed.team_id}}", None, Vec::new(), &[]),
    request("{{url}}/c", None, Vec::new(), &[("team_id", "$.id")]),
  ]);
  assert_eq!(
    check_placeholders(&late, &project()),
    Err(PlaceholderError::UnknownCapture {
      place: PlaceholderPlace::SeedUrl { request: 1 },
      capture: "team_id".to_owned()
    })
  );
  let before_seed = service("web", &[]).with_variables(
    ServiceVariables::new(BTreeMap::from([(variable("TEAM"), plain("{{seed.team_id}}"))])).expect("named once"),
  );
  assert_eq!(
    check_placeholders(&before_seed, &project()),
    Err(PlaceholderError::UnknownCapture {
      place: PlaceholderPlace::Variable(variable("TEAM")),
      capture: "team_id".to_owned()
    })
  );
  let command = service("web", &[]).with_seed(Seed::Command {
    program: CommandProgram::new("scripts/seed.sh").expect("a program"),
    arguments: arguments(&["--run={{run}}", "--team={{seed.team_id}}"]),
  });
  assert_eq!(
    check_placeholders(&command, &project()),
    Err(PlaceholderError::UnknownCapture {
      place: PlaceholderPlace::SeedArgument(1),
      capture: "team_id".to_owned()
    })
  );
  let ui = service("web", &[]).with_ui_start(UiStart::new("{{url}}/{{seed.team_id}}").expect("a place"));
  assert_eq!(
    check_placeholders(&ui, &project()),
    Err(PlaceholderError::UnknownCapture {
      place: PlaceholderPlace::UiStart,
      capture: "team_id".to_owned()
    })
  );
}

#[test]
fn placeholder_errors_of_a_service_name_their_place() {
  let unclosed = service("web", &[]).with_ui_start(UiStart::new("{{url}}/{{run").expect("a place"));
  assert_eq!(
    check_placeholders(&unclosed, &project()),
    Err(PlaceholderError::Unclosed {
      place: PlaceholderPlace::UiStart
    })
  );
  assert_eq!(
    check_placeholders(&http_ready("/{{host}}"), &project()),
    Err(PlaceholderError::UnknownField {
      place: PlaceholderPlace::ReadinessPath,
      placeholder: "{{host}}".to_owned()
    })
  );
  let malformed = seeded(vec![request("{{url}}/a", Some("{{}}"), Vec::new(), &[])]);
  assert_eq!(
    check_placeholders(&malformed, &project()),
    Err(PlaceholderError::Malformed {
      place: PlaceholderPlace::SeedBody { request: 0 }
    })
  );
}

#[test]
fn filling_puts_in_the_run_values_and_leaves_secrets_as_names() {
  let web = http_ready("/healthz/{{run}}")
    .with_arguments(arguments(&["--port", "{{port}}", "--api={{services.api.url}}"]))
    .with_variables(
      ServiceVariables::new(BTreeMap::from([
        (variable("API_PORT"), plain("{{services.api.port}}")),
        (variable("TOKEN"), secret("TEAM_TOKEN")),
      ]))
      .expect("named once"),
    );
  let values = values();
  assert_eq!(
    values.arguments(&web),
    Ok(vec![
      "--port".to_owned(),
      "41234".to_owned(),
      "--api=http://127.0.0.1:41235".to_owned()
    ])
  );
  assert_eq!(
    values.variables(&web),
    Ok(vec![
      (variable("API_PORT"), plain("41235")),
      (variable("TOKEN"), secret("TEAM_TOKEN"))
    ])
  );
  assert_eq!(values.readiness_path(&web), Ok(Some(format!("/healthz/{RUN}"))));
  assert_eq!(values.readiness_path(&service("api", &[])), Ok(None));
  assert_eq!(values.ui_start(&web), Ok(None));
}

#[test]
fn filling_seed_requests_uses_what_earlier_requests_captured() {
  let web = seeded_project_seed().with_ui_start(UiStart::new("{{url}}/teams/{{seed.team_id}}").expect("a place"));
  let mut values = values();
  let first = values.seed_request(&web, 0).expect("filled").expect("a first request");
  assert_eq!(first.method(), HttpMethod::Post);
  assert_eq!(first.url(), "http://127.0.0.1:41235/teams");
  assert_eq!(first.body(), Some(format!(r#"{{"run": "{RUN}"}}"#).as_str()));
  assert_eq!(
    values.seed_request(&web, 1),
    Err(PlaceholderError::NoValue {
      place: PlaceholderPlace::SeedUrl { request: 1 },
      placeholder: Placeholder::Seed(field("team_id"))
    })
  );
  values.capture(field("team_id"), "team-7".to_owned());
  let second = values.seed_request(&web, 1).expect("filled").expect("a second request");
  assert_eq!(second.url(), "http://127.0.0.1:41235/teams/team-7/invites");
  assert_eq!(second.headers(), [("X-Team".to_owned(), plain("team-7"))]);
  assert_eq!(values.seed_request(&web, 2), Ok(None));
  assert_eq!(
    values.ui_start(&web),
    Ok(Some("http://127.0.0.1:41234/teams/team-7".to_owned()))
  );
}

#[test]
fn filling_seed_commands_and_secret_headers() {
  let command = service("web", &[]).with_seed(Seed::Command {
    program: CommandProgram::new("scripts/seed.sh").expect("a program"),
    arguments: arguments(&["--run={{run}}", "--port={{port}}"]),
  });
  let values = values();
  assert_eq!(
    values.seed_arguments(&command),
    Ok(vec![format!("--run={RUN}"), "--port=41234".to_owned()])
  );
  assert_eq!(values.seed_arguments(&service("web", &[])), Ok(Vec::new()));
  assert_eq!(values.seed_request(&command, 0), Ok(None));
  let authorization = SeedHeader::new("Authorization", secret("TEAM_ADMIN_TOKEN")).expect("a header");
  let web = seeded(vec![request("{{url}}/a", None, vec![authorization], &[])]);
  let filled = values.seed_request(&web, 0).expect("filled").expect("a request");
  assert_eq!(
    filled.headers(),
    [("Authorization".to_owned(), secret("TEAM_ADMIN_TOKEN"))]
  );
}

#[test]
fn filling_without_a_value_names_the_place_and_placeholder() {
  let values = RunValues::new(RUN.parse::<RunId>().expect("a run id"), 4000, "http://127.0.0.1:4000");
  let error = values
    .fill("{{services.api.url}}/x", PlaceholderPlace::Argument(0))
    .expect_err("api is not started");
  assert_eq!(
    error,
    PlaceholderError::NoValue {
      place: PlaceholderPlace::Argument(0),
      placeholder: Placeholder::ServiceUrl(name("api"))
    }
  );
  assert_eq!(
    error.to_string(),
    "argument 1: the run has no value for `{{services.api.url}}`"
  );
  assert_eq!(
    values.fill("{{url", PlaceholderPlace::UiStart),
    Err(PlaceholderError::Unclosed {
      place: PlaceholderPlace::UiStart
    })
  );
}

#[test]
fn filled_values_stay_out_of_debug() {
  let mut values = values();
  values.capture(field("invite_token"), "inv-s3cr3t-token".to_owned());
  let debug = format!("{values:?}");
  assert!(
    !debug.contains("inv-s3cr3t-token") && debug.contains("invite_token"),
    "{debug}"
  );
  let body = r#"{"password": "hunter2-correct-horse"}"#;
  let web = seeded(vec![request(
    "{{url}}/users?token={{seed.invite_token}}",
    Some(body),
    Vec::new(),
    &[],
  )]);
  let filled = values.seed_request(&web, 0).expect("filled").expect("a request");
  let debug = format!("{filled:?}");
  assert!(!debug.contains("hunter2") && !debug.contains("inv-s3cr3t"), "{debug}");
}

#[test]
fn ui_start_is_one_place_without_spaces() {
  assert_eq!(
    UiStart::new("{{url}}/teams").expect("a place").as_str(),
    "{{url}}/teams"
  );
  for invalid in ["", "{{url}}/a b", "{{url}}\n"] {
    assert_eq!(UiStart::new(invalid), Err(ServiceError::UiStart));
  }
}
