//! The scheme's registration: the installer writes it per user and the uninstaller removes it; the app never writes it,
//! so a development build or a QA copy never takes links.

use std::{fs, path::Path};

/// The installer's script, `tools/installer/orchester.iss`.
fn script() -> String {
  let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/installer/orchester.iss");
  fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The `[Registry]` entries of the script that write under `key`.
fn entries(script: &str, key: &str) -> Vec<String> {
  let subkey = format!("Subkey: \"{key}\"");
  script
    .lines()
    .skip_while(|line| line.trim() != "[Registry]")
    .skip(1)
    .take_while(|line| !line.starts_with('['))
    .filter(|line| line.contains(&subkey))
    .map(str::to_owned)
    .collect()
}

#[test]
fn open_link_installer_registers_the_scheme_per_user() {
  let script = script();
  let scheme = entries(&script, r"Software\Classes\orchester");
  assert_eq!(
    scheme,
    [
      r#"Root: HKCU; Subkey: "Software\Classes\orchester"; ValueType: string; ValueName: ""; ValueData: "URL:Orchester link"; Flags: uninsdeletekey"#,
      r#"Root: HKCU; Subkey: "Software\Classes\orchester"; ValueType: string; ValueName: "URL Protocol"; ValueData: """#,
    ],
    "the scheme's key, a URL protocol, which the uninstaller removes whole"
  );
  assert_eq!(
    entries(&script, r"Software\Classes\orchester\DefaultIcon"),
    [
      r#"Root: HKCU; Subkey: "Software\Classes\orchester\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: """{app}\orchester.exe"",0""#
    ]
  );
  assert_eq!(
    entries(&script, r"Software\Classes\orchester\shell\open\command"),
    [
      r#"Root: HKCU; Subkey: "Software\Classes\orchester\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\orchester.exe"" --open-link ""%1""""#
    ],
    "the installed program with the switch and the link"
  );
  assert!(
    script.contains("PrivilegesRequired=lowest"),
    "a per-user install, which writes HKCU only"
  );
}

#[test]
fn open_link_installer_alone_registers_the_scheme() {
  let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
  let mut written = Vec::new();
  visit(&source, &mut |path, text| {
    let text = text.to_ascii_lowercase();
    if [r"classes\orchester", r"classes\\orchester", "url protocol"]
      .iter()
      .any(|key| text.contains(key))
    {
      written.push(path.display().to_string());
    }
  });
  assert!(written.is_empty(), "the app never registers the scheme: {written:?}");
}

fn visit(folder: &Path, found: &mut impl FnMut(&Path, &str)) {
  for entry in fs::read_dir(folder).expect("a source folder").flatten() {
    let path = entry.path();
    if path.is_dir() {
      visit(&path, found);
    } else if path.extension().is_some_and(|extension| extension == "rs") {
      found(&path, &fs::read_to_string(&path).expect("a source file"));
    }
  }
}
