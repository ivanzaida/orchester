//! Settings › Browser (`09 · Settings · Browser` `j3Lre`, `09b · … not found` `NZECi`): the browser found with its
//! version and its program's path, or none found with what was looked for and *Look again*; *Choose another program…*
//! taking a Chromium-based browser's program and refusing any other with the reason; the profiles' folder; and *Open
//! the Browser window when an agent starts browsing*, on until turned off and kept. The installed browsers are looked
//! for in the test's own folder only. Run with `cargo test -p orchester-app --test settings_browser browser_`.

#![cfg(any(target_os = "macos", target_os = "windows"))]

mod support;

use std::{
  fs,
  path::{Path, PathBuf},
};

use orchester_app::routes::Route;
use orchester_workspace::workspace::Workspace;
use support::{Data, Window, stand_ins::StandIns};

#[cfg(target_os = "macos")]
const CHROME: &str = "Google Chrome.app/Contents/MacOS/Google Chrome";
#[cfg(target_os = "windows")]
const CHROME: &str = r"Google\Chrome\Application\chrome.exe";
#[cfg(target_os = "macos")]
const CHROMIUM: &str = "Chromium.app/Contents/MacOS/Chromium";
#[cfg(target_os = "windows")]
const CHROMIUM: &str = r"Chromium\Application\chrome.exe";
#[cfg(target_os = "macos")]
const EDGE: &str = "Microsoft Edge.app/Contents/MacOS/Microsoft Edge";
#[cfg(target_os = "windows")]
const EDGE: &str = r"Microsoft\Edge\Application\msedge.exe";
#[cfg(target_os = "macos")]
const FIREFOX: &str = "Firefox.app/Contents/MacOS/firefox";
#[cfg(target_os = "windows")]
const FIREFOX: &str = r"Mozilla Firefox\firefox.exe";

struct Page {
  window: Window,
  stand_ins: StandIns,
  workspace: Workspace,
}

impl Page {
  /// Settings › Browser, with the installed browsers looked for in `installed/` of the test's folder only.
  fn open(installed: &[&str]) -> Self {
    Self::installed(installed).shown()
  }

  /// The window with the browsers installed, before Settings › Browser is shown.
  fn installed(installed: &[&str]) -> Self {
    let (mut window, stand_ins) = Window::with_stand_ins(Data::new());
    window.tree.resize(1440, 960);
    let workspace = window.launch.workspace().expect("the session started").clone();
    let page = Self {
      window,
      stand_ins,
      workspace,
    };
    fs::create_dir_all(page.folder("installed")).expect("the installed folder");
    for program in installed {
      page.install("installed", program);
    }
    page.workspace.look_for_browsers_in(&[page.folder("installed")]);
    page
  }

  fn shown(mut self) -> Self {
    self.window.navigation().go(Route::Browser);
    self.window.until("Settings › Browser", |window| {
      window.texts("settings-browser-found-text").is_some()
    });
    self
  }

  fn folder(&self, name: &str) -> PathBuf {
    self.window.data.dir.path().join(name)
  }

  fn install(&self, below: &str, program: &str) -> PathBuf {
    let path = self.folder(below).join(program);
    fs::create_dir_all(path.parent().expect("a folder above")).expect("its folder");
    fs::write(&path, "").expect("a stand-in program");
    path
  }

  /// Makes the install around the stand-in `program` tell `version`.
  #[cfg(target_os = "macos")]
  fn tell_version(&self, program: &Path, version: &str) {
    let contents = program.parent().and_then(Path::parent).expect("the bundle's Contents");
    fs::write(
      contents.join("Info.plist"),
      format!("<plist><dict><key>CFBundleShortVersionString</key><string>{version}</string></dict></plist>"),
    )
    .expect("the bundle's Info.plist");
  }

  /// Makes the install around the stand-in `program` tell `version`.
  #[cfg(target_os = "windows")]
  fn tell_version(&self, program: &Path, version: &str) {
    fs::create_dir(program.with_file_name(version)).expect("the version's folder");
  }

  /// *Choose another program…* with the picker handing over `program`.
  fn choose(&mut self, program: &Path) {
    self.stand_ins.choose(Some(program.to_path_buf()));
    self.window.press("settings-browser-choose");
  }

  fn found(&mut self) -> String {
    self.window.text("settings-browser-found-text")
  }
}

#[test]
fn browser_found_shows_its_name_its_program_and_the_profiles_folder() {
  let mut page = Page::open(&[CHROME]);
  let program = page.folder("installed").join(CHROME);

  assert_eq!(page.window.text("settings-tab-browser"), "Browser");
  assert_eq!(page.found(), "Google Chrome");
  assert!(
    page.window.texts("settings-browser-found-version").is_none(),
    "an install that does not tell shows no version"
  );
  assert!(
    page
      .window
      .text("settings-browser-found-hint")
      .starts_with("Found on this computer."),
    "{}",
    page.window.text("settings-browser-found-hint")
  );
  assert_eq!(
    page.window.text("settings-browser-program-path"),
    program.display().to_string()
  );
  assert_eq!(page.window.text("settings-browser-choose"), "Choose another program…");
  assert!(page.window.texts("settings-browser-not-found").is_none(), "no notice");
  let profiles = page.window.text("settings-browser-profiles-path");
  assert!(Path::new(&profiles).ends_with("browser"), "{profiles}");
  assert_eq!(page.window.text("settings-save-state"), "All changes saved");
}

#[test]
fn browser_found_shows_the_version_its_install_tells_and_a_chosen_one_that_does_not_shows_none() {
  let page = Page::installed(&[CHROME]);
  page.tell_version(&page.folder("installed").join(CHROME), "130.0.6723.92");
  let mut page = page.shown();

  assert_eq!(page.found(), "Google Chrome");
  assert_eq!(page.window.text("settings-browser-found-version"), "130.0.6723.92");

  let chromium = page.install("elsewhere", CHROMIUM);
  page.choose(&chromium);
  page.window.until("Chromium chosen", |window| {
    window.text("settings-browser-found-text") == "Chromium"
  });
  assert!(
    page.window.texts("settings-browser-found-version").is_none(),
    "no made-up version"
  );
}

#[test]
fn browser_not_found_says_what_was_looked_for_and_looks_again() {
  let mut page = Page::open(&[]);

  assert_eq!(page.found(), "None found");
  assert_eq!(
    page.window.text("settings-browser-found-hint"),
    "Agents' browser tools answer that the browser is unavailable."
  );
  assert_eq!(
    page.window.text("settings-browser-not-found-title"),
    "No Chromium-based browser found"
  );
  let looked = page.window.text("settings-browser-not-found-body");
  assert!(looked.starts_with("Orchester looked for"), "{looked}");
  assert!(looked.contains("look again, or choose"), "how to fix it: {looked}");
  assert_eq!(page.window.text("settings-browser-choose"), "Choose a program…");
  assert!(
    page.window.texts("settings-browser-program-path").is_none(),
    "no program to show"
  );

  page.install("installed", CHROME);
  page.window.press("settings-browser-look-again");
  page.window.until("the browser installed meanwhile", |window| {
    window.text("settings-browser-found-text") == "Google Chrome"
  });
  assert!(page.window.texts("settings-browser-not-found").is_none());
}

#[test]
fn browser_another_chromium_based_program_is_taken_and_kept() {
  let mut page = Page::open(&[CHROME]);
  let chromium = page.install("elsewhere", CHROMIUM);

  page.choose(&chromium);
  page.window.until("Chromium chosen", |window| {
    window.text("settings-browser-found-text") == "Chromium"
  });
  assert!(
    page
      .window
      .text("settings-browser-found-hint")
      .starts_with("Chosen by you."),
    "{}",
    page.window.text("settings-browser-found-hint")
  );
  assert_eq!(
    page.window.text("settings-browser-program-path"),
    chromium.display().to_string()
  );
  assert_eq!(
    page.workspace.browser_settings().expect("read").browser,
    orchester_workspace::workspace::AgentsBrowser::Chosen(
      orchester_exec::browser::chosen(&chromium).expect("Chromium")
    ),
    "kept in the application database"
  );
}

#[test]
fn browser_a_second_program_chosen_in_a_row_is_shown_as_taken() {
  let mut page = Page::open(&[CHROME]);
  let chromium = page.install("elsewhere", CHROMIUM);
  let edge = page.install("elsewhere", EDGE);

  page.choose(&chromium);
  page.window.until("Chromium chosen", |window| {
    window.text("settings-browser-found-text") == "Chromium"
  });
  page.choose(&edge);
  page.window.until("Microsoft Edge chosen after Chromium", |window| {
    window.text("settings-browser-found-text") == "Microsoft Edge"
  });
  assert_eq!(
    page.window.text("settings-browser-program-path"),
    edge.display().to_string()
  );
  assert_eq!(
    page.workspace.browser_settings().expect("read").browser,
    orchester_workspace::workspace::AgentsBrowser::Chosen(
      orchester_exec::browser::chosen(&edge).expect("Microsoft Edge")
    ),
    "what Settings shows is what agents get"
  );
}

#[test]
fn browser_a_program_that_is_not_chromium_based_is_refused_with_the_reason() {
  let mut page = Page::open(&[CHROME]);
  let firefox = page.install("elsewhere", FIREFOX);

  page.choose(&firefox);
  page.window.until("the refusal", |window| {
    window.texts("settings-browser-refused").is_some()
  });
  assert_eq!(
    page.window.text("settings-browser-refused"),
    format!(
      "Not taken. {} isn't a Chromium-based browser. Choose Microsoft Edge, Google Chrome, Brave or Chromium.",
      firefox.display()
    )
  );
  assert_eq!(page.found(), "Google Chrome", "nothing changed");
  assert!(matches!(
    page.workspace.browser_settings().expect("read").browser,
    orchester_workspace::workspace::AgentsBrowser::Found(_)
  ));
}

#[test]
fn browser_window_switch_is_on_until_turned_off() {
  let mut page = Page::open(&[CHROME]);
  assert!(page.workspace.browser_settings().expect("read").opens_window);
  assert!(
    page.window.texts("settings-browser-window-group-note").is_none(),
    "the window is there, so no note says it comes later"
  );

  page.window.press("settings-browser-window");
  let workspace = page.workspace.clone();
  page.window.until("the switch to be saved off", |_| {
    !workspace.browser_settings().expect("read").opens_window
  });
  page.window.until("the header", |window| {
    window.text("settings-save-state") == "All changes saved"
  });
}
