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
/// with `base`'s variable (`$env:LOCALAPPDATA`, `$HOME`), so the command reads as the person's own. A folder name with
/// anything but letters, digits, spaces, `.`, `_`, `-`, `:` and separators is written as a single-quoted literal, so
/// nothing in it is expanded or ends the quotes: PowerShell takes the typographic quotes `“ ” „` as `"` and
/// `‘ ’ ‚ ‛` as `'`, and zsh expands `!` inside double quotes.
pub fn sign_in_command(cli: AccountCli, folder: Option<&Path>, shell: Shell, base: Option<(&Path, &str)>) -> String {
  let login = std::iter::once(cli.program())
    .chain(cli.login_args().iter().copied())
    .collect::<Vec<_>>()
    .join(" ");
  let Some(folder) = folder else {
    return login;
  };
  let value = match base.and_then(|(path, variable)| Some((folder.strip_prefix(path).ok()?, variable))) {
    Some((rest, variable)) if rest.as_os_str().is_empty() => match shell {
      Shell::PowerShell => variable.to_owned(),
      Shell::Zsh => format!("\"{variable}\""),
    },
    Some((rest, variable)) => {
      let separator = if shell == Shell::PowerShell { '\\' } else { '/' };
      let rest = format!("{separator}{}", rest.to_string_lossy());
      match shell {
        _ if plain(&rest) => format!("\"{variable}{rest}\""),
        Shell::PowerShell => format!("{variable} + {}", literal(&rest, shell)),
        Shell::Zsh => format!("\"{variable}\"{}", literal(&rest, shell)),
      }
    }
    None => {
      let folder = folder.to_string_lossy();
      if plain(&folder) {
        format!("\"{folder}\"")
      } else {
        literal(&folder, shell)
      }
    }
  };
  let variable = cli.folder_variable();
  match shell {
    Shell::PowerShell => format!("$env:{variable}={value}; {login}"),
    Shell::Zsh => format!("{variable}={value} {login}"),
  }
}

/// Whether `text` holds nothing a shell reads inside double quotes.
fn plain(text: &str) -> bool {
  text
    .chars()
    .all(|character| character.is_alphanumeric() || " ._-:\\/".contains(character))
}

/// `text` as a single-quoted literal of `shell`, in which nothing is expanded.
fn literal(text: &str, shell: Shell) -> String {
  let mut out = String::from("'");
  for character in text.chars() {
    match (shell, character) {
      // PowerShell ends a single-quoted string at any of its single quotes; doubled, each stands for itself.
      (Shell::PowerShell, '\'' | '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}') => {
        out.push(character);
        out.push(character);
      }
      (Shell::Zsh, '\'') => out.push_str("'\\''"),
      _ => out.push(character),
    }
  }
  out.push('\'');
  out
}
