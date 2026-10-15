//! How the Windows launcher hands a path to Explorer: always in double quotes, so a comma in it does not split it
//! (Explorer reads a bare argument as options separated by commas). Built as a string; Explorer is not started.

use std::ffi::OsStr;

use orchester_exec::external::explorer_argument;

#[test]
fn external_explorer_argument_quotes_every_path() {
  let quoted = |target: &str| {
    explorer_argument(OsStr::new(target))
      .expect("a path without quotes")
      .into_string()
      .expect("UTF-8")
  };
  assert_eq!(
    quoted(r"C:\repo\evil.hta,z\notes.txt"),
    r#""C:\repo\evil.hta,z\notes.txt""#
  );
  assert_eq!(quoted(r"\\?\C:\a b\c.md"), r#""\\?\C:\a b\c.md""#);
  assert_eq!(quoted("https://example.com/a,b"), r#""https://example.com/a,b""#);
  assert!(
    explorer_argument(OsStr::new(r#"C:\a"b.txt"#)).is_err(),
    "a quote would end the quotes"
  );
}
