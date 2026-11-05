//! Where `run_tests` keeps its results: `<run's storage>/tests/<t1, t2, …>/`, each with `result.json` and the copied
//! artifacts under `artifacts/`. The run's storage outlives the workspace (ORC-114), so later steps and `test_output`
//! read a result back.

use std::{
  fs, io,
  path::{Path, PathBuf},
};

use orchester_core::test_report::{SuiteRun, TestRunId};

/// The folder of the run's storage the test runs go to.
const TESTS: &str = "tests";
const RESULT: &str = "result.json";
const ARTIFACTS: &str = "artifacts";

/// The folder of one test run, made for it.
pub(super) struct RunFolder {
  id: TestRunId,
  path: PathBuf,
}

impl RunFolder {
  /// A new folder in `storage` for the next test run of the run: the one after the highest there is.
  pub(super) fn create(storage: &Path) -> Result<Self, String> {
    let tests = storage.join(TESTS);
    let cannot = |error: io::Error| format!("the run's storage cannot hold the test results: {error}");
    fs::create_dir_all(&tests).map_err(cannot)?;
    let highest = fs::read_dir(&tests)
      .map_err(cannot)?
      .filter_map(|entry| entry.ok()?.file_name().to_str()?.parse::<TestRunId>().ok())
      .max();
    let mut id = highest.map_or(TestRunId::FIRST, TestRunId::next);
    loop {
      let path = tests.join(id.to_string());
      match fs::create_dir(&path) {
        Ok(()) => return Ok(Self { id, path }),
        // Another call of the run took this one meanwhile.
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => id = id.next(),
        Err(error) => return Err(cannot(error)),
      }
    }
  }

  pub(super) const fn id(&self) -> TestRunId {
    self.id
  }

  /// Where the run's artifacts are copied.
  pub(super) fn artifacts(&self) -> PathBuf {
    self.path.join(ARTIFACTS)
  }

  /// Keeps `run` as the folder's result; the file's path, with `/`.
  pub(super) fn save(&self, run: &SuiteRun) -> Result<String, String> {
    let file = self.path.join(RESULT);
    let json = serde_json::to_vec_pretty(run).map_err(|error| format!("the test result cannot be written: {error}"))?;
    fs::write(&file, json).map_err(|error| format!("the test result cannot be kept in the run's storage: {error}"))?;
    Ok(shown(&file))
  }

  /// Removes the folder of a test run whose program never ran.
  pub(super) fn discard(self) {
    if let Err(error) = fs::remove_dir_all(&self.path) {
      tracing::warn!(%error, run = %self.id, "the folder of a test run that did not start stays");
    }
  }
}

/// The test run `id` kept in `storage`.
pub(super) fn load(storage: &Path, id: TestRunId) -> Result<SuiteRun, String> {
  let file = storage.join(TESTS).join(id.to_string()).join(RESULT);
  let text = match fs::read(&file) {
    Ok(text) => text,
    Err(error) if error.kind() == io::ErrorKind::NotFound => {
      return Err(format!(
        "this run has no test run {id}; run_tests names the ones it made"
      ));
    }
    Err(error) => return Err(format!("the test run {id} cannot be read: {error}")),
  };
  serde_json::from_slice(&text).map_err(|error| format!("the stored test run {id} cannot be read: {error}"))
}

/// `path` as answers name files: absolute, with `/` between its parts.
pub(super) fn shown(path: &Path) -> String {
  path.to_string_lossy().replace('\\', "/")
}
