//! Programs that reach the network by themselves, each read against the options it is known to take
//! ([`options`]): what they upload or send is a file they read (`curl -T f`, `-d @f`, `-F x=@f`), what
//! they download a file they write (`-o`, `-O`, `wget -O`), and an option that is not listed (a config
//! file, a proxy command, a program to start) leaves the command Unknown. `ssh`, `scp` and `rsync` run
//! or copy on another machine; their later commands are never alike. `sftp`, `ftp`, `nc` and `telnet`
//! take commands Orchester cannot read and are not listed: they are Unknown.

use super::{
  network_tables::{CURL, RSYNC, SCP, SSH, WGET},
  options::{self, Parsed},
  programs::{Judged, Kind},
};

/// The verdict for a network program; `None` for any other program.
pub(super) fn judge(program: &str, args: &[String]) -> Option<Judged> {
  Some(match program {
    "curl" => options::parse(args, CURL).map_or_else(Judged::unknown, |parsed| curl(&parsed)),
    "wget" => options::parse(args, WGET).map_or_else(Judged::unknown, |parsed| wget(&parsed)),
    "ssh" => options::parse(args, SSH).map_or_else(Judged::unknown, |parsed| ssh(&parsed)),
    "scp" => options::parse(args, SCP).map_or_else(Judged::unknown, |parsed| copy(parsed, false)),
    "rsync" => options::parse(args, RSYNC).map_or_else(Judged::unknown, |parsed| {
      let deletes = parsed.options.iter().any(|(name, _)| name.starts_with("--delete"));
      copy(parsed, deletes)
    }),
    "ping" => Judged::of(Kind::Network),
    _ => return None,
  })
}

/// Options whose value `@file` reads the file.
const AT_FILE: [&str; 9] = [
  "-d",
  "--data",
  "--data-binary",
  "--data-ascii",
  "--json",
  "-H",
  "--header",
  "-w",
  "--write-out",
];

fn curl(parsed: &Parsed) -> Judged {
  let mut reads = parsed.reads.clone();
  reads.extend(
    parsed
      .values(&AT_FILE)
      .filter_map(|value| value.strip_prefix('@'))
      .map(str::to_owned),
  );
  reads.extend(parsed.values(&["--data-urlencode"]).filter_map(urlencoded_file));
  reads.extend(parsed.values(&["-F", "--form"]).filter_map(form_file));
  // A cookie without `=` is the file the cookies are read from.
  reads.extend(
    parsed
      .values(&["-b", "--cookie"])
      .filter(|value| !value.contains('='))
      .map(str::to_owned),
  );
  // `%output{file}` in a write-out format writes the file.
  if parsed
    .values(&["-w", "--write-out"])
    .any(|value| value.contains("%output{"))
  {
    return Judged::unknown();
  }
  let mut writes = parsed.writes.clone();
  let remote_name = parsed.has(&["-O", "--remote-name", "--remote-name-all", "-J", "--remote-header-name"]);
  if remote_name && !parsed.has(&["--output-dir"]) {
    writes.push(".".to_owned());
  }
  let judged = Judged::writing(Kind::Network, writes).reading(reads);
  if parsed.has(&["-n", "--netrc", "--netrc-optional"]) {
    judged.secret()
  } else {
    judged
  }
}

/// The file of `--data-urlencode [name]@file`.
fn urlencoded_file(value: &str) -> Option<String> {
  let (name, file) = value.split_once('@')?;
  (!name.contains('=')).then(|| file.to_owned())
}

/// The file of `-F name=@file` or `-F name=<file`, without its `;type=…`.
fn form_file(value: &str) -> Option<String> {
  let (_, content) = value.split_once('=')?;
  let file = content.strip_prefix(['@', '<'])?;
  Some(file.split(';').next().unwrap_or(file).to_owned())
}

/// `wget` saves into the folder it runs in unless `-O` names the file, `-P` the folder, or `--spider`
/// only checks.
fn wget(parsed: &Parsed) -> Judged {
  let mut writes = parsed.writes.clone();
  let named = parsed.has(&["-O", "--output-document", "-P", "--directory-prefix", "--spider"]);
  if !named {
    writes.push(".".to_owned());
  }
  Judged::writing(Kind::Network, writes).reading(parsed.reads.clone())
}

/// `ssh host [command]`: a host is needed; the command runs on that machine.
fn ssh(parsed: &Parsed) -> Judged {
  if parsed.operands.is_empty() {
    return Judged::unknown();
  }
  Judged::writing(Kind::Network, parsed.writes.clone())
    .reading(parsed.reads.clone())
    .not_alike()
}

/// `scp` and `rsync`: the last operand is the target, written when it is on this machine; the sources
/// on this machine are read.
fn copy(parsed: Parsed, deletes: bool) -> Judged {
  let mut sources = parsed.operands;
  let Some(target) = sources.pop() else {
    return Judged::unknown();
  };
  if sources.is_empty() {
    return Judged::unknown();
  }
  let writes = if is_remote(&target) { Vec::new() } else { vec![target] };
  let mut reads = parsed.reads;
  reads.extend(sources.into_iter().filter(|source| !is_remote(source)));
  let judged = Judged::writing(Kind::Network, writes).reading(reads).not_alike();
  if deletes { judged.deleting() } else { judged }
}

/// `host:path`, `user@host:path` or `rsync://…`, not `C:\…`.
fn is_remote(word: &str) -> bool {
  word
    .split_once(':')
    .is_some_and(|(before, _)| before.len() > 1 && !before.contains(['/', '\\']))
}
