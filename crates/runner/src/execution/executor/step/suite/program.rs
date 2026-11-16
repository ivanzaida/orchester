//! Running a suite's program for `run_tests`: found as a command tool's program is, in its working folder inside the
//! worktree, without a shell, with the filter in its arguments and its secrets read from the keychain, stopped with
//! everything it started at the suite's timeout. Its report is read from the program's output or from the file it
//! wrote, which is removed before the program starts, so a report left by an earlier run is never read as this one's.
//! A report that cannot be read is kept as such, with the program's last lines, never as a pass.

use std::{fs, io, path::Path};

use orchester_core::{
  command::{CommandError, SecretVariable, redact_secrets},
  connection::invocation::Platform,
  orchester_tools::{OutputKept, ProgramOutput},
  resource::{TestFilter, TestSuite},
  test_report::{self, SuiteRunEnd, last_lines, read_report},
};
use orchester_exec::run_capped;

use super::{StepSuites, store::shown};
use crate::execution::executor::step::service::project_secret;

/// The most bytes of each stream kept: a printed report is read whole, and one cut off is unreadable.
const OUTPUT_CAP: usize = 32 * 1024 * 1024;
/// The largest report file read.
const REPORT_FILE_CAP: u64 = 64 * 1024 * 1024;

/// How a suite's program ended, and its report.
pub(super) struct Finished {
  pub(super) exit_code: Option<i32>,
  pub(super) end: SuiteRunEnd,
}

impl StepSuites {
  /// Runs `suite` with `filter`; why it could not start when it did not.
  pub(super) fn execute(&self, suite: &TestSuite, filter: Option<&TestFilter>) -> Result<Finished, String> {
    let folder = suite.folder_in(&self.worktree);
    if !folder.is_dir() {
      return Err(format!(
        "the working folder `{}` of the suite is not in the worktree",
        shown(&folder)
      ));
    }
    let platform = Platform::current();
    let inherited_path = self
      .inherited
      .iter()
      .find(|(variable, _)| variable.to_string_lossy().eq_ignore_ascii_case("PATH"))
      .map(|(_, value)| value.clone());
    let path = suite.search_path(&self.project, inherited_path);
    let program = suite
      .program_candidates(&self.worktree, path.as_deref(), platform)
      .into_iter()
      .find(|candidate| candidate.is_file())
      .ok_or_else(|| {
        format!(
          "the program `{}` of the suite was not found",
          suite.command().program().as_str()
        )
      })?;
    suite
      .command()
      .program()
      .check_place(&program, &self.root)
      .map_err(|_| {
        "the program of the suite now leads into the project's repository or off this computer".to_owned()
      })?;
    let process = suite.process(
      (&program, &self.worktree),
      (self.inherited.iter().cloned(), &self.project),
      platform,
      filter,
    );
    let mut command = process.command;
    let mut secrets = Vec::new();
    for (variable, secret) in &process.secrets {
      let secret = project_secret((self.project_id, self.computer.as_ref()), variable, secret)?;
      command = command.with_secret_env(secret.clone());
      secrets.push(secret);
    }
    let report_file = suite.report_file_in(&self.worktree);
    if let Some(file) = &report_file {
      match fs::remove_file(file) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("the old report `{}` cannot be removed: {error}", shown(file))),
      }
    }
    let output = match run_capped(&command, suite.timeout().duration(), (OUTPUT_CAP, OutputKept::Head)) {
      Ok(output) => output,
      Err(CommandError::TimedOut) => {
        return Ok(Finished {
          exit_code: None,
          end: SuiteRunEnd::TimedOut {
            timeout_secs: suite.timeout().secs(),
          },
        });
      }
      Err(error) => return Err(format!("the program of the suite did not run: {error}")),
    };
    Ok(self.read(suite, report_file.as_deref(), &output, &secrets))
  }

  /// The report of a program that ended with `output`, read from `report_file` or from what it printed, with the
  /// values of `secrets` removed.
  fn read(
    &self,
    suite: &TestSuite,
    report_file: Option<&Path>,
    output: &ProgramOutput,
    secrets: &[SecretVariable],
  ) -> Finished {
    let stdout = redact_secrets(&output.stdout, secrets);
    let text = match report_file {
      Some(file) => self.report_text(file),
      None if output.left_out > 0 => Err(format!(
        "the printed report is longer than {} MiB",
        OUTPUT_CAP / (1024 * 1024)
      )),
      None => Ok(stdout.clone()),
    };
    let format = test_report::ReportFormat::from(suite.report().format());
    let report =
      text.and_then(|text| read_report(format, &redact_secrets(&text, secrets)).map_err(|error| error.to_string()));
    let end = match report {
      Ok(report) => SuiteRunEnd::Read { report },
      Err(error) => SuiteRunEnd::Unreadable {
        error,
        last_lines: last_lines(&stdout, &redact_secrets(&output.stderr, secrets)),
      },
    };
    Finished {
      exit_code: output.code,
      end,
    }
  }

  /// The text of the report file `file`, which must stay inside the worktree.
  fn report_text(&self, file: &Path) -> Result<String, String> {
    let named = shown(file);
    let resolved = match fs::canonicalize(file) {
      Ok(resolved) => resolved,
      Err(error) if error.kind() == io::ErrorKind::NotFound => {
        return Err(format!("the program wrote no report at `{named}`"));
      }
      Err(error) => return Err(format!("the report `{named}` cannot be read: {error}")),
    };
    let worktree = fs::canonicalize(&self.worktree).map_err(|error| format!("the worktree cannot be read: {error}"))?;
    if !resolved.starts_with(&worktree) {
      return Err(format!("the report `{named}` leads out of the worktree"));
    }
    let size = fs::metadata(&resolved)
      .map_err(|error| format!("the report `{named}` cannot be read: {error}"))?
      .len();
    if size > REPORT_FILE_CAP {
      return Err(format!(
        "the report `{named}` is larger than {} MiB",
        REPORT_FILE_CAP / (1024 * 1024)
      ));
    }
    let bytes = fs::read(&resolved).map_err(|error| format!("the report `{named}` cannot be read: {error}"))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
  }
}
