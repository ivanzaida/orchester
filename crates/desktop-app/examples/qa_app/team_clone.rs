//! `team-clone` (ORC-584): Acme shares one project per way a clone from *Link a folder* ends ([`team::CLONED`]), and
//! before the window shows the QA copy types where each one goes and presses *Clone repository…* on its page with its
//! own pointer, as a person would: an MCP client's press is refused. The console is a stand-in ([`ConsoleScript`]): no
//! console window opens, nothing goes over the network and nobody signs in, and what a clone writes goes into
//! `clones/<slug>` beside the database. The window stays on Kontur's page as a person sees it after a clone that
//! worked; each other page, opened again, follows its project's clone with *Clone into* empty, as the app does.

use std::{
  error::Error,
  fs,
  path::{Path, PathBuf},
};

use lurq::app::{App, Tree};
use orchester_app::{
  mcp::WindowLink,
  routes::{JoinTarget, JoinWay, Route},
  services::Services,
  test_support::stand_ins::{Console, ConsoleScript},
};
use orchester_core::id::ServerId;
use orchester_workspace::sharing::project::clone::{CloneRefusal, Clones, LaunchJob};

use super::team::{self, window::Window};

/// The folder beside the application database the clones go in.
const FOLDER: &str = "clones";
const INTO: &str = "join-team-link-clone-into";
const TARGET: &str = "join-team-link-clone-target";
const START: &str = "join-team-link-clone-start";

/// How a project's clone ends.
enum Ending {
  /// The stand-in console plays this.
  Plays(ConsoleScript),
  /// No clone runs, for this reason.
  Refused(CloneRefusal),
}

/// How the clone of the project at `slug` ends, and the element its page shows then.
fn ending(slug: &str) -> Option<(Ending, &'static str)> {
  use ConsoleScript::{Closes, Exits, Holds, WritesThen};
  use Ending::{Plays, Refused};
  Some(match slug {
    // Still running: the console waits until the QA copy ends.
    "atlas" => (Plays(Holds(Box::new(Closes))), "join-team-link-clone-waiting"),
    // Git refused, as a denied sign-in ends, and left nothing.
    "billing" => (Plays(Exits(128)), "join-team-link-clone-failed"),
    // The console closed before Git ended, with a half-written checkout kept.
    "courier" => (Plays(WritesThen(Box::new(Closes))), "join-team-link-clone-left"),
    // Git ended with 0, but what it left is no checkout of the project.
    "depot" => (
      Plays(WritesThen(Box::new(Exits(0)))),
      "join-team-link-clone-not-checkout",
    ),
    "gateway" => (Refused(CloneRefusal::GitMissing), "join-team-link-clone-git-missing"),
    "harbor" => (Refused(CloneRefusal::NotOpened), "join-team-link-clone-not-opened"),
    "ingest" => (Refused(CloneRefusal::NotStarted), "join-team-link-clone-not-started"),
    "juno" => (Refused(CloneRefusal::Unsupported), "join-team-link-clone-unsupported"),
    "ledger" => (Refused(CloneRefusal::Lost), "join-team-link-clone-lost"),
    "kontur" => (Plays(ConsoleScript::Clones), "join-team-link-clone-done"),
    _ => return None,
  })
}

/// The slug of the project the clone `job` is of: the last segment of `https://github.com/acme/<slug>.git`.
fn slug_of(job: &LaunchJob) -> &str {
  job
    .url
    .rsplit('/')
    .next()
    .and_then(|last| last.strip_suffix(".git"))
    .unwrap_or_default()
}

/// The stand-in console's clones and the folder they go in. Dropped before the runtime, it lets Atlas's clone end, so
/// the runtime's end does not wait on its console.
pub struct StandIn {
  pub clones: Clones,
  console: Console,
  folder: PathBuf,
}

impl StandIn {
  /// Clones into an empty folder of its own beside the application database at `database`, one console at a time:
  /// the QA copy waits for each to show how it ended.
  pub fn new(database: &Path) -> Result<Self, Box<dyn Error>> {
    let folder = database.with_file_name(FOLDER);
    fs::create_dir_all(&folder)?;
    let console = Console::default();
    let played = console.clone();
    let clones = Clones::with_launcher(move |job| match ending(slug_of(job)) {
      Some((Ending::Plays(script), _)) => {
        played.script(script);
        played.run(job)
      }
      Some((Ending::Refused(refusal), _)) => Err(refusal),
      // Only Acme's projects are cloned here.
      None => Err(CloneRefusal::Lost),
    });
    Ok(Self {
      clones,
      console,
      folder,
    })
  }

  /// Clones each of Acme's projects on its page, Kontur last, and waits for each page to show how its clone ended.
  pub fn drive(
    &self,
    (app, tree): (&mut App, &mut Tree),
    link: &WindowLink,
    services: &Services,
  ) -> Result<(), Box<dyn Error>> {
    let server = acme(services)?;
    let mut window = Window { app, tree };
    for (_, slug, _) in team::CLONED {
      let (_, shows) = ending(slug).ok_or_else(|| format!("{slug} has no ending"))?;
      let target = JoinTarget::new(server, slug).ok_or("a project's slug is not empty")?;
      link.navigation()?.go(Route::JoinTeam(JoinWay::Folder(target)));
      window.until(INTO)?;
      window.set(INTO, &destination(&self.folder, slug))?;
      window.until(TARGET)?;
      window.press(START)?;
      window.until(shows)?;
    }
    Ok(())
  }
}

impl Drop for StandIn {
  fn drop(&mut self) {
    self.console.release();
  }
}

/// Where the clone of `slug` goes: a folder that doesn't exist yet in `folder`.
fn destination(folder: &Path, slug: &str) -> String {
  folder.join(slug).display().to_string()
}

/// The id of Acme, the only server the copy joined.
fn acme(services: &Services) -> Result<ServerId, Box<dyn Error>> {
  services
    .servers
    .entries()?
    .into_iter()
    .find(|entry| entry.in_use())
    .map(|entry| entry.stored.id)
    .ok_or_else(|| "the QA copy joined Acme".into())
}
