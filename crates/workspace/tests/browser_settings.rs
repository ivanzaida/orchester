//! What Settings › Browser reads and changes (`Workspace::browser_settings`, `docs/design/browser.md`, "Finding the
//! browser"): the browser installed, or none with what was looked for; a program the person chose, taken when it is a
//! Chromium-based browser's and refused with the reason otherwise; a chosen program that is gone; the profiles' folder
//! beside the application database; and the Browser window switch, on until turned off. The installed browsers are
//! looked for in the test's own folders only. Run with `cargo test -p orchester-workspace --test browser_settings`.

#![cfg(any(target_os = "macos", target_os = "windows"))]

use std::{fs, path::PathBuf};

use orchester_exec::browser::NotABrowser;
use orchester_store::AppDb;
use orchester_workspace::workspace::{AgentsBrowser, ChooseBrowserError, Workspace};

/// Where an install of Google Chrome, and of Chromium, below a folder puts its program.
#[cfg(target_os = "macos")]
const CHROME: &str = "Google Chrome.app/Contents/MacOS/Google Chrome";
#[cfg(target_os = "windows")]
const CHROME: &str = r"Google\Chrome\Application\chrome.exe";
#[cfg(target_os = "macos")]
const CHROMIUM: &str = "Chromium.app/Contents/MacOS/Chromium";
#[cfg(target_os = "windows")]
const CHROMIUM: &str = r"Chromium\Application\chrome.exe";
#[cfg(target_os = "macos")]
const FIREFOX: &str = "Firefox.app/Contents/MacOS/firefox";
#[cfg(target_os = "windows")]
const FIREFOX: &str = r"Mozilla Firefox\firefox.exe";

struct Fixture {
  dir: tempfile::TempDir,
  workspace: Workspace,
}

impl Fixture {
  /// A workspace whose installed browsers are looked for in `installed/` of the test's folder only.
  fn new() -> Self {
    let dir = tempfile::tempdir().expect("temporary folder");
    let workspace = Workspace::new(AppDb::open(&dir.path().join("orchester.db")).expect("the application database"));
    fs::create_dir_all(dir.path().join("installed")).expect("the installed folder");
    workspace.look_for_browsers_in(&[dir.path().join("installed")]);
    Self { dir, workspace }
  }

  fn install(&self, below: &str, program: &str) -> PathBuf {
    let path = self.dir.path().join(below).join(program);
    fs::create_dir_all(path.parent().expect("a folder above")).expect("its folder");
    fs::write(&path, "").expect("a stand-in program");
    path
  }

  fn path(&self, below: &str) -> PathBuf {
    self.dir.path().join(below)
  }
}

#[test]
fn browser_settings_say_none_is_installed_and_where_they_looked() {
  let fixture = Fixture::new();
  let settings = fixture.workspace.browser_settings().expect("read");

  let AgentsBrowser::NotFound(unavailable) = &settings.browser else {
    panic!("none is installed: {:?}", settings.browser);
  };
  assert!(
    unavailable
      .reason()
      .starts_with("no Chromium-based browser is installed"),
    "{unavailable}"
  );
  assert!(
    unavailable
      .reason()
      .ends_with(&fixture.path("installed").display().to_string()),
    "{unavailable}"
  );
  // The database names its folder as the file system resolves it (on macOS the temporary folders lie behind `/var`).
  let data = fs::canonicalize(fixture.dir.path()).expect("the test's folder");
  assert_eq!(settings.profiles, Some(data.join("browser")), "beside orchester.db");
  assert!(
    settings.opens_window,
    "the Browser window opens until a person turns it off"
  );
}

#[test]
fn browser_settings_show_the_browser_installed() {
  let fixture = Fixture::new();
  let chrome = fixture.install("installed", CHROME);

  let settings = fixture.workspace.browser_settings().expect("read");
  let AgentsBrowser::Found(found) = settings.browser else {
    panic!("Chrome is installed");
  };
  assert_eq!((found.name(), found.path()), ("Google Chrome", chrome.as_path()));
}

#[test]
fn browser_program_chosen_is_kept_when_it_is_a_chromium_based_browsers_and_refused_otherwise() {
  let fixture = Fixture::new();
  fixture.install("installed", CHROME);
  let chromium = fixture.install("elsewhere", CHROMIUM);
  let firefox = fixture.install("elsewhere", FIREFOX);

  let taken = fixture
    .workspace
    .choose_browser_program(&chromium)
    .expect("Chromium is taken");
  assert_eq!(taken.name(), "Chromium");
  let settings = fixture.workspace.browser_settings().expect("read");
  assert!(
    matches!(&settings.browser, AgentsBrowser::Chosen(program) if program.path() == chromium),
    "the choice comes before the browser installed: {:?}",
    settings.browser
  );

  let refused = fixture
    .workspace
    .choose_browser_program(&firefox)
    .expect_err("not Chromium-based");
  assert!(
    matches!(&refused, ChooseBrowserError::NotABrowser(NotABrowser::NotChromium(path)) if path == &firefox),
    "{refused}"
  );
  let missing = fixture.path("gone").join("chrome");
  assert!(matches!(
    fixture.workspace.choose_browser_program(&missing),
    Err(ChooseBrowserError::NotABrowser(NotABrowser::Missing(_)))
  ));
  let settings = fixture.workspace.browser_settings().expect("read");
  assert!(
    matches!(&settings.browser, AgentsBrowser::Chosen(program) if program.path() == chromium),
    "a refused program changes nothing: {:?}",
    settings.browser
  );
}

#[test]
fn browser_program_chosen_that_is_gone_is_unusable_and_not_passed_over() {
  let fixture = Fixture::new();
  fixture.install("installed", CHROME);
  let chromium = fixture.install("elsewhere", CHROMIUM);
  fixture.workspace.choose_browser_program(&chromium).expect("taken");
  fs::remove_dir_all(fixture.path("elsewhere")).expect("Chromium is removed");

  let settings = fixture.workspace.browser_settings().expect("read");
  assert!(
    matches!(
      &settings.browser,
      AgentsBrowser::ChosenUnusable(NotABrowser::Missing(_))
    ),
    "{:?}",
    settings.browser
  );
}

#[test]
fn browser_window_switch_is_kept() {
  let fixture = Fixture::new();
  fixture
    .workspace
    .set_browser_window_opens_itself(false)
    .expect("turned off");

  assert!(!fixture.workspace.browser_settings().expect("read").opens_window);
}
