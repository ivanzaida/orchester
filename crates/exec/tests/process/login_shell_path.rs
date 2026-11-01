//! The login shell's `PATH` (read on macOS as the app starts) and the search path's folders with
//! their sources. Reading, parsing and merging are platform-neutral, so they are tested everywhere.

use std::{
  env,
  ffi::OsString,
  fs,
  path::{Path, PathBuf},
  time::{Duration, Instant},
};

use orchester_exec::search_path::{
  FolderSource, ProgramLookup, SearchBasis, SearchPath,
  login_shell::{self, LoginShellError, LoginShellPath, PATH_END, PATH_START},
  shown_folder,
};

const PROGRAM: &str = "orchester-probe-cli";

#[test]
fn login_shell_path_is_parsed_between_the_markers() {
  let stdout = format!(
    "Welcome back!\nnoise {PATH_START} that is not the PATH\n{PATH_START}/opt/homebrew/bin::relative/bin:/usr/bin:{PATH_END}\nbye {PATH_END}\n"
  );
  let folders = login_shell::parse(&stdout).expect("a PATH between the markers");
  assert_eq!(folders, [PathBuf::from("/opt/homebrew/bin"), PathBuf::from("/usr/bin")]);

  assert!(matches!(
    login_shell::parse("Welcome back!\n"),
    Err(LoginShellError::NoPath)
  ));
  assert!(matches!(
    login_shell::parse(&format!("{PATH_START}/usr/bin")),
    Err(LoginShellError::NoPath)
  ));
}

#[test]
fn login_shell_path_read_that_does_not_finish_times_out() {
  let (program, args) = long_running();
  let started = Instant::now();
  let read = login_shell::read_with(Path::new(program), args, Duration::from_millis(300));
  assert!(matches!(read, Err(LoginShellError::TimedOut(_))), "{read:?}");
  assert!(started.elapsed() < Duration::from_secs(10), "{:?}", started.elapsed());
}

#[test]
fn login_shell_path_takes_only_standard_output() {
  let stdout_path = format!("{PATH_START}/a/bin:/b/bin{PATH_END}");
  let stderr_path = format!("{PATH_START}/stderr/bin{PATH_END}");
  #[cfg(windows)]
  let (program, args) = (
    "cmd",
    [
      "/C",
      "echo",
      "greeting",
      "&",
      "echo",
      &stdout_path,
      "&",
      "echo",
      &stderr_path,
      "1>&2",
    ],
  );
  #[cfg(unix)]
  let script = format!("echo greeting; echo '{stdout_path}'; echo '{stderr_path}' >&2");
  #[cfg(unix)]
  let (program, args) = ("/bin/sh", ["-c", &script]);

  let folders = login_shell::read_with(Path::new(program), &args, login_shell::TIMEOUT).expect("the PATH is read");
  assert_eq!(folders, [PathBuf::from("/a/bin"), PathBuf::from("/b/bin")]);
}

#[test]
fn login_shell_path_is_read_while_a_background_job_holds_the_output() {
  let path = format!("{PATH_START}/a/bin{PATH_END}");
  let (program, args) = leaving_a_background_job(Some(&path));
  let args: Vec<&str> = args.iter().map(String::as_str).collect();
  let started = Instant::now();
  let folders = login_shell::read_with(Path::new(program), &args, login_shell::TIMEOUT).expect("the PATH is read");
  assert_eq!(folders, [PathBuf::from("/a/bin")]);
  assert!(started.elapsed() < login_shell::TIMEOUT, "{:?}", started.elapsed());
}

#[test]
fn login_shell_path_read_ends_at_the_deadline_while_a_background_job_holds_the_output() {
  let (program, args) = leaving_a_background_job(None);
  let args: Vec<&str> = args.iter().map(String::as_str).collect();
  let timeout = Duration::from_secs(1);
  let started = Instant::now();
  let read = login_shell::read_with(Path::new(program), &args, timeout);
  assert!(matches!(read, Err(LoginShellError::TimedOut(_))), "{read:?}");
  assert!(started.elapsed() < Duration::from_secs(5), "{:?}", started.elapsed());
}

#[test]
fn login_shell_path_failed_read_keeps_what_an_earlier_read_found() {
  let shell = Path::new("/bin/zsh");
  let none = LoginShellPath::default().after_read(Err(LoginShellError::NoPath), shell);
  assert_eq!(none, LoginShellPath::default());
  assert_eq!(
    none.basis(),
    SearchBasis::UsualFolders,
    "no read succeeded, so no shell is named"
  );

  let read = none.after_read(Ok(vec![PathBuf::from("/a/bin")]), shell);
  assert_eq!(&**read.folders(), [PathBuf::from("/a/bin")]);
  let zsh = SearchBasis::LoginShell {
    shell: "zsh".to_owned(),
  };
  assert_eq!(read.basis(), zsh);

  let kept = read
    .clone()
    .after_read(Err(LoginShellError::NoPath), Path::new("/bin/bash"));
  assert_eq!(kept, read, "a failed read again loses nothing");

  let again = kept.after_read(Ok(vec![PathBuf::from("/b/bin")]), Path::new("/bin/bash"));
  assert_eq!(&**again.folders(), [PathBuf::from("/b/bin")]);
  assert_eq!(
    again.basis(),
    SearchBasis::LoginShell {
      shell: "bash".to_owned()
    }
  );
}

#[test]
fn login_shell_path_folder_is_shown_with_the_home_folder_as_a_tilde() {
  let home = Path::new("/Users/me");
  assert_eq!(
    shown_folder(Path::new("/Users/me/.local/bin"), Some(home)),
    "~/.local/bin"
  );
  assert_eq!(shown_folder(home, Some(home)), "~");
  assert_eq!(
    shown_folder(Path::new("/opt/homebrew/bin"), Some(home)),
    "/opt/homebrew/bin"
  );
  assert_eq!(
    shown_folder(Path::new("/Users/me/.local/bin"), None),
    "/Users/me/.local/bin"
  );
}

#[test]
fn login_shell_path_folders_follow_the_inherited_path_before_the_install_folders() {
  let inherited = joined(&["/usr/bin", "/bin"]);
  let login_shell = ["/opt/homebrew/bin", "/usr/bin"].map(PathBuf::from);
  let install = [
    PathBuf::from("/opt/homebrew/bin"),
    PathBuf::from("/usr/local/bin"),
    unjoinable(),
  ];

  let search = SearchPath::merge(Some(&inherited), &login_shell, &install);
  let folders: Vec<(&Path, FolderSource)> = search
    .folders()
    .iter()
    .map(|folder| (folder.path(), folder.source()))
    .collect();
  assert_eq!(
    folders,
    [
      (Path::new("/usr/bin"), FolderSource::Inherited),
      (Path::new("/bin"), FolderSource::Inherited),
      (Path::new("/opt/homebrew/bin"), FolderSource::LoginShell),
      (Path::new("/usr/local/bin"), FolderSource::InstallFolder),
    ]
  );
  assert_eq!(
    search.joined(),
    joined(&["/usr/bin", "/bin", "/opt/homebrew/bin", "/usr/local/bin"])
  );
}

#[test]
fn login_shell_path_lookup_names_the_folders_searched_when_not_found() {
  let root = tempfile::tempdir().expect("temporary directory");
  let [first, second, shell] = ["first", "second", "shell"].map(|name| {
    let folder = root.path().join(name);
    fs::create_dir(&folder).expect("folder");
    folder
  });
  let inherited = env::join_paths([first.as_path(), Path::new("relative"), second.as_path()]).expect("a PATH");
  let search = SearchPath::merge(Some(&inherited), std::slice::from_ref(&shell), &[]);

  let ProgramLookup::NotFound { searched } = search.lookup(PROGRAM) else {
    panic!("no copy of {PROGRAM} exists yet");
  };
  let searched: Vec<(&Path, FolderSource)> = searched.iter().map(|folder| (folder.path(), folder.source())).collect();
  assert_eq!(
    searched,
    [
      (first.as_path(), FolderSource::Inherited),
      (second.as_path(), FolderSource::Inherited),
      (shell.as_path(), FolderSource::LoginShell),
    ]
  );

  let copies = [&shell, &second].map(|folder| {
    let copy = folder.join(program_file());
    fs::write(&copy, "").expect("a copy of the program");
    copy
  });
  assert_eq!(
    search.lookup(PROGRAM),
    ProgramLookup::Found(vec![copies[1].clone(), copies[0].clone()])
  );
}

/// A fake login shell whose start-up prints greetings and puts a folder first on the PATH; it runs
/// the script it is given (`-l -i -c <script>`) with `/bin/sh`.
#[cfg(target_os = "macos")]
#[test]
fn login_shell_path_of_a_shell_with_a_noisy_profile() {
  use std::os::unix::fs::PermissionsExt;

  let folder = tempfile::tempdir().expect("temporary directory");
  let shell = folder.path().join("fake-shell");
  fs::write(
    &shell,
    "#!/bin/sh\necho 'Welcome to the fake shell'\necho 'profile noise' >&2\nPATH=\"/opt/fake/bin:$PATH\"\nexport PATH\nexec /bin/sh -c \"$4\"\n",
  )
  .expect("the fake shell");
  fs::set_permissions(&shell, fs::Permissions::from_mode(0o755)).expect("the fake shell runs");

  let folders = login_shell::read(&shell, login_shell::TIMEOUT).expect("the PATH is read");
  assert_eq!(folders.first(), Some(&PathBuf::from("/opt/fake/bin")), "{folders:?}");
}

fn joined(folders: &[&str]) -> OsString {
  env::join_paths(folders).expect("folders a PATH can hold")
}

/// A folder no `PATH` can hold: its name has the separator, or a quote on Windows.
fn unjoinable() -> PathBuf {
  if cfg!(windows) {
    PathBuf::from("C:\\with\"quote")
  } else {
    PathBuf::from("/with:colon")
  }
}

fn program_file() -> String {
  if cfg!(windows) {
    format!("{PROGRAM}.exe")
  } else {
    PROGRAM.to_owned()
  }
}

#[cfg(windows)]
fn long_running() -> (&'static str, &'static [&'static str]) {
  ("ping", &["-n", "30", "127.0.0.1"])
}

#[cfg(not(windows))]
fn long_running() -> (&'static str, &'static [&'static str]) {
  ("sleep", &["30"])
}

/// A shell that starts a program in the background, which holds the shell's output open for
/// seconds after the shell exited, then prints `output` if there is one.
#[cfg(windows)]
fn leaving_a_background_job(output: Option<&str>) -> (&'static str, Vec<String>) {
  let mut args = ["/C", "start", "/b", "ping", "-n", "10", "127.0.0.1"]
    .map(String::from)
    .to_vec();
  if let Some(output) = output {
    args.extend(["&", "echo", output].map(String::from));
  }
  ("cmd", args)
}

#[cfg(not(windows))]
fn leaving_a_background_job(output: Option<&str>) -> (&'static str, Vec<String>) {
  let echo = output.map(|output| format!(" echo '{output}'")).unwrap_or_default();
  ("/bin/sh", vec!["-c".to_owned(), format!("sleep 10 &{echo}")])
}
