//! Paths as the run's roots compare them ([`super::RunRoots`]): absolute, with `.` and `..` taken away, links and
//! junctions followed as far as they exist (on this platform only), and the spellings of one file made one: on Windows
//! `\\?\` and `\\.\` prefixes, Git Bash's `/c/…`, `/` for `\`, any case, trailing dots and spaces and a stream
//! (`file::$DATA`); on macOS the `/private` aliases of `/var`, `/tmp` and `/etc`, and any case, as the default APFS
//! volume ignores it. Each platform is read by its own rules on either, so both are tested on one machine.

use std::path::{Path, PathBuf};

use crate::{connection::invocation::Platform, permission::canonical::existing};

/// A path as the roots compare it: what it is shown as, and the names they compare.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RootPath {
  shown: PathBuf,
  /// The root (`C:`, `\\host\share`, `/`) and each name after it, folded where the file system ignores case.
  names: Vec<String>,
}

impl RootPath {
  /// `path` as `platform` reads it, relative to `base` when it is not absolute; `None` when it cannot be told: a
  /// relative path without a base, one that starts with `~` (no shell expands it here), a Windows path rooted without
  /// a drive (`\x`, `/etc/hosts`) or on a drive without a root (`C:x`), or a path whose links cannot be read.
  pub fn new(platform: Platform, path: &str, base: Option<&Self>) -> Option<Self> {
    if path.is_empty() || path.starts_with('~') {
      return None;
    }
    let full = if is_relative(platform, path) {
      format!("{}{}{path}", base?.shown.to_string_lossy(), separators(platform)[0])
    } else {
      path.to_owned()
    };
    let written = Self::of_names(platform, names(platform, &full)?)?;
    if platform != Platform::current() {
      return Some(written);
    }
    let found = existing(&written.shown).ok()?;
    Self::of_names(platform, names(platform, &found.path.to_string_lossy())?)
  }

  /// A root folder, as written: absolute, or not a root.
  pub(super) fn folder(platform: Platform, path: &Path) -> Option<Self> {
    Self::new(platform, &path.to_string_lossy(), None)
  }

  /// The path of `names` (the root first), each spelling of one file made one.
  fn of_names(platform: Platform, mut names: Vec<String>) -> Option<Self> {
    if platform == Platform::MacOs && names.len() > 2 && names[1] == "private" && ALIASED.contains(&names[2].as_str()) {
      names.remove(1);
    }
    let shown = match platform {
      Platform::Windows => {
        let root = names.first()?;
        PathBuf::from(format!("{root}\\{}", names[1..].join("\\")))
      }
      Platform::MacOs => PathBuf::from(format!("/{}", names[1..].join("/"))),
    };
    let names = names
      .iter()
      .enumerate()
      .map(|(at, name)| {
        if at == 0 {
          name.to_lowercase()
        } else {
          compared(platform, name)
        }
      })
      .collect();
    Some(Self { shown, names })
  }

  /// The path as it is shown, in `platform`'s own form.
  pub fn path(&self) -> &Path {
    &self.shown
  }

  /// Whether this path is `folder` or lies in it.
  pub fn is_in(&self, folder: &Self) -> bool {
    self.names.starts_with(&folder.names)
  }
}

/// The macOS folders `/private` holds whose aliases at the root (`/tmp` → `/private/tmp`) name them too.
const ALIASED: [&str; 3] = ["var", "tmp", "etc"];

fn separators(platform: Platform) -> &'static [char] {
  match platform {
    Platform::Windows => &['\\', '/'],
    Platform::MacOs => &['/'],
  }
}

/// Whether `path` names no root at all: on Windows neither a separator nor a drive at its start.
fn is_relative(platform: Platform, path: &str) -> bool {
  match platform {
    Platform::MacOs => !path.starts_with('/'),
    Platform::Windows => !path.starts_with(['\\', '/']) && path.as_bytes().get(1) != Some(&b':'),
  }
}

/// The root of the absolute `path` and each name after it, `.` and `..` taken away; `None` when its root cannot be
/// read.
fn names(platform: Platform, path: &str) -> Option<Vec<String>> {
  let (root, rest) = match platform {
    Platform::MacOs => (String::new(), path.strip_prefix('/')?.to_owned()),
    Platform::Windows => windows_root(&path.replace('/', "\\"))?,
  };
  let mut names = vec![root];
  for name in rest.split(separators(platform)) {
    match name {
      "" | "." => {}
      ".." => {
        if names.len() > 1 {
          names.pop();
        }
      }
      name => names.push(name.to_owned()),
    }
  }
  Some(names)
}

/// A Windows path's root, a drive (`C:`) or a share (`\\host\share`), without `\\?\`, `\\.\` or `\\?\UNC\`; Git
/// Bash's `\c\…` is `C:\…`.
fn windows_root(path: &str) -> Option<(String, String)> {
  if let Some(share) = path.strip_prefix(r"\\?\UNC\") {
    return share_root(share);
  }
  if let Some(local) = path.strip_prefix(r"\\?\").or_else(|| path.strip_prefix(r"\\.\")) {
    return drive_root(local);
  }
  if let Some(share) = path.strip_prefix(r"\\") {
    return share_root(share);
  }
  let bytes = path.as_bytes();
  if bytes.len() >= 2 && bytes[0] == b'\\' && bytes[1].is_ascii_alphabetic() && bytes.get(2).is_none_or(|c| *c == b'\\')
  {
    return Some((
      format!("{}:", char::from(bytes[1]).to_ascii_uppercase()),
      path[2..].to_owned(),
    ));
  }
  drive_root(path)
}

fn drive_root(path: &str) -> Option<(String, String)> {
  let bytes = path.as_bytes();
  let rooted =
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes.get(2).is_none_or(|c| *c == b'\\');
  rooted.then(|| {
    (
      format!("{}:", char::from(bytes[0]).to_ascii_uppercase()),
      path[2..].to_owned(),
    )
  })
}

fn share_root(share: &str) -> Option<(String, String)> {
  let mut names = share.splitn(3, '\\');
  let (host, folder) = (
    names.next().filter(|name| !name.is_empty())?,
    names.next().filter(|name| !name.is_empty())?,
  );
  Some((
    format!(r"\\{host}\{folder}"),
    names.next().unwrap_or_default().to_owned(),
  ))
}

/// `name`, one after the root, as `platform`'s file system compares it.
fn compared(platform: Platform, name: &str) -> String {
  match platform {
    // `a.txt:stream` is a stream of `a.txt`, and Windows drops a name's trailing dots and spaces.
    Platform::Windows => {
      let file = name.split(':').next().unwrap_or(name);
      file.trim_end_matches(['.', ' ']).to_lowercase()
    }
    Platform::MacOs => name.to_lowercase(),
  }
}
