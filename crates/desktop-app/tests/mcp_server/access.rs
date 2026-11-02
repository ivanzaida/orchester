//! Who may read the discovery file, which holds the token: the person and the account the app runs as, and nobody
//! else. The two differ for a QA copy an agent starts inside Codex's Windows sandbox, which runs as the sandbox's user
//! while its environment still names the person; the copy's own account must read the file it wrote.

use std::path::{Path, PathBuf};

use super::Served;

fn discovery_file(served: &Served) -> PathBuf {
  served
    .server
    .as_ref()
    .expect("the server runs")
    .discovery_path()
    .expect("the server listens")
}

#[test]
fn the_discovery_file_admits_the_person_and_the_apps_own_account_only() {
  let served = Served::start(true);
  admits_only_its_readers(&discovery_file(&served));
}

/// The test above in a process of its own whose environment names another account than the one it runs as (SYSTEM),
/// as a sandboxed QA copy's does.
#[cfg(windows)]
#[test]
fn the_apps_own_account_reads_the_file_when_the_environment_names_another_user() {
  let output = std::process::Command::new(std::env::current_exe().expect("this test program"))
    .args([
      "--exact",
      "access::the_discovery_file_admits_the_person_and_the_apps_own_account_only",
      "--test-threads=1",
    ])
    .env("USERDOMAIN", "NT AUTHORITY")
    .env("USERNAME", "SYSTEM")
    .output()
    .expect("the test program runs");
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(
    output.status.success(),
    "{stdout}\n{}",
    String::from_utf8_lossy(&output.stderr)
  );
  assert!(stdout.contains("1 passed"), "the test ran: {stdout}");
}

/// The file's access list is its own (nothing inherited) and admits exactly this process's account and the account
/// `%USERDOMAIN%\%USERNAME%` names, as Windows reads them.
#[cfg(windows)]
fn admits_only_its_readers(file: &Path) {
  const SCRIPT: &str = r#"
$sid = [System.Security.Principal.SecurityIdentifier]
$acl = Get-Acl -LiteralPath $env:ORCHESTER_TEST_DISCOVERY
'protected ' + $acl.AreAccessRulesProtected
foreach ($rule in $acl.GetAccessRules($true, $true, $sid)) {
    'rule ' + $rule.IdentityReference.Value + ' ' + $rule.IsInherited
}
'own ' + [System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value
'named ' + (New-Object System.Security.Principal.NTAccount($env:USERDOMAIN, $env:USERNAME)).Translate($sid).Value
"#;
  let output = std::process::Command::new("powershell.exe")
    .args(["-NoProfile", "-NonInteractive", "-Command", SCRIPT])
    .env("ORCHESTER_TEST_DISCOVERY", file)
    .output()
    .expect("PowerShell runs");
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(
    output.status.success(),
    "{stdout}{}",
    String::from_utf8_lossy(&output.stderr)
  );
  let line = |prefix: &str| -> Vec<String> {
    stdout
      .lines()
      .filter_map(|line| line.trim().strip_prefix(prefix).map(str::to_owned))
      .collect()
  };
  assert_eq!(line("protected "), ["True"], "nothing is inherited: {stdout}");
  let mut rules = line("rule ");
  rules.sort();
  rules.dedup();
  let mut readers: Vec<String> = [line("own "), line("named ")]
    .concat()
    .into_iter()
    .map(|sid| format!("{sid} False"))
    .collect();
  readers.sort();
  readers.dedup();
  assert_eq!(rules, readers, "{stdout}");
}

/// The file is `0600` and owned by the account this process runs as, which a Seatbelt sandbox shares.
#[cfg(unix)]
fn admits_only_its_readers(file: &Path) {
  use std::os::unix::fs::{MetadataExt, PermissionsExt};

  let metadata = std::fs::metadata(file).expect("the discovery file");
  assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
  let own = tempfile::NamedTempFile::new().expect("a file of this process");
  assert_eq!(metadata.uid(), own.as_file().metadata().expect("its metadata").uid());
}
