//! Finding the browser for agents and starting it headless (`orchester_exec::browser`): the first installed browser
//! wins, none installed is `browser_unavailable` with its reason, the command runs on the project's profile with a
//! free debugging port, and the debugging address is read back from `DevToolsActivePort`. Run with
//! `cargo test -p orchester-exec --test browser browser_`.

use std::{fs, path::PathBuf};

use orchester_exec::browser::{
  BROWSER_UNAVAILABLE, BrowserUnavailable, DevToolsActivePort, first_installed, headed, headless,
  registry_default_value,
};

fn program_in(folder: &std::path::Path, name: &str) -> PathBuf {
  let path = folder.join(name);
  fs::write(&path, "").expect("write a stand-in program");
  path
}

#[test]
fn browser_first_installed_takes_the_first_program_that_is_a_file() {
  let folder = tempfile::tempdir().expect("temporary folder");
  let edge = program_in(folder.path(), "msedge");
  let chrome = program_in(folder.path(), "chrome");
  let found = first_installed(
    [
      ("Missing", folder.path().join("missing")),
      ("Microsoft Edge", edge.clone()),
      ("Google Chrome", chrome),
    ],
    "the test folder",
  )
  .expect("a browser is installed");

  assert_eq!(found.name(), "Microsoft Edge");
  assert_eq!(found.path(), edge);
}

#[test]
fn browser_none_installed_is_unavailable_with_its_reason() {
  let folder = tempfile::tempdir().expect("temporary folder");
  fs::create_dir(folder.path().join("a folder, not a program")).expect("make a folder");
  let unavailable = first_installed(
    [
      ("Google Chrome", folder.path().join("missing")),
      ("Google Chrome", folder.path().join("elsewhere")),
      ("Brave", folder.path().join("a folder, not a program")),
    ],
    "/Applications and ~/Applications",
  )
  .expect_err("no program is a file");

  assert_eq!(
    unavailable.reason(),
    "no Chromium-based browser is installed: looked for Google Chrome, Brave in /Applications and ~/Applications"
  );
  assert!(
    unavailable.to_string().starts_with(&format!("{BROWSER_UNAVAILABLE}: ")),
    "{unavailable}"
  );
}

#[test]
fn browser_headless_command_runs_on_the_profile_with_a_free_debugging_port() {
  let folder = tempfile::tempdir().expect("temporary folder");
  let program = first_installed([("Google Chrome", program_in(folder.path(), "chrome"))], "here").expect("installed");
  let profile = folder.path().join("browser").join("project");
  let command = headless(&program, &profile).expect("a Unicode profile folder");

  assert_eq!(command.program(), program.path());
  let args = command.args();
  for flag in [
    "--headless=new",
    "--remote-debugging-port=0",
    "--no-first-run",
    "--no-default-browser-check",
    "--disable-extensions",
    "--disable-sync",
    "--mute-audio",
  ] {
    assert!(args.iter().any(|arg| arg == flag), "{flag} in {args:?}");
  }
  let profile_arg = format!("--user-data-dir={}", profile.display());
  assert!(args.contains(&profile_arg), "{args:?}");
  assert_eq!(args.last().map(String::as_str), Some("about:blank"));
}

/// *Open in a browser window* starts the same browser on the same profile, only without `--headless`.
#[test]
fn browser_headed_command_is_the_headless_one_without_headless() {
  let folder = tempfile::tempdir().expect("temporary folder");
  let program = first_installed([("Google Chrome", program_in(folder.path(), "chrome"))], "here").expect("installed");
  let profile = folder.path().join("browser").join("project");
  let headless = headless(&program, &profile).expect("a Unicode profile folder");
  let headed = headed(&program, &profile).expect("a Unicode profile folder");

  assert_eq!(headed.program(), program.path());
  let expected: Vec<&String> = headless.args().iter().filter(|arg| *arg != "--headless=new").collect();
  assert_eq!(headed.args().iter().collect::<Vec<_>>(), expected);
  assert!(!headed.args().iter().any(|arg| arg.starts_with("--headless")));
}

#[test]
fn browser_devtools_active_port_reads_the_port_and_the_browser_path() {
  let port = DevToolsActivePort::parse("51234\n/devtools/browser/3f0c2b7e-1d2a-4c55-9b1e-0a3e5d8c7f21\n")
    .expect("a complete file");

  assert_eq!(port.port(), 51234);
  assert_eq!(
    port.websocket_url(),
    "ws://127.0.0.1:51234/devtools/browser/3f0c2b7e-1d2a-4c55-9b1e-0a3e5d8c7f21"
  );
  for incomplete in [
    "",
    "51234",
    "51234\n",
    "0\n/devtools/browser/id",
    "port\n/devtools/browser/id",
    "51234\n/devtools/page/id",
    "51234\n/devtools/browser/a b",
  ] {
    assert_eq!(DevToolsActivePort::parse(incomplete), None, "{incomplete:?}");
  }
}

#[test]
fn browser_devtools_active_port_is_read_from_the_profile_and_removed_before_a_start() {
  let profile = tempfile::tempdir().expect("temporary folder");
  assert_eq!(DevToolsActivePort::read(profile.path()).expect("readable"), None);
  DevToolsActivePort::remove(profile.path()).expect("nothing to remove is no error");

  fs::write(
    profile.path().join(orchester_exec::browser::DEVTOOLS_ACTIVE_PORT),
    "9222\n/devtools/browser/id\n",
  )
  .expect("write the file");
  let port = DevToolsActivePort::read(profile.path())
    .expect("readable")
    .expect("complete");
  assert_eq!(port.port(), 9222);

  DevToolsActivePort::remove(profile.path()).expect("removed");
  assert_eq!(DevToolsActivePort::read(profile.path()).expect("readable"), None);
}

#[test]
fn browser_registry_default_value_is_read_whatever_the_language() {
  let english = "\r\nHKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\App Paths\\msedge.exe\r\n    \
                 (Default)    REG_SZ    C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe\r\n\r\n";
  let german = "HKEY_CURRENT_USER\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\App Paths\\chrome.exe\n    \
                (Standard)    REG_EXPAND_SZ    \"C:\\Users\\a\\AppData\\Local\\Google\\Chrome\\Application\\chrome.exe\"\n";
  let unset = "HKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\App Paths\\chrome.exe\n    \
               (Default)    REG_SZ    \n";

  assert_eq!(
    registry_default_value(english),
    Some(PathBuf::from(
      "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe"
    ))
  );
  assert_eq!(
    registry_default_value(german),
    Some(PathBuf::from(
      "C:\\Users\\a\\AppData\\Local\\Google\\Chrome\\Application\\chrome.exe"
    ))
  );
  assert_eq!(registry_default_value(unset), None);
  assert_eq!(registry_default_value(""), None);
}

#[test]
fn browser_unavailable_keeps_its_reason() {
  let unavailable = BrowserUnavailable::new("the browser for agents is not supported on this platform yet");

  assert_eq!(
    unavailable.to_string(),
    "browser_unavailable: the browser for agents is not supported on this platform yet"
  );
}

/// A chosen program, and the folders [`orchester_exec::browser::find_in`] looks in, on Windows and macOS.
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod chosen {
  use std::{fs, path::Path};

  use orchester_exec::browser::{NotABrowser, chosen, find_in};

  /// Where an install of Google Chrome below `folder` puts its program, as the platform finds it.
  #[cfg(target_os = "macos")]
  const CHROME: &str = "Google Chrome.app/Contents/MacOS/Google Chrome";
  #[cfg(target_os = "windows")]
  const CHROME: &str = r"Google\Chrome\Application\chrome.exe";
  /// Another browser's program, which is not Chromium-based.
  #[cfg(target_os = "macos")]
  const FIREFOX: &str = "Firefox.app/Contents/MacOS/firefox";
  #[cfg(target_os = "windows")]
  const FIREFOX: &str = r"Mozilla Firefox\firefox.exe";

  fn install(folder: &Path, program: &str) -> std::path::PathBuf {
    let path = folder.join(program);
    fs::create_dir_all(path.parent().expect("a folder above")).expect("make its folder");
    fs::write(&path, "").expect("write a stand-in program");
    path
  }

  #[test]
  fn browser_chosen_program_of_a_chromium_based_browser_is_taken_with_its_name() {
    let folder = tempfile::tempdir().expect("temporary folder");
    let program = install(folder.path(), CHROME);
    let taken = chosen(&program).expect("Google Chrome's program");

    assert_eq!(taken.name(), "Google Chrome");
    assert_eq!(taken.path(), program);
  }

  #[cfg(target_os = "macos")]
  #[test]
  fn browser_chosen_app_bundle_stands_for_the_program_inside_it() {
    let folder = tempfile::tempdir().expect("temporary folder");
    let program = install(folder.path(), "Brave Browser.app/Contents/MacOS/Brave Browser");
    let taken = chosen(&folder.path().join("Brave Browser.app")).expect("Brave's bundle");

    assert_eq!(taken.name(), "Brave");
    assert_eq!(taken.path(), program);
  }

  #[cfg(target_os = "windows")]
  #[test]
  fn browser_chosen_chrome_exe_under_a_chromium_folder_is_chromium() {
    let folder = tempfile::tempdir().expect("temporary folder");
    let program = install(folder.path(), r"Chromium\Application\CHROME.EXE");

    assert_eq!(chosen(&program).expect("Chromium's program").name(), "Chromium");
  }

  #[test]
  fn browser_chosen_program_that_is_not_a_chromium_based_browser_is_refused_with_the_reason() {
    let folder = tempfile::tempdir().expect("temporary folder");
    let firefox = install(folder.path(), FIREFOX);
    let missing = folder.path().join("gone").join("chrome");

    assert_eq!(chosen(&firefox), Err(NotABrowser::NotChromium(firefox.clone())));
    assert_eq!(
      chosen(&firefox).expect_err("refused").to_string(),
      format!(
        "{} is not a Chromium-based browser's program: choose Microsoft Edge, Google Chrome, Brave or Chromium",
        firefox.display()
      )
    );
    assert_eq!(chosen(&missing), Err(NotABrowser::Missing(missing.clone())));
    assert_eq!(
      chosen(folder.path()),
      Err(NotABrowser::Folder(folder.path().to_path_buf()))
    );
  }

  #[test]
  fn browser_find_in_looks_only_in_the_folders_it_is_given() {
    let empty = tempfile::tempdir().expect("temporary folder");
    let unavailable = find_in(&[empty.path().to_path_buf()]).expect_err("nothing is installed there");
    assert!(
      unavailable
        .reason()
        .ends_with(&format!(" in {}", empty.path().display())),
      "{unavailable}"
    );
    assert!(unavailable.reason().contains("Google Chrome"), "{unavailable}");

    let installed = tempfile::tempdir().expect("temporary folder");
    let program = install(installed.path(), CHROME);
    let found = find_in(&[empty.path().to_path_buf(), installed.path().to_path_buf()]).expect("found there");
    assert_eq!((found.name(), found.path()), ("Google Chrome", program.as_path()));
  }
}

#[cfg(target_os = "macos")]
mod macos {
  use std::path::{Path, PathBuf};

  use orchester_exec::browser::macos::{candidates, folders};

  #[test]
  fn browser_macos_looks_in_applications_then_the_persons_own() {
    assert_eq!(
      folders(Some(Path::new("/Users/a"))),
      [PathBuf::from("/Applications"), PathBuf::from("/Users/a/Applications")]
    );
    assert_eq!(folders(None), [PathBuf::from("/Applications")]);
  }

  #[test]
  fn browser_macos_prefers_chrome_then_edge_brave_and_chromium_wherever_each_is() {
    let found = candidates(&folders(Some(Path::new("/Users/a"))));
    let names: Vec<&str> = found.iter().map(|(name, _)| *name).collect();

    assert_eq!(
      names,
      [
        "Google Chrome",
        "Google Chrome",
        "Microsoft Edge",
        "Microsoft Edge",
        "Brave",
        "Brave",
        "Chromium",
        "Chromium"
      ]
    );
    assert_eq!(
      found[1].1,
      PathBuf::from("/Users/a/Applications/Google Chrome.app/Contents/MacOS/Google Chrome")
    );
    assert_eq!(
      found[4].1,
      PathBuf::from("/Applications/Brave Browser.app/Contents/MacOS/Brave Browser")
    );
  }

  #[test]
  fn browser_macos_finds_an_installed_program_or_says_why_not() {
    match orchester_exec::browser::find() {
      Ok(program) => assert!(program.path().is_file(), "{program:?}"),
      Err(unavailable) => assert!(unavailable.reason().contains("/Applications"), "{unavailable}"),
    }
  }
}
