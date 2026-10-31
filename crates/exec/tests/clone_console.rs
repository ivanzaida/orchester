//! The clone console (ORC-584): its words carry no control characters, its result is read as Git's exit code or as a
//! closed window, and the program, the address and the folder reach Git as data. On Windows the real PowerShell host
//! runs with a hidden window; on macOS the Terminal script runs with `/bin/sh`. Git is `orchester-exit-with`, which
//! writes down what it was given. Opening a window a person sees, and Terminal itself, are not run here.
//!
//! `cargo test -p orchester-exec --features test-program --test clone_console clone_console_`

use std::{
  fs,
  path::{Path, PathBuf},
};

use orchester_exec::clone_console::{CloneJob, ConsoleEnd, ConsoleError, ConsoleWords, read_end, terminal_script};
use tempfile::TempDir;

const GIT: &str = env!("CARGO_BIN_EXE_orchester-exit-with");
const URL: &str = "https://github.com/acme/kontur.git";
/// A folder name full of what shells read as code.
const AWKWARD: &str = "dest & % ^ ! ' $(whoami) ; `x` ü";

fn scratch() -> TempDir {
  let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/clone-console-tests");
  fs::create_dir_all(&root).expect("a scratch folder");
  tempfile::tempdir_in(root).expect("a folder of the test's own")
}

fn words() -> ConsoleWords {
  ConsoleWords::new("Orchester · cloning Kontur", "Git did not finish", "Press Enter")
}

fn printed(args: &[&str]) -> String {
  args.iter().map(|arg| format!("{arg:?}\n")).collect()
}

#[test]
fn clone_console_words_drop_control_characters() {
  let words = ConsoleWords::new("Kon\u{1b}]0;evil\u{7}tur\n", "fa\riled", "cl\u{9b}ose");
  assert_eq!(words.title(), "Kon]0;eviltur");
  assert_eq!(words.failed(), "failed");
  assert_eq!(words.close(), "close");
}

#[test]
fn clone_console_read_end_tells_an_exit_from_a_closed_window() {
  let dir = scratch();
  let result = dir.path().join("result");
  assert_eq!(read_end(&result, || Ok(true)).expect("read"), None, "still running");
  assert_eq!(
    read_end(&result, || Ok(false)).expect("read"),
    Some(ConsoleEnd::Closed),
    "gone without a result"
  );
  fs::write(&result, "0\r\n").expect("a result");
  assert_eq!(
    read_end(&result, || panic!("a result needs no look at the host")).expect("read"),
    Some(ConsoleEnd::Exited { code: 0 })
  );
  fs::write(&result, "128").expect("a result");
  assert_eq!(
    read_end(&result, || Ok(true)).expect("read"),
    Some(ConsoleEnd::Exited { code: 128 })
  );
  fs::write(&result, "not a code").expect("a result");
  assert!(matches!(
    read_end(&result, || Ok(true)),
    Err(ConsoleError::UnreadableResult)
  ));
}

#[test]
fn clone_console_read_end_reads_a_result_written_as_the_host_ended() {
  let dir = scratch();
  let result = dir.path().join("result");
  let end = read_end(&result, || {
    fs::write(&result, "0").expect("a result");
    Ok(false)
  });
  assert_eq!(end.expect("read"), Some(ConsoleEnd::Exited { code: 0 }));
}

#[test]
fn clone_console_terminal_script_quotes_every_value() {
  let dir = scratch();
  let destination = dir.path().join(AWKWARD);
  let words = ConsoleWords::new("it's $(title)", "`failed`", "close; rm");
  let job = CloneJob {
    git: Path::new(GIT),
    url: URL,
    destination: &destination,
    scratch: dir.path(),
    words: &words,
  };
  let script = terminal_script(&job, &dir.path().join("pid"), &dir.path().join("result")).expect("a script");
  let quoted = |text: &str| format!("'{}'", text.replace('\'', r"'\''"));
  let clone = format!(
    "{} clone -- {} {}\n",
    quoted(GIT),
    quoted(URL),
    quoted(&destination.display().to_string())
  );
  assert!(script.contains(&clone), "{script}");
  assert!(script.contains(&quoted("it's $(title)")), "{script}");
  assert!(script.contains(&quoted("`failed`")), "{script}");
  assert!(script.contains("unset GIT_TERMINAL_PROMPT GCM_INTERACTIVE\n"));
  assert!(!script.contains("$(whoami)\n"), "the folder is never unquoted");
}

#[cfg(unix)]
#[test]
fn clone_console_terminal_script_runs_values_as_data() {
  use std::process::Command;

  let dir = scratch();
  let destination = dir.path().join(AWKWARD);
  let (args, result) = (dir.path().join("args"), dir.path().join("result"));
  let words = words();
  let job = CloneJob {
    git: Path::new(GIT),
    url: URL,
    destination: &destination,
    scratch: dir.path(),
    words: &words,
  };
  let script = dir.path().join("clone.command");
  fs::write(
    &script,
    terminal_script(&job, &dir.path().join("pid"), &result).expect("a script"),
  )
  .expect("written");
  let status = Command::new("/bin/sh")
    .arg(&script)
    .env("ORCHESTER_TEST_ARGS", &args)
    .env("GIT_TERMINAL_PROMPT", "0")
    .status()
    .expect("sh");
  assert!(status.success());
  let destination = destination.display().to_string();
  assert_eq!(
    fs::read_to_string(&args).expect("git ran"),
    printed(&["clone", "--", URL, &destination])
  );
  assert_eq!(fs::read_to_string(&result).expect("a result").trim(), "0");
  assert!(dir.path().join("pid").exists(), "the script wrote its process id");
}

#[cfg(windows)]
mod windows {
  use std::{
    fs,
    os::windows::process::CommandExt,
    path::Path,
    process::Stdio,
    thread,
    time::{Duration, Instant},
  };

  use orchester_exec::clone_console::{CloneJob, ConsoleEnd, read_end, windows::command};

  use super::{AWKWARD, GIT, URL, printed, scratch, words};

  const CREATE_NO_WINDOW: u32 = 0x0800_0000;

  /// Runs the console for `job` with a hidden window and Git's exit code `code`, until its result is written; the
  /// host is ended then, as it waits for Enter after a failure. What Git was given, and how it ended.
  fn hidden(job: &CloneJob<'_>, code: u8) -> (String, ConsoleEnd) {
    let (args, result) = (job.scratch.join("args"), job.scratch.join("result"));
    let mut process = command(job, &result);
    process
      .creation_flags(CREATE_NO_WINDOW)
      .stdin(Stdio::null())
      .stdout(Stdio::null())
      .stderr(Stdio::null())
      .env("ORCHESTER_TEST_ARGS", &args)
      .env("ORCHESTER_TEST_EXIT_CODE", code.to_string());
    let mut host = process.spawn().expect("PowerShell starts");
    let deadline = Instant::now() + Duration::from_secs(60);
    let end = loop {
      let looked = read_end(&result, || Ok(host.try_wait().expect("the host").is_none())).expect("a result");
      if let Some(end) = looked {
        break end;
      }
      assert!(Instant::now() < deadline, "the console never ended");
      thread::sleep(Duration::from_millis(50));
    };
    if host.try_wait().expect("the host").is_none() {
      host.kill().expect("the host waiting for Enter ends");
    }
    (fs::read_to_string(&args).expect("git ran"), end)
  }

  #[test]
  fn clone_console_windows_passes_values_as_data() {
    let dir = scratch();
    let destination = dir.path().join(AWKWARD);
    let words = words();
    let job = CloneJob {
      git: Path::new(GIT),
      url: URL,
      destination: &destination,
      scratch: dir.path(),
      words: &words,
    };
    let (given, end) = hidden(&job, 0);
    let destination = destination.display().to_string();
    assert_eq!(given, printed(&["clone", "--", URL, &destination]));
    assert_eq!(end, ConsoleEnd::Exited { code: 0 });
    assert!(
      !dir.path().join("result.tmp").exists(),
      "the result was renamed into place"
    );
  }

  #[test]
  fn clone_console_windows_records_the_exit_code() {
    let dir = scratch();
    let destination = dir.path().join("kontur");
    let words = words();
    let job = CloneJob {
      git: Path::new(GIT),
      url: URL,
      destination: &destination,
      scratch: dir.path(),
      words: &words,
    };
    let (_, end) = hidden(&job, 3);
    assert_eq!(end, ConsoleEnd::Exited { code: 3 });
  }
}
