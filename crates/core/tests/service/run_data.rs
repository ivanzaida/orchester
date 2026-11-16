//! A service's per-run data: its run folder in the run's storage, reached through `{{data}}`, which only such a
//! service may use; and a database container's `docker run` arguments, its name and the notes that name it.

use std::{collections::BTreeMap, path::Path};

use orchester_core::{
  id::{ProjectId, RunId},
  resource::{CommandArg, CommandProgram, VariableName},
  secret::SecretName,
  service::{
    ContainerName, ContainerStart, Placeholder, PlaceholderError, PlaceholderPlace, Readiness, ReadyCheck,
    ReadyTimeout, RunData, RunFolder, RunValues, Service, ServiceValue, ServiceVariables, check_placeholders,
    container_notes_folder, run_data_folder,
  },
};

use super::{name, service};

const RUN: &str = "0190f2a4-7b3c-7d4e-8f00-1234567890ab";
const PROJECT: &str = "0190f2a4-7b3c-7d4e-8f00-0000000000aa";

fn run() -> RunId {
  RUN.parse().expect("a run id")
}

fn arguments(values: &[&str]) -> Vec<CommandArg> {
  values
    .iter()
    .map(|value| CommandArg::new(value).expect("an argument"))
    .collect()
}

fn with_folder(folder: &str) -> Service {
  service("web", &[]).with_run_data(RunData::RunFolder(RunFolder::new(folder).expect("a run folder")))
}

#[test]
fn run_data_folders_lie_in_the_runs_storage() {
  let repo = Path::new("repo");
  let storage = repo.join(".orchester").join("runs").join(RUN);
  assert_eq!(run_data_folder(repo, run()), storage.join("data"));
  assert_eq!(container_notes_folder(repo, run()), storage.join("containers"));
  let folder = RunFolder::new("team/web").expect("a run folder");
  assert_eq!(
    folder.path_in(repo, run()),
    storage.join("data").join("team").join("web")
  );
}

#[test]
fn run_data_placeholder_is_the_run_folder_of_a_service_that_has_one() {
  let web = with_folder("web").with_arguments(arguments(&["--data={{data}}", "{{port}}"]));
  assert_eq!(check_placeholders(&web, std::slice::from_ref(&web)), Ok(()));
  let values = RunValues::new(run(), 4000, "http://127.0.0.1:4000").with_data("/repo/data/web");
  assert_eq!(
    values.arguments(&web),
    Ok(vec!["--data=/repo/data/web".to_owned(), "4000".to_owned()])
  );
  assert_eq!(
    Placeholder::find_all("{{data}}", &PlaceholderPlace::UiStart),
    Ok(vec![Placeholder::Data])
  );
  assert_eq!(Placeholder::Data.to_string(), "{{data}}");
}

#[test]
fn run_data_placeholder_is_refused_without_a_run_folder() {
  let plain = service("web", &[]).with_arguments(arguments(&["--data={{data}}"]));
  let error = check_placeholders(&plain, std::slice::from_ref(&plain)).expect_err("no run folder");
  assert_eq!(
    error,
    PlaceholderError::NoRunFolder {
      place: PlaceholderPlace::Argument(0)
    }
  );
  assert_eq!(
    error.to_string(),
    "argument 1 uses `{{data}}`, but this service's per-run data is not a run folder"
  );
  let values = RunValues::new(run(), 4000, "http://127.0.0.1:4000");
  assert_eq!(
    values.arguments(&plain),
    Err(PlaceholderError::NoValue {
      place: PlaceholderPlace::Argument(0),
      placeholder: Placeholder::Data
    })
  );
}

#[test]
fn run_data_container_names_are_of_their_run_service_and_port() {
  let name = ContainerName::of(run(), &name("postgres"), 41_234);
  assert_eq!(name.as_str(), format!("orchester-{RUN}-postgres-41234"));
  assert_eq!(ContainerName::noted(run(), name.as_str()), Some(name));
  let other: RunId = "0190f2a4-7b3c-7d4e-8f00-ffffffffffff".parse().expect("a run id");
  for noted in [
    format!("orchester-{other}-postgres-41234"),
    format!("orchester-{RUN}-postgres"),
    format!("orchester-{RUN}-Postgres-41234"),
    format!("orchester-{RUN}-postgres-99999"),
    format!("orchester-{RUN}-postgres-041234"),
    "my-own-database".to_owned(),
  ] {
    assert_eq!(ContainerName::noted(run(), &noted), None, "{noted}");
  }
}

#[test]
fn run_data_container_runs_the_image_on_the_services_port() {
  let readiness = Readiness::new(ReadyCheck::PortOpen, ReadyTimeout::from_secs(30).expect("in range"));
  let variables = ServiceVariables::new(BTreeMap::from([
    (
      VariableName::new("POSTGRES_DB").expect("a name"),
      ServiceValue::Plain("run-{{run}}".to_owned()),
    ),
    (
      VariableName::new("POSTGRES_PASSWORD").expect("a name"),
      ServiceValue::Secret(SecretName::new("DB_PASSWORD").expect("a name")),
    ),
  ]))
  .expect("named once");
  let postgres = Service::new(
    name("postgres"),
    CommandProgram::new("postgres:16-alpine").expect("an image"),
    readiness,
  )
  .with_arguments(arguments(&["-c", "fsync=off"]))
  .with_variables(variables)
  .with_run_data(RunData::DatabaseContainer);
  let container = ContainerName::of(run(), postgres.name(), 41_234);
  let start = ContainerStart {
    name: &container,
    project: PROJECT.parse::<ProjectId>().expect("a project id"),
    run: run(),
    host_port: 41_234,
    container_port: 5432,
  };
  let values = RunValues::new(run(), 41_234, "http://127.0.0.1:41234");
  let process = postgres.container_process(&start, &values).expect("arguments");
  let expected: Vec<String> = [
    "run",
    "--rm",
    "--name",
    container.as_str(),
    "--label",
    &format!("orchester.project={PROJECT}"),
    "--label",
    &format!("orchester.run={RUN}"),
    "--label",
    "orchester.service=postgres",
    "--publish",
    "127.0.0.1:41234:5432",
    "--env",
    &format!("POSTGRES_DB=run-{RUN}"),
    "--env",
    "POSTGRES_PASSWORD",
    "postgres:16-alpine",
    "-c",
    "fsync=off",
  ]
  .into_iter()
  .map(str::to_owned)
  .collect();
  assert_eq!(process.arguments, expected);
  assert_eq!(
    process.secrets,
    [(
      VariableName::new("POSTGRES_PASSWORD").expect("a name"),
      SecretName::new("DB_PASSWORD").expect("a name")
    )]
  );
}
