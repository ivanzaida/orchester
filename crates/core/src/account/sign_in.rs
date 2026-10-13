//! The command a person can run in a terminal of their own to sign an account in: the CLI's own login with the
//! account's folder variable set for that terminal only. PowerShell on Windows, zsh on macOS, as the design shows.

use std::path::Path;

use super::AccountCli;
use crate::connection::invocation::Platform;

/// The shell a sign-in command is written for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shell {
  PowerShell,
  Zsh,
}

impl Shell {
  pub const fn of(platform: Platform) -> Self {
    match platform {
      Platform::Windows => Self::PowerShell,
      Platform::MacOs => Self::Zsh,
    }
  }

  /// The shell's name as people know it.
  pub const fn name(self) -> &'static str {
    match self {
      Self::PowerShell => "PowerShell",
      Self::Zsh => "zsh",
    }
  }
}

/// The login of `cli` for `folder` (`None`: the CLI's own folder) in `shell`. A folder under `base`'s path is written
/// with `base`'s variable (`$env:LOCALAPPDATA`, `$HOME`), so the command reads as the person's own.
pub fn sign_in_command(cli: AccountCli, folder: Option<&Path>, shell: Shell, base: Option<(&Path, &str)>) -> String {
  let login = std::iter::once(cli.program())
    .chain(cli.login_args().iter().copied())
    .collect::<Vec<_>>()
    .join(" ");
  let Some(folder) = folder else {
    return login;
  };
  let value = match base.and_then(|(path, variable)| Some((folder.strip_prefix(path).ok()?, variable))) {
    Some((rest, variable)) => {
      let separator = if shell == Shell::PowerShell { '\\' } else { '/' };
      let rest = escaped(&rest.to_string_lossy(), shell);
      if rest.is_empty() {
        variable.to_owned()
      } else {
        format!("{variable}{separator}{rest}")
      }
    }
    None => escaped(&folder.to_string_lossy(), shell),
  };
  let variable = cli.folder_variable();
  match shell {
    Shell::PowerShell => format!("$env:{variable}=\"{value}\"; {login}"),
    Shell::Zsh => format!("{variable}=\"{value}\" {login}"),
  }
}

/// `text` inside double quotes of `shell`, with nothing in it expanded.
fn escaped(text: &str, shell: Shell) -> String {
  let (escape, special): (char, &[char]) = match shell {
    Shell::PowerShell => ('`', &['`', '"', '$']),
    Shell::Zsh => ('\\', &['\\', '"', '$', '`']),
  };
  let mut out = String::with_capacity(text.len());
  for character in text.chars() {
    if special.contains(&character) {
      out.push(escape);
    }
    out.push(character);
  }
  out
}
