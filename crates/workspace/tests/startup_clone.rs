//! Cloning a shared project from *Link a folder* (ORC-584): the address is the hint's `https://…git`, the destination
//! must be new or empty, and a clone counts only when Git ended with 0 and the folder is a checkout of the project.
//! What a failed or closed clone left is named and kept, an occupied destination opens no console, and a second press
//! follows the clone that runs. The console is a stand-in here (`Clones::with_launcher`) that does in the destination
//! what Git would; the console itself is `orchester-exec`'s `clone_console` tests.
//!
//! `cargo test -p orchester-workspace --test startup_clone startup_clone_`

use std::{
  fs,
  path::{Path, PathBuf},
  pin::Pin,
  process::Command,
  sync::{Arc, mpsc},
  time::{Duration, Instant},
};

use orchester_api::SharedProject;
use orchester_core::{
  id::{ProjectId, ServerId},
  repository::RepositoryIdentity,
  sharing::MemberRole,
};
use orchester_exec::{
  clone_console::{ConsoleEnd, ConsoleWords},
  programs::Programs,
  search_path::SearchPath,
};
use orchester_workspace::sharing::project::{
  clone::{
    CloneEnd, CloneRefusal, CloneRequest, CloneState, Clones, Destination, LaunchJob, check_destination, find_git,
  },
  folder::FolderCheck,
  landing::{clone_command, clone_url},
};
use parking_lot::Mutex;
use tempfile::TempDir;
use tokio::runtime::Runtime;

const HINT: &str = "github.com/acme/kontur";
const URL: &str = "https://github.com/acme/kontur.git";

fn identity(text: &str) -> RepositoryIdentity {
  RepositoryIdentity::parse(text).expect("identity")
}

fn git(root: &Path, args: &[&str]) {
  let output = Command::new("git").args(args).current_dir(root).output().expect("git");
  assert!(
    output.status.success(),
    "git {args:?}: {}",
    String::from_utf8_lossy(&output.stderr)
  );
}

/// What a stand-in console does in the destination before it ends.
fn checkout(folder: &Path, origin: &str) {
  fs::create_dir_all(folder).expect("the clone's folder");
  git(folder, &["init", "-b", "main"]);
  git(folder, &["remote", "add", "origin", origin]);
}

/// Folders under the repository's `.tmp/`, the project Kontur with the hint `github.com/acme/kontur`, and a runtime.
struct Setup {
  scratch: TempDir,
  server: ServerId,
  project: SharedProject,
  runtime: Runtime,
}

impl Setup {
  fn new() -> Self {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/startup-clone-tests");
    fs::create_dir_all(&root).expect("scratch");
    Self {
      scratch: tempfile::tempdir_in(&root).expect("scratch"),
      server: ServerId::generate(),
      project: SharedProject {
        id: ProjectId::generate(),
        slug: "kontur".to_owned(),
        address: "https://orchester.example.com/kontur".to_owned(),
        key: "KON".to_owned(),
        name: "Kontur".to_owned(),
        remote_hint: Some(identity(HINT)),
        role: Some(MemberRole::Member),
        next_number: 1,
      },
      runtime: Runtime::new().expect("runtime"),
    }
  }

  fn path(&self, name: &str) -> PathBuf {
    self.scratch.path().join(name)
  }

  fn request(&self, destination: &Path) -> CloneRequest {
    CloneRequest {
      server: self.server,
      project: self.project.clone(),
      destination: destination.to_owned(),
      words: ConsoleWords::new("Orchester · cloning Kontur", "Git did not finish", "Press Enter"),
    }
  }

  fn start(&self, clones: &Clones, destination: &Path) -> Result<CloneEnd, CloneRefusal> {
    self
      .runtime
      .block_on(clones.start(Programs::processes(), self.request(destination)))
  }
}

/// Clones whose stand-in console runs `console` and keeps each job it was given.
fn recorded(
  console: impl Fn(&LaunchJob) -> Result<ConsoleEnd, CloneRefusal> + Send + Sync + 'static,
) -> (Clones, Arc<Mutex<Vec<LaunchJob>>>) {
  let jobs: Arc<Mutex<Vec<LaunchJob>>> = Arc::default();
  let kept = jobs.clone();
  let clones = Clones::with_launcher(move |job| {
    kept.lock().push(job.clone());
    console(job)
  });
  (clones, jobs)
}

#[test]
fn startup_clone_url_is_the_hints_https_address() {
  assert_eq!(clone_url(&identity(HINT)).as_deref(), Some(URL));
  assert_eq!(
    clone_command(&identity(HINT)).as_deref(),
    Some("git clone https://github.com/acme/kontur.git")
  );
  assert_eq!(clone_url(&identity("github.com/acme/$(whoami)")), None);
  assert_eq!(clone_url(&identity("github.com/acme/ko;ntur")), None);
}

#[test]
fn startup_clone_destination_says_what_a_folder_is() {
  let setup = Setup::new();
  let new = setup.path("kontur");
  assert_eq!(
    check_destination(&new),
    Destination::New {
      parent: setup.scratch.path().to_owned()
    }
  );
  let empty = setup.path("empty");
  fs::create_dir(&empty).expect("a folder");
  assert_eq!(check_destination(&empty), Destination::EmptyFolder);
  let hidden = setup.path("hidden");
  fs::create_dir(&hidden).expect("a folder");
  fs::write(hidden.join(".keep"), "").expect("a hidden file");
  assert_eq!(
    check_destination(&hidden),
    Destination::Occupied,
    "a hidden file counts"
  );
  let file = setup.path("file");
  fs::write(&file, "notes").expect("a file");
  assert_eq!(
    check_destination(&file),
    Destination::Occupied,
    "a file at the path counts"
  );
  let orphan = setup.path("missing").join("kontur");
  assert_eq!(
    check_destination(&orphan),
    Destination::NoParent {
      parent: setup.path("missing")
    }
  );
  assert_eq!(check_destination(Path::new("kontur")), Destination::NotAbsolute);
  #[cfg(windows)]
  assert_eq!(
    check_destination(Path::new(r"\\server\share\kontur")),
    Destination::NotLocal
  );
}

#[test]
fn startup_clone_a_checkout_of_the_project_is_cloned() {
  let setup = Setup::new();
  let (clones, jobs) = recorded(|job| {
    checkout(&job.destination, URL);
    Ok(ConsoleEnd::Exited { code: 0 })
  });
  // A separator at the end is dropped, as Git and the folder check are given it.
  let destination = setup.path("kontur");
  let typed = PathBuf::from(format!("{}{}", destination.display(), std::path::MAIN_SEPARATOR));
  let end = setup.start(&clones, &typed).expect("cloned");
  assert_eq!(
    end,
    CloneEnd::Cloned {
      folder: destination.clone()
    }
  );
  let jobs = jobs.lock();
  assert_eq!(jobs.len(), 1);
  assert_eq!(jobs[0].url, URL);
  assert_eq!(jobs[0].destination, destination);
  assert_eq!(jobs[0].words.title(), "Orchester · cloning Kontur");
  assert!(!destination.join(".orchester").exists(), "nothing was linked");
}

#[test]
fn startup_clone_git_done_without_the_projects_checkout_is_not_a_clone() {
  let setup = Setup::new();
  let (clones, _) = recorded(|job| {
    checkout(&job.destination, "https://github.com/maria/other.git");
    Ok(ConsoleEnd::Exited { code: 0 })
  });
  let destination = setup.path("kontur");
  let end = setup.start(&clones, &destination).expect("ended");
  assert!(
    matches!(&end, CloneEnd::NotCheckout { folder, check: FolderCheck::Differs { .. } } if *folder == destination),
    "{end:?}"
  );
}

#[test]
fn startup_clone_a_failure_keeps_what_git_left() {
  let setup = Setup::new();
  let (clones, _) = recorded(|job| {
    fs::create_dir_all(job.destination.join(".git")).expect("a partial clone");
    fs::write(job.destination.join(".git").join("HEAD"), "ref: refs/heads/main\n").expect("written");
    Ok(ConsoleEnd::Exited { code: 128 })
  });
  let destination = setup.path("kontur");
  let end = setup.start(&clones, &destination).expect("ended");
  assert_eq!(
    end,
    CloneEnd::Failed {
      code: 128,
      left: Some(destination.clone())
    }
  );
  assert!(destination.join(".git").join("HEAD").exists(), "what Git left is kept");
}

#[test]
fn startup_clone_a_closed_console_keeps_the_empty_folder_it_was_given() {
  let setup = Setup::new();
  let (clones, _) = recorded(|_| Ok(ConsoleEnd::Closed));
  let destination = setup.path("empty");
  fs::create_dir(&destination).expect("an empty folder");
  let end = setup.start(&clones, &destination).expect("ended");
  assert_eq!(end, CloneEnd::Closed { left: None });
  assert!(destination.is_dir(), "the person's folder is kept");
}

#[test]
fn startup_clone_an_occupied_destination_opens_no_console() {
  let setup = Setup::new();
  let (clones, jobs) = recorded(|_| panic!("no console opens for an occupied folder"));
  let destination = setup.path("kontur");
  fs::create_dir(&destination).expect("a folder");
  fs::write(destination.join("notes.txt"), "mine").expect("a file");
  assert_eq!(
    setup.start(&clones, &destination),
    Err(CloneRefusal::Destination(Destination::Occupied))
  );
  assert!(jobs.lock().is_empty());
  assert_eq!(fs::read_to_string(destination.join("notes.txt")).expect("kept"), "mine");
}

#[test]
fn startup_clone_without_git_says_so() {
  let setup = Setup::new();
  let (clones, _) = recorded(|_| Err(CloneRefusal::GitMissing));
  assert_eq!(
    setup.start(&clones, &setup.path("kontur")),
    Err(CloneRefusal::GitMissing)
  );
  assert_eq!(
    find_git(&SearchPath::default(), &Programs::processes()),
    Err(CloneRefusal::GitMissing)
  );
}

#[test]
fn startup_clone_a_project_without_an_address_has_no_clone() {
  let mut setup = Setup::new();
  setup.project.remote_hint = None;
  let (clones, jobs) = recorded(|_| panic!("nothing to clone"));
  assert_eq!(setup.start(&clones, &setup.path("kontur")), Err(CloneRefusal::NoUrl));
  assert!(jobs.lock().is_empty());
}

#[test]
fn startup_clone_a_second_start_follows_the_clone_that_runs() {
  let setup = Setup::new();
  let (release, released) = mpsc::channel::<()>();
  let released = Mutex::new(released);
  let (clones, jobs) = recorded(move |job| {
    released.lock().recv().expect("released");
    checkout(&job.destination, URL);
    Ok(ConsoleEnd::Exited { code: 0 })
  });
  let destination = setup.path("kontur");
  let key = (setup.server, "kontur".to_owned());
  assert_eq!(clones.follow(&key), None);
  let mut first: Started<'_> = Box::pin(clones.start(Programs::processes(), setup.request(&destination)));
  let deadline = Instant::now() + Duration::from_secs(10);
  while clones.follow(&key) != Some(CloneState::Running) {
    assert!(waits(&setup.runtime, &mut first), "the first waits for its console");
    assert!(Instant::now() < deadline, "the clone never started");
  }
  let elsewhere = setup.path("elsewhere");
  let mut second: Started<'_> = Box::pin(clones.start(Programs::processes(), setup.request(&elsewhere)));
  assert!(waits(&setup.runtime, &mut second), "the second follows the first");
  release.send(()).expect("the console ends");
  let cloned = CloneEnd::Cloned { folder: destination };
  assert_eq!(setup.runtime.block_on(first), Ok(cloned.clone()));
  assert_eq!(setup.runtime.block_on(second), Ok(cloned.clone()));
  assert_eq!(jobs.lock().len(), 1, "one console");
  assert!(!elsewhere.exists());
  assert_eq!(clones.follow(&key), Some(CloneState::Ended(Ok(cloned))));
}

/// A clone started and not awaited yet.
type Started<'a> = Pin<Box<dyn Future<Output = Result<CloneEnd, CloneRefusal>> + Send + 'a>>;

/// Lets `started` run a moment; whether it still waits then.
fn waits(runtime: &Runtime, started: &mut Started<'_>) -> bool {
  runtime
    .block_on(async { tokio::time::timeout(Duration::from_millis(10), started).await })
    .is_err()
}
