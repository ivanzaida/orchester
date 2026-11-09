use std::{
  collections::BTreeMap,
  ffi::OsString,
  path::{Path, PathBuf},
};

use orchester_core::{
  connection::invocation::Platform,
  id::RunId,
  resource::{CommandArg, CommandEnvironment, VariableName},
  secret::SecretName,
  service::{
    RelativeFolder, RunValues, Service, ServiceName, ServiceValue, ServiceVariables, UiDriver,
    cdp::{CdpUnsupported, WEBVIEW2_ARGUMENTS, cdp_supported},
  },
};

use super::service;

fn variable(value: &str) -> VariableName {
  VariableName::new(value).expect("a variable name")
}

fn values() -> RunValues {
  let run = "0190f2a4-7b3c-7d4e-8f00-1234567890ab"
    .parse::<RunId>()
    .expect("a run id");
  RunValues::new(run, 4100, "http://127.0.0.1:4100")
}

/// The program starts in its working folder with its filled arguments, the agent's part of the environment, the
/// project's variables and its own plain ones on top; a secret stays a name for the runner to read.
#[test]
fn service_process_fills_arguments_and_variables_and_keeps_secrets_as_names() {
  let variables = ServiceVariables::new(BTreeMap::from([
    (variable("PORT"), ServiceValue::Plain("{{port}}".to_owned())),
    (variable("MODE"), ServiceValue::Plain("own".to_owned())),
    (
      variable("API_TOKEN"),
      ServiceValue::Secret(SecretName::new("TEAM_TOKEN").expect("a secret name")),
    ),
  ]))
  .expect("variables");
  let arguments =
    ["--port", "{{port}}", "--base", "{{url}}/api"].map(|value| CommandArg::new(value).expect("an argument"));
  let web = service("web", &[])
    .with_arguments(arguments.to_vec())
    .with_working_folder(RelativeFolder::new("web/team").expect("a folder"))
    .with_variables(variables);
  let project = CommandEnvironment::new(BTreeMap::from([
    (variable("MODE"), "project".to_owned()),
    (variable("NODE_ENV"), "test".to_owned()),
  ]))
  .expect("project variables");
  let worktree = Path::new("/runs/1");
  let inherited = [(OsString::from("PATH"), OsString::from("/usr/bin"))];
  let process = web
    .process(
      (Path::new("/usr/bin/npm"), worktree),
      (inherited, &project),
      Platform::MacOs,
      &values(),
    )
    .expect("filled");
  let command = &process.command;
  assert_eq!(command.program(), Path::new("/usr/bin/npm"));
  assert_eq!(
    command.args(),
    ["--port", "4100", "--base", "http://127.0.0.1:4100/api"]
  );
  assert_eq!(
    command.current_dir(),
    Some(PathBuf::from("/runs/1").join("web/team").as_path())
  );
  assert!(!command.inherits_env());
  let env: BTreeMap<&str, &str> = command
    .env()
    .iter()
    .map(|(name, value)| (name.as_str(), value.as_str()))
    .collect();
  assert_eq!(env.get("PORT"), Some(&"4100"));
  assert_eq!(env.get("MODE"), Some(&"own"), "the service's own value wins");
  assert_eq!(env.get("NODE_ENV"), Some(&"test"));
  assert_eq!(env.get("PATH"), Some(&"/usr/bin"));
  assert_eq!(env.get("API_TOKEN"), None, "a secret is never a plain variable");
  assert_eq!(
    process.secrets,
    [(
      variable("API_TOKEN"),
      SecretName::new("TEAM_TOKEN").expect("a secret name")
    )]
  );
  let windows = web
    .process(
      (Path::new("C:/tools/npm.cmd"), worktree),
      (Vec::new(), &project),
      Platform::Windows,
      &values(),
    )
    .expect("filled");
  assert!(
    windows
      .command
      .env()
      .contains(&("NoDefaultCurrentDirectoryInExePath".to_owned(), "1".to_owned()))
  );
}

/// The service's own plain `PATH` is where its program is looked up, then the project's, then the runner's.
#[test]
fn service_search_path_prefers_its_own_then_the_projects() {
  let inherited = Some(OsString::from("/usr/bin"));
  let bare = service("web", &[]);
  assert_eq!(
    bare.search_path(&CommandEnvironment::default(), inherited.clone()),
    inherited
  );
  let project = CommandEnvironment::new(BTreeMap::from([(variable("PATH"), "/project/bin".to_owned())]))
    .expect("project variables");
  assert_eq!(
    bare.search_path(&project, inherited.clone()),
    Some(OsString::from("/project/bin"))
  );
  let own = bare.with_variables(
    ServiceVariables::new(BTreeMap::from([(
      variable("Path"),
      ServiceValue::Plain("/own/bin".to_owned()),
    )]))
    .expect("variables"),
  );
  assert_eq!(own.search_path(&project, inherited), Some(OsString::from("/own/bin")));
}

/// The value of `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` the process of `service` gets on `platform`, if any.
fn webview2_arguments(service: &Service, platform: Platform) -> Option<String> {
  let process = service
    .process(
      (Path::new("C:/apps/app.exe"), Path::new("/runs/1")),
      (Vec::new(), &CommandEnvironment::default()),
      platform,
      &values(),
    )
    .expect("filled");
  process
    .command
    .env()
    .iter()
    .find(|(name, _)| name == WEBVIEW2_ARGUMENTS)
    .map(|(_, value)| value.clone())
}

/// A service with the `cdp` driver gets its own port as a WebView2 app's debugging port on Windows, after the browser
/// arguments its own variable gives; no other service gets it, and macOS, which has no WebView2, sets nothing.
#[test]
fn service_cdp_process_gets_the_webview2_debugging_port() {
  let app = service("app", &[]).with_ui_driver(UiDriver::Cdp);
  assert_eq!(
    webview2_arguments(&app, Platform::Windows).as_deref(),
    Some("--remote-debugging-port=4100")
  );
  assert_eq!(webview2_arguments(&app, Platform::MacOs), None);
  let own = app.with_variables(
    ServiceVariables::new(BTreeMap::from([(
      variable(WEBVIEW2_ARGUMENTS),
      ServiceValue::Plain("--lang=fr".to_owned()),
    )]))
    .expect("variables"),
  );
  assert_eq!(
    webview2_arguments(&own, Platform::Windows).as_deref(),
    Some("--lang=fr --remote-debugging-port=4100"),
    "the service's own arguments are kept, the port after them"
  );
  for driver in [UiDriver::None, UiDriver::Native, UiDriver::Browser, UiDriver::Mcp] {
    let other = service("app", &[]).with_ui_driver(driver);
    assert_eq!(webview2_arguments(&other, Platform::Windows), None, "{driver:?}");
  }
}

/// WKWebView on macOS has no debugging port: the `cdp` driver says so and points to the native driver there.
#[test]
fn service_cdp_driver_on_macos_says_to_use_the_native_driver() {
  let name = ServiceName::new("desktop").expect("a name");
  assert_eq!(cdp_supported(Platform::Windows, &name), Ok(()));
  let refused = cdp_supported(Platform::MacOs, &name);
  assert_eq!(refused, Err(CdpUnsupported(name)));
  let said = refused.expect_err("refused").to_string();
  assert!(said.contains("`desktop` uses the cdp UI driver"), "{said}");
  assert!(said.contains("WKWebView on macOS has none"), "{said}");
  assert!(
    said.contains("use the native driver for this service on macOS"),
    "{said}"
  );
}
