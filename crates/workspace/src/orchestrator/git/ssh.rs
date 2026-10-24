//! The ssh command network git runs with, made to fail at once instead of asking for a passphrase, a password or
//! whether to trust a host: `-o BatchMode=yes` for OpenSSH and any other ssh, `-batch` for PuTTY's plink and
//! TortoisePlink, which take no `-o`. It starts from what the caller or the repository set: `GIT_SSH_COMMAND`, else a
//! `GIT_SSH` program (quoted for the shell git runs the command with, so git still tells its variant by its name),
//! else `core.sshCommand`, else `ssh`.

use std::env;

use super::command::Git;
use crate::app_mcp::ToolError;

/// The ssh command network git runs with in `git`'s repository.
pub(super) fn batch_ssh(git: &Git) -> Result<String, ToolError> {
  if let Ok(command) = env::var("GIT_SSH_COMMAND")
    && !command.trim().is_empty()
  {
    return Ok(batch_ssh_command(&command));
  }
  if let Some(program) = env::var_os("GIT_SSH").filter(|program| !program.is_empty()) {
    return Ok(batch_ssh_program(&program.to_string_lossy()));
  }
  let configured = git.run(&["config", "--get", "core.sshCommand"], None)?;
  let base = configured.stdout.trim();
  if configured.success() && !base.is_empty() {
    return Ok(batch_ssh_command(base));
  }
  Ok(batch_ssh_command("ssh"))
}

/// `command`, an ssh command line as `GIT_SSH_COMMAND` or `core.sshCommand` holds it, with the option that keeps its
/// program from asking anything: `-batch` for plink and TortoisePlink, else `-o BatchMode=yes`.
pub fn batch_ssh_command(command: &str) -> String {
  let command = command.trim();
  format!("{command} {}", option_for(first_word(command)))
}

/// `program`, a program `GIT_SSH` names, as a command line with that option, quoted for the shell git runs it with.
pub fn batch_ssh_program(program: &str) -> String {
  let quoted = format!("'{}'", program.replace('\'', r"'\''"));
  format!("{quoted} {}", option_for(program))
}

/// The option that keeps the ssh program `program` from asking anything.
fn option_for(program: &str) -> &'static str {
  let name = program.rsplit(['/', '\\']).next().unwrap_or(program).to_lowercase();
  let name = name.strip_suffix(".exe").unwrap_or(&name);
  if matches!(name, "plink" | "tortoiseplink") {
    "-batch"
  } else {
    "-o BatchMode=yes"
  }
}

/// The program a command line starts with, without the quotes around it.
fn first_word(command: &str) -> &str {
  for quote in ['\'', '"'] {
    if let Some(rest) = command.strip_prefix(quote) {
      return rest.split(quote).next().unwrap_or(rest);
    }
  }
  command.split_whitespace().next().unwrap_or(command)
}
