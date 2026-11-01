//! The clone console of the test window (`stand_ins`): instead of a window the person sees, it does in the
//! destination what a test scripts and ends as Git would, so every way a clone ends can be reached without a console,
//! a network or a sign-in.

use std::{
  fs,
  path::Path,
  process::{Command, Stdio},
  sync::Arc,
};

use orchester_exec::clone_console::ConsoleEnd;
use orchester_workspace::sharing::project::clone::{CloneRefusal, LaunchJob};
use parking_lot::{Condvar, Mutex};

/// What the stand-in console does with a clone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConsoleScript {
  /// No git is installed.
  GitMissing,
  /// The console window does not open.
  NotOpened,
  /// Git ends with this code, nothing written.
  Exits(i32),
  /// The window is closed before Git ends, nothing written.
  Closes,
  /// Git makes the destination a checkout whose `origin` is the job's address, and ends with 0.
  Clones,
  /// Git leaves a half-written `.git` in the destination first, then ends as the script says.
  WritesThen(Box<Self>),
  /// The console waits until [`Console::release`], then ends as the script says.
  Holds(Box<Self>),
}

/// The stand-in console: its script, the jobs it was given, and the hold [`ConsoleScript::Holds`] waits on.
#[derive(Clone)]
pub struct Console {
  script: Arc<Mutex<ConsoleScript>>,
  jobs: Arc<Mutex<Vec<LaunchJob>>>,
  held: Arc<(Mutex<bool>, Condvar)>,
}

impl Default for Console {
  fn default() -> Self {
    Self {
      script: Arc::new(Mutex::new(ConsoleScript::Closes)),
      jobs: Arc::default(),
      held: Arc::default(),
    }
  }
}

impl Console {
  pub fn script(&self, script: ConsoleScript) {
    *self.script.lock() = script;
  }

  pub fn jobs(&self) -> Vec<LaunchJob> {
    self.jobs.lock().clone()
  }

  /// Lets a console that holds end.
  pub fn release(&self) {
    let (released, waiting) = &*self.held;
    *released.lock() = true;
    waiting.notify_all();
  }

  /// Runs `job` as the script says. Blocking, as the real console is.
  pub fn run(&self, job: &LaunchJob) -> Result<ConsoleEnd, CloneRefusal> {
    self.jobs.lock().push(job.clone());
    let script = self.script.lock().clone();
    self.play(&script, job)
  }

  fn play(&self, script: &ConsoleScript, job: &LaunchJob) -> Result<ConsoleEnd, CloneRefusal> {
    match script {
      ConsoleScript::GitMissing => Err(CloneRefusal::GitMissing),
      ConsoleScript::NotOpened => Err(CloneRefusal::NotOpened),
      ConsoleScript::Exits(code) => Ok(ConsoleEnd::Exited { code: *code }),
      ConsoleScript::Closes => Ok(ConsoleEnd::Closed),
      ConsoleScript::Clones => {
        checkout(&job.destination, &job.url);
        Ok(ConsoleEnd::Exited { code: 0 })
      }
      ConsoleScript::WritesThen(then) => {
        let git = job.destination.join(".git");
        fs::create_dir_all(&git).expect("the stand-in console writes in the test's own folder");
        fs::write(git.join("HEAD"), "ref: refs/heads/main\n").expect("the stand-in console writes its file");
        self.play(then, job)
      }
      ConsoleScript::Holds(then) => {
        let (released, waiting) = &*self.held;
        let mut released = released.lock();
        while !*released {
          waiting.wait(&mut released);
        }
        drop(released);
        self.play(then, job)
      }
    }
  }
}

/// `folder` made a git checkout whose `origin` is `url`, as a clone leaves it.
fn checkout(folder: &Path, url: &str) {
  fs::create_dir_all(folder).expect("the stand-in console makes the clone's folder");
  for args in [&["init", "-b", "main"][..], &["remote", "add", "origin", url]] {
    let status = Command::new("git")
      .args(args)
      .current_dir(folder)
      .stdout(Stdio::null())
      .stderr(Stdio::null())
      .status()
      .expect("git runs for the stand-in console");
    assert!(status.success(), "git {args:?} in the stand-in console");
  }
}
