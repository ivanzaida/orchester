//! PowerShell's cmdlets by their named parameters. In a script `pwsh -Command` runs, the aliases that
//! share a name with a Unix program (`rm`, `cp`, `curl`, `cat`, …) are the cmdlets, so they are read as
//! such ([`cmdlet`]); `sc` is `Set-Content` in Windows PowerShell and the service manager `sc.exe` in
//! PowerShell 7, so it is Unknown everywhere. Parameters are matched by their full name only (an
//! abbreviation could stand for another parameter), a value may follow a colon (`-Path:x`), and a path
//! on a provider other than the file system (`env:`, `HKCU:`, `Cert:`, `Variable:`) is Unknown.

use super::programs::{Judged, Kind};

/// Reading cmdlets. They have no parameter that writes a file or starts a program; the common
/// parameters that set a variable are refused ([`SETS_VARIABLE`]).
const READS: [&str; 13] = [
  "get-content",
  "get-childitem",
  "select-string",
  "get-location",
  "test-path",
  "write-output",
  "write-host",
  "measure-object",
  "get-item",
  "resolve-path",
  "select-object",
  "set-location",
  "get-filehash",
];

/// Common parameters that set a variable, by name and alias, and `-Credential`.
const SETS_VARIABLE: [&str; 11] = [
  "outvariable",
  "ov",
  "errorvariable",
  "ev",
  "warningvariable",
  "wv",
  "informationvariable",
  "iv",
  "pipelinevariable",
  "pv",
  "credential",
];

/// Switches every writing cmdlet takes.
const COMMON_SWITCHES: [&str; 4] = ["force", "whatif", "confirm", "passthru"];

/// A cmdlet's parameters: switches and those that take a value.
struct Parameters {
  switches: &'static [&'static str],
  valued: &'static [&'static str],
}

const SET_CONTENT: Parameters = Parameters {
  switches: &["nonewline", "asbytestream"],
  valued: &["path", "literalpath", "value", "encoding"],
};
const OUT_FILE: Parameters = Parameters {
  switches: &["append", "noclobber", "nonewline"],
  valued: &["filepath", "literalpath", "encoding", "width", "inputobject"],
};
const NEW_ITEM: Parameters = Parameters {
  switches: &[],
  valued: &["path", "name", "itemtype", "type", "value"],
};
const REMOVE_ITEM: Parameters = Parameters {
  switches: &["recurse"],
  valued: &["path", "literalpath", "filter", "include", "exclude"],
};
const COPY_ITEM: Parameters = Parameters {
  switches: &["recurse", "container"],
  valued: &["path", "literalpath", "destination", "filter", "include", "exclude"],
};
const RENAME_ITEM: Parameters = Parameters {
  switches: &[],
  valued: &["path", "literalpath", "newname"],
};
const WEB_REQUEST: Parameters = Parameters {
  switches: &[
    "usebasicparsing",
    "skipcertificatecheck",
    "skiphttperrorcheck",
    "disablekeepalive",
  ],
  valued: &[
    "uri",
    "method",
    "body",
    "contenttype",
    "useragent",
    "timeoutsec",
    "maximumredirection",
    "outfile",
    "infile",
  ],
};

/// The cmdlet `program` names in a PowerShell script: an alias as its cmdlet, any other name as is.
pub(super) fn cmdlet(program: &str) -> &str {
  match program {
    "rm" | "rmdir" | "rd" | "del" | "erase" | "ri" => "remove-item",
    "cp" | "copy" | "cpi" => "copy-item",
    "mv" | "move" | "mi" => "move-item",
    "ren" | "rni" => "rename-item",
    "cat" | "type" | "gc" => "get-content",
    "ls" | "dir" | "gci" => "get-childitem",
    "echo" | "write" => "write-output",
    "pwd" | "gl" => "get-location",
    "cd" | "chdir" | "sl" => "set-location",
    "sls" => "select-string",
    "gi" => "get-item",
    "mkdir" | "md" | "ni" => "new-item",
    "curl" | "wget" | "iwr" => "invoke-webrequest",
    "irm" => "invoke-restmethod",
    other => other,
  }
}

/// The verdict for a cmdlet; `None` for any other program.
pub(super) fn judge(program: &str, args: &[String]) -> Option<Judged> {
  let parameters = match program {
    "set-content" | "add-content" => SET_CONTENT,
    "out-file" => OUT_FILE,
    "new-item" => NEW_ITEM,
    "remove-item" => REMOVE_ITEM,
    "copy-item" | "move-item" => COPY_ITEM,
    "rename-item" => RENAME_ITEM,
    "invoke-webrequest" | "invoke-restmethod" => WEB_REQUEST,
    "sc" => return Some(Judged::unknown()),
    program if READS.contains(&program) => return Some(read(program, args)),
    _ => return None,
  };
  if args.iter().any(|arg| unread(arg)) {
    return Some(Judged::unknown());
  }
  Some(read_parameters(args, &parameters).map_or_else(Judged::unknown, |given| writes(program, &given)))
}

fn read(program: &str, args: &[String]) -> Judged {
  // A path on another provider (`env:GH_TOKEN`, `Cert:\…`, a share) or `-Credential` may read a credential no
  // placing sees: held as one, as an unread part now goes to auto mode's reviewer (ORC-339).
  if reads_unplaced(args) {
    return Judged::unknown().secret();
  }
  if refused(args) {
    return Judged::unknown();
  }
  // The file readers with only the parameters that pick what they read: a known-safe read like `cat` or
  // `ls`, its paths placed as theirs (`-Stream`, a data stream of a file, is not among them).
  let known = KNOWN_READS.contains(&program)
    && args
      .iter()
      .all(|arg| parameter(arg).is_none_or(|(name, _)| KNOWN_READ_PARAMETERS.contains(&name.to_lowercase().as_str())))
    && !paths_globbed(args);
  if known {
    Judged::of(Kind::ReadOnly).known_safe()
  } else {
    Judged::of(Kind::ReadOnly)
  }
}

/// Whether a reading cmdlet's `args` cannot be read ([`unread`]) or set a variable ([`SETS_VARIABLE`]): a
/// path on another provider (`env:GH_TOKEN`, `Cert:\…`) could be a credential no placing sees.
pub(super) fn refused(args: &[String]) -> bool {
  args.iter().any(|arg| {
    unread(arg) || parameter(arg).is_some_and(|(name, _)| SETS_VARIABLE.contains(&name.to_lowercase().as_str()))
  })
}

/// Whether a reading cmdlet's `args` name a path no placing sees ([`on_provider`]) or pass `-Credential`.
fn reads_unplaced(args: &[String]) -> bool {
  args
    .iter()
    .any(|arg| on_provider(arg) || parameter(arg).is_some_and(|(name, _)| name.eq_ignore_ascii_case("credential")))
}

/// Reading cmdlets that are known-safe reads with [`KNOWN_READ_PARAMETERS`].
const KNOWN_READS: [&str; 5] = ["get-childitem", "get-item", "test-path", "get-content", "resolve-path"];
const KNOWN_READ_PARAMETERS: [&str; 11] = [
  "path",
  "literalpath",
  "filter",
  "recurse",
  "depth",
  "name",
  "force",
  "totalcount",
  "tail",
  "raw",
  "encoding",
];

/// What a writing cmdlet or a web request writes, from its parameters: the path is `-Path` (or
/// `-LiteralPath`, `-FilePath`), else the first positional argument.
fn writes(program: &str, given: &Given) -> Judged {
  let mut paths = given.named(&["path", "literalpath", "filepath"]);
  let mut positional = given.positional.iter().cloned();
  if paths.is_empty() {
    paths.extend(positional.next());
  }
  match program {
    "set-content" | "add-content" | "out-file" => Judged::writing(Kind::Writes, paths),
    "new-item" => {
      paths.extend(given.named(&["name"]));
      Judged::writing(Kind::Writes, paths)
    }
    "remove-item" | "rename-item" => {
      paths.extend(positional);
      Judged::writing(Kind::Writes, paths).deleting()
    }
    "copy-item" | "move-item" => {
      let target = given
        .named(&["destination"])
        .into_iter()
        .next()
        .or_else(|| positional.next())
        .unwrap_or_else(|| ".".to_owned());
      if program == "move-item" {
        paths.push(target);
        Judged::writing(Kind::Writes, paths).deleting()
      } else {
        Judged::writing(Kind::Writes, vec![target]).reading(paths)
      }
    }
    _ => Judged::writing(Kind::Network, given.named(&["outfile"])).reading(given.named(&["infile"])),
  }
}

/// A cmdlet's arguments by parameter.
struct Given {
  positional: Vec<String>,
  named: Vec<(String, String)>,
}

impl Given {
  fn named(&self, names: &[&str]) -> Vec<String> {
    self
      .named
      .iter()
      .filter(|(name, _)| names.contains(&name.as_str()))
      .map(|(_, value)| value.clone())
      .collect()
  }
}

/// `args` read against `parameters`; `None` when a parameter is not one of them or lacks its value.
fn read_parameters(args: &[String], parameters: &Parameters) -> Option<Given> {
  let mut given = Given {
    positional: Vec::new(),
    named: Vec::new(),
  };
  let mut args = args.iter();
  while let Some(arg) = args.next() {
    let Some((name, attached)) = parameter(arg) else {
      given.positional.push(arg.clone());
      continue;
    };
    let name = name.to_lowercase();
    if parameters.switches.contains(&name.as_str()) || COMMON_SWITCHES.contains(&name.as_str()) {
      if attached.is_some() {
        return None;
      }
    } else if parameters.valued.contains(&name.as_str()) {
      let value = match attached {
        Some(value) => value.to_owned(),
        None => args.next()?.clone(),
      };
      given.named.push((name, value));
    } else {
      return None;
    }
  }
  Some(given)
}

/// `-Name` or `-Name:value`: the name and the attached value.
fn parameter(arg: &str) -> Option<(&str, Option<&str>)> {
  let rest = arg.strip_prefix('-')?;
  if !rest.starts_with(|c: char| c.is_ascii_alphabetic()) {
    return None;
  }
  Some(match rest.split_once(':') {
    Some((name, value)) => (name, Some(value)),
    None => (rest, None),
  })
}

/// Whether a path argument of a file reader (not `-Filter`'s value, which only picks names in its folder)
/// holds a wildcard, which PowerShell expands by rules of its own.
fn paths_globbed(args: &[String]) -> bool {
  const VALUED: [&str; 7] = [
    "path",
    "literalpath",
    "filter",
    "depth",
    "totalcount",
    "tail",
    "encoding",
  ];
  let globbed = |value: &str| value.contains(['*', '?', '[']);
  let mut previous: Option<String> = None;
  args.iter().any(|arg| match parameter(arg) {
    Some((name, attached)) => {
      let name = name.to_lowercase();
      let found = attached.is_some_and(|value| name != "filter" && globbed(value));
      previous = (attached.is_none() && VALUED.contains(&name.as_str())).then_some(name);
      found
    }
    None => {
      let found = previous.as_deref() != Some("filter") && globbed(arg);
      previous = None;
      found
    }
  })
}

/// Whether `arg` cannot be read: a comma makes it an array of values (the quotes that would keep it one
/// value are gone by now), or it names a path on a provider ([`on_provider`]).
fn unread(arg: &str) -> bool {
  arg.contains(',') || on_provider(arg)
}

/// Whether `arg` (or the value attached to a parameter) names a path on a provider: any `::`
/// (`Microsoft.PowerShell.Core\FileSystem::C:\…`, `…\Environment::`), a drive other than one letter (`env:PATH`,
/// `HKCU:\…`, `Cert:\…`, `Variable:`, `Function:`, `WSMan:`), or a share (`\\host`, `//host`). A URL (`https://…`)
/// is not.
fn on_provider(arg: &str) -> bool {
  let value = parameter(arg).map_or(Some(arg), |(_, attached)| attached);
  value.is_some_and(|value| {
    value.contains("::")
      || value.starts_with("\\\\")
      || value.starts_with("//")
      || value.split_once(':').is_some_and(|(provider, rest)| {
        let url = provider.chars().all(|c| c.is_ascii_alphanumeric()) && rest.starts_with("//");
        let drive = provider.len() == 1 && provider.chars().all(|c| c.is_ascii_alphabetic());
        !(url || drive)
      })
  })
}
