//! The `sh` script Terminal runs for a clone on macOS, written here on every platform so its quoting is tested
//! everywhere. Every value is a single-quoted word, so the shell reads none of it as code.

use std::{io, path::Path};

use super::{CloneJob, ConsoleError};

/// The script that writes its own process id to `pid_file`, titles the window, lets Git ask for a sign-in, runs the
/// clone, writes Git's exit code beside `result_file` and renames it there, and after a failure waits for Enter so
/// its message stays readable. A value that is not valid Unicode can't be written in it.
pub fn terminal_script(job: &CloneJob<'_>, pid_file: &Path, result_file: &Path) -> Result<String, ConsoleError> {
  let path = |path: &Path| {
    path.to_str().map(quoted).ok_or_else(|| {
      ConsoleError::NotOpened(io::Error::new(
        io::ErrorKind::InvalidInput,
        "a path is not valid Unicode",
      ))
    })
  };
  let (pid, result) = (path(pid_file)?, path(result_file)?);
  let (pid_written, result_written) = (
    path(&pid_file.with_extension("tmp"))?,
    path(&result_file.with_extension("tmp"))?,
  );
  let words = job.words;
  Ok(format!(
    "#!/bin/sh\n\
     echo $$ > {pid_written} && mv {pid_written} {pid}\n\
     printf '\\033]0;%s\\007' {title}\n\
     unset GIT_TERMINAL_PROMPT GCM_INTERACTIVE\n\
     {git} clone -- {url} {destination}\n\
     code=$?\n\
     echo \"$code\" > {result_written} && mv {result_written} {result}\n\
     if [ \"$code\" -ne 0 ]; then\n  \
       printf '\\n%s\\n%s ' {failed} {close}\n  \
       read -r _\n\
     fi\n",
    git = path(job.git)?,
    url = quoted(job.url),
    destination = path(job.destination)?,
    title = quoted(words.title()),
    failed = quoted(words.failed()),
    close = quoted(words.close()),
  ))
}

/// `text` in single quotes for `sh`, a `'` in it closed, escaped and opened again.
fn quoted(text: &str) -> String {
  format!("'{}'", text.replace('\'', r"'\''"))
}
