use std::{
  collections::{BTreeMap, BTreeSet},
  num::NonZeroU32,
};

use orchester_core::{
  id::{ResourceId, SecretId},
  name::Name,
  resource::{
    AccessPolicy, AccessPolicyError, AccessWindow, AllowedSteps, ApprovalMode, DatabaseQuery, DeniedBehavior,
    FieldName, HttpAuth, HttpError, HttpMethod, HttpRequest, RequestTarget, ResponseSource, StepKey, Tool, ToolAction,
    ToolError,
  },
};

fn field(name: &str) -> FieldName {
  FieldName::new(name).expect("valid field name")
}

fn bearer() -> HttpAuth {
  HttpAuth::Bearer {
    secret: SecretId::generate(),
  }
}

fn request(url: &str, allowed_host: &str, auth: HttpAuth) -> Result<HttpRequest, HttpError> {
  HttpRequest::new(
    HttpMethod::Post,
    url.into(),
    allowed_host.into(),
    auth,
    BTreeMap::from([(field("revision"), RequestTarget::Body("deployment.revision".into()))]),
    BTreeMap::from([
      (field("status"), ResponseSource::Status),
      (field("preview_url"), ResponseSource::Body("deployment.url".into())),
    ]),
  )
}

fn with_input(target: RequestTarget) -> Result<HttpRequest, HttpError> {
  HttpRequest::new(
    HttpMethod::Get,
    "https://deploy.example.test/v1".into(),
    "deploy.example.test".into(),
    HttpAuth::None,
    BTreeMap::from([(field("value"), target)]),
    BTreeMap::new(),
  )
}

fn policy(callers: BTreeMap<ResourceId, AllowedSteps>) -> Result<AccessPolicy, AccessPolicyError> {
  AccessPolicy::new(
    callers,
    Name::new("Staging").expect("valid environment"),
    ApprovalMode::EveryCall,
    AccessWindow::UntilStepEnds,
    DeniedBehavior::AwaitChangedPlan,
  )
}

#[test]
fn tool_http_request_binds_a_secret_reference_to_bearer_auth() {
  let auth = bearer();
  let request = request(
    "https://Deploy.Example.test:8443/v1/deployments?dry=0",
    "deploy.example.test",
    auth,
  )
  .unwrap();
  assert_eq!(request.auth(), auth);
  assert_eq!(request.method(), HttpMethod::Post);
  assert_eq!(request.inputs().len(), 1);
  assert_eq!(request.outputs().len(), 2);
}

#[test]
fn tool_http_request_url_must_be_absolute_without_credentials() {
  for url in [
    "deploy.example.test/v1",
    "ftp://deploy.example.test/",
    "https:///v1",
    "https://user:pass@deploy.example.test/v1",
    "https://deploy.example.test/v 1",
  ] {
    assert_eq!(
      request(url, "deploy.example.test", HttpAuth::None),
      Err(HttpError::InvalidUrl(url.into())),
      "{url}"
    );
  }
}

#[test]
fn tool_http_request_host_must_be_the_allowed_host() {
  assert_eq!(
    request("https://evil.example.test/v1", "deploy.example.test", bearer()),
    Err(HttpError::HostNotAllowed {
      host: "evil.example.test".into(),
      allowed: "deploy.example.test".into(),
    })
  );
}

#[test]
fn tool_http_request_bearer_requires_https() {
  assert_eq!(
    request("http://deploy.example.test/v1", "deploy.example.test", bearer()),
    Err(HttpError::InsecureBearer)
  );
  assert!(request("http://localhost:8080/v1", "localhost", HttpAuth::None).is_ok());
}

#[test]
fn tool_http_request_mapping_locations_are_validated() {
  assert!(with_input(RequestTarget::Header("X-Revision".into())).is_ok());
  assert!(with_input(RequestTarget::Query("rev".into())).is_ok());
  for target in [
    RequestTarget::Header("X Revision".into()),
    RequestTarget::Query(String::new()),
    RequestTarget::Body("deployment..revision".into()),
  ] {
    assert_eq!(with_input(target), Err(HttpError::InvalidLocation(field("value"))));
  }
}

#[test]
fn tool_http_request_inputs_cannot_set_the_authorization_header() {
  assert_eq!(
    with_input(RequestTarget::Header("authorization".into())),
    Err(HttpError::ReservedHeader(field("value")))
  );
}

#[test]
fn tool_field_names_are_identifiers() {
  assert!(FieldName::new("preview_url").is_ok());
  for invalid in ["", "PreviewUrl", "1st", "preview-url"] {
    assert!(FieldName::new(invalid).is_err(), "{invalid}");
  }
}

#[test]
fn tool_database_actions_need_a_query_and_credentialed_tools_a_policy() {
  assert_eq!(
    DatabaseQuery::new(SecretId::generate(), String::new()),
    Err(ToolError::BlankQuery)
  );
  let query = DatabaseQuery::new(SecretId::generate(), "SELECT 1".into()).unwrap();
  assert_eq!(
    Tool::new("", ToolAction::DatabaseQuery(query), None),
    Err(ToolError::AccessPolicyRequired)
  );
  let http = request("https://deploy.example.test/v1", "deploy.example.test", bearer()).unwrap();
  assert_eq!(
    Tool::new("", ToolAction::Http(http), None),
    Err(ToolError::AccessPolicyRequired)
  );
}

#[test]
fn tool_access_policy_denies_everyone_not_listed() {
  let deploy_pipeline = ResourceId::generate();
  let review_pipeline = ResourceId::generate();
  let deploy = StepKey::new("deploy").unwrap();
  let policy = policy(BTreeMap::from([
    (deploy_pipeline, AllowedSteps::Only(BTreeSet::from([deploy.clone()]))),
    (review_pipeline, AllowedSteps::All),
  ]))
  .unwrap();
  assert!(policy.allows(deploy_pipeline, &deploy));
  assert!(!policy.allows(deploy_pipeline, &StepKey::new("build").unwrap()));
  assert!(policy.allows(review_pipeline, &StepKey::new("build").unwrap()));
  assert!(!policy.allows(ResourceId::generate(), &deploy));
}

#[test]
fn tool_access_policy_rejects_an_empty_step_selection() {
  let pipeline = ResourceId::generate();
  assert_eq!(
    policy(BTreeMap::from([(pipeline, AllowedSteps::Only(BTreeSet::new()))])),
    Err(AccessPolicyError::NoStepsSelected(pipeline))
  );
}

#[test]
fn tool_body_round_trips_and_revalidates_on_read() {
  let access = AccessPolicy::new(
    BTreeMap::from([(ResourceId::generate(), AllowedSteps::All)]),
    Name::new("Preview").unwrap(),
    ApprovalMode::OncePerRun,
    AccessWindow::Minutes(NonZeroU32::new(15).unwrap()),
    DeniedBehavior::FailStep,
  )
  .unwrap();
  let action = ToolAction::Http(request("https://deploy.example.test/v1", "deploy.example.test", bearer()).unwrap());
  let tool = Tool::new("Deploy a preview.", action, Some(access)).unwrap();
  let json = serde_json::to_string(&tool).unwrap();
  assert_eq!(serde_json::from_str::<Tool>(&json).unwrap(), tool);

  let moved_host = json.replace("https://deploy.example.test", "https://evil.example.test");
  let error = serde_json::from_str::<Tool>(&moved_host).unwrap_err();
  assert!(error.to_string().contains("not the allowed host"), "{error}");

  let database = Tool::new(
    "",
    ToolAction::DatabaseQuery(DatabaseQuery::new(SecretId::generate(), "SELECT 1".into()).unwrap()),
    tool.access().cloned(),
  )
  .unwrap();
  let json = serde_json::to_string(&database).unwrap();
  assert_eq!(serde_json::from_str::<Tool>(&json).unwrap(), database);
}
