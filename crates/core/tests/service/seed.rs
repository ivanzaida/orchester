use orchester_core::{
  resource::{CommandArg, CommandProgram, FieldName, HttpMethod},
  secret::SecretName,
  service::{Capture, CapturePath, Seed, SeedHeader, SeedRequest, SeedRequests, ServiceError, ServiceValue},
};

use super::service;

fn capture(name: &str, path: &str) -> Capture {
  Capture::new(
    FieldName::new(name).expect("a field name"),
    CapturePath::new(path).expect("a JSON path"),
  )
}

fn post(url: &str, captures: Vec<Capture>) -> SeedRequest {
  SeedRequest::new(HttpMethod::Post, url, None, Vec::new(), captures).expect("a request")
}

#[test]
fn seed_requests_run_in_order_with_their_captures() {
  let teams = post(
    "{{services.team-server.url}}/api/teams",
    vec![capture("team_id", "$.id")],
  );
  let invites = post(
    "{{services.team-server.url}}/api/teams/{{team_id}}/invites",
    vec![capture("invite_token", "$.token")],
  );
  let requests = SeedRequests::new(vec![teams, invites]).expect("captures named once");
  let web = service("web", &[]).with_seed(Seed::Requests(requests));
  let Seed::Requests(requests) = web.seed() else {
    panic!("a seed of requests");
  };
  let urls: Vec<_> = requests.as_slice().iter().map(SeedRequest::url).collect();
  assert_eq!(
    urls,
    [
      "{{services.team-server.url}}/api/teams",
      "{{services.team-server.url}}/api/teams/{{team_id}}/invites"
    ]
  );
  assert_eq!(requests.as_slice()[1].captures()[0].name().as_str(), "invite_token");
}

#[test]
fn seed_requests_capture_each_name_once() {
  assert_eq!(SeedRequests::new(Vec::new()), Err(ServiceError::NoSeedRequests));
  let first = post("https://localhost/a", vec![capture("team_id", "$.id")]);
  let second = post("https://localhost/b", vec![capture("team_id", "$.team.id")]);
  assert_eq!(
    SeedRequests::new(vec![first, second]),
    Err(ServiceError::DuplicateCapture(
      FieldName::new("team_id").expect("a field name")
    ))
  );
}

#[test]
fn seed_capture_paths_are_json_paths() {
  for valid in ["$", "$.id", "$.items[0].id", "$[2]", "$.data.team-id"] {
    assert_eq!(CapturePath::new(valid).expect("a JSON path").as_str(), valid);
  }
  for invalid in ["", "id", "$.", "$..id", "$.a b", "$[x]", "$[]", "$[0", "$id", "$.a]"] {
    assert_eq!(
      CapturePath::new(invalid),
      Err(ServiceError::CapturePath(invalid.to_owned()))
    );
  }
}

#[test]
fn seed_request_urls_and_headers_are_valid() {
  for invalid in ["", "localhost/api", "ftp://host/x", "{{url}}/a b", "https://host/\n"] {
    assert_eq!(
      SeedRequest::new(HttpMethod::Get, invalid, None, Vec::new(), Vec::new()),
      Err(ServiceError::SeedUrl)
    );
  }
  let tokened = SeedRequest::new(
    HttpMethod::Get,
    "https://host/a b?token=t0ken",
    None,
    Vec::new(),
    Vec::new(),
  );
  assert!(!tokened.expect_err("a space").to_string().contains("t0ken"));
  let plain = |value: &str| ServiceValue::Plain(value.to_owned());
  assert_eq!(
    SeedHeader::new("X Team", plain("1")),
    Err(ServiceError::HeaderName("X Team".to_owned()))
  );
  assert_eq!(
    SeedHeader::new("X-Team", plain("1\r\nHost: evil")),
    Err(ServiceError::HeaderValue("X-Team".to_owned()))
  );
  let headers = vec![
    SeedHeader::new("Content-Type", plain("application/json")).expect("a header"),
    SeedHeader::new("content-type", plain("text/plain")).expect("a header"),
  ];
  assert_eq!(
    SeedRequest::new(HttpMethod::Post, "{{url}}/a", None, headers, Vec::new()),
    Err(ServiceError::DuplicateHeader("content-type".to_owned()))
  );
}

#[test]
fn seed_request_debug_leaves_out_its_body_and_plain_header_values() {
  let password = "hunter2-correct-horse";
  let headers = vec![
    SeedHeader::new("X-Admin-Key", ServiceValue::Plain(password.to_owned())).expect("a header"),
    SeedHeader::new(
      "Authorization",
      ServiceValue::Secret(SecretName::new("TEAM_ADMIN_TOKEN").expect("a secret name")),
    )
    .expect("a header"),
  ];
  let body = Some(format!(r#"{{"email": "qa@example.test", "password": "{password}"}}"#));
  let request = SeedRequest::new(HttpMethod::Post, "{{url}}/api/users", body, headers, Vec::new()).expect("a request");
  assert!(request.body().is_some_and(|body| body.contains(password)));
  let debug = format!("{request:?}");
  assert!(!debug.contains(password), "{debug}");
  assert!(
    debug.contains("TEAM_ADMIN_TOKEN") && debug.contains("{{url}}/api/users"),
    "{debug}"
  );
}

#[test]
fn seed_command_runs_a_program_without_a_shell() {
  let program = CommandProgram::new("scripts/seed.sh").expect("a program in the worktree");
  let arguments = vec![CommandArg::new("--team={{run}}").expect("an argument")];
  let seed = Seed::Command { program, arguments };
  let web = service("web", &[]).with_seed(seed.clone());
  assert_eq!(web.seed(), &seed);
  assert_eq!(service("web", &[]).seed(), &Seed::None);
}
