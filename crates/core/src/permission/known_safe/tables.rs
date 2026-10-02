//! The options of the known-safe programs other than git. Each list holds options of GNU and BSD
//! (macOS) builds that only read and print; options that take a file of paths, follow links into
//! other folders, start programs or write files are left out, so they ask.

use super::options::Options;

/// `ls` without `-L` (`--dereference`), which follows links, also those `-R` walks into.
const LS: Options = Options {
  long: &[&[
    "all",
    "almost-all",
    "directory",
    "classify",
    "human-readable",
    "inode",
    "reverse",
    "recursive",
    "size",
    "color",
    "sort",
    "group-directories-first",
  ]],
  flags: "1aAcdFghilnoprRsStuU",
  valued: "",
};

const CAT: Options = Options {
  long: &[&[
    "number",
    "number-nonblank",
    "show-all",
    "show-ends",
    "show-tabs",
    "show-nonprinting",
    "squeeze-blank",
  ]],
  flags: "AbeEnstTv",
  valued: "",
};

/// `head` and `tail`, also with a count alone (`-20`); `tail` without `-f`, which waits for more.
const HEAD_TAIL: Options = Options {
  long: &[&["lines", "bytes", "quiet", "silent", "verbose", "zero-terminated"]],
  flags: "0123456789qvz",
  valued: "nc",
};

/// `wc` without `--files0-from`, which reads the files a file names.
const WC: Options = Options {
  long: &[&["bytes", "chars", "lines", "words", "max-line-length"]],
  flags: "clmwL",
  valued: "",
};

const PWD: Options = Options {
  long: &[],
  flags: "LP",
  valued: "",
};

const ECHO: Options = Options {
  long: &[],
  flags: "neE",
  valued: "",
};

/// `which` without `--read-alias` and `--read-functions`, which read input.
const WHICH: Options = Options {
  long: &[&["all"]],
  flags: "as",
  valued: "",
};

/// `file` without `-f` (`--files-from`, a file of paths), `-m` (a magic file), `-C` (writes a compiled
/// magic file), `-z`/`-Z` (may start a decompressor), `-s` (reads devices) and `-p` (sets access times).
const FILE: Options = Options {
  long: &[&[
    "brief",
    "mime",
    "mime-type",
    "mime-encoding",
    "dereference",
    "no-dereference",
    "keep-going",
    "raw",
    "extension",
  ]],
  flags: "bhiLkNrE",
  valued: "",
};

/// `stat`. GNU's `-c`/`-f` and BSD's `-f`/`-t` differ in taking a value, so all are read as flags: an
/// attached value is refused, a separate one is an ordinary argument.
const STAT: Options = Options {
  long: &[&["dereference", "file-system", "format", "printf", "terse"]],
  flags: "cfLlnqrstx",
  valued: "",
};

/// `rg` without `--pre`, `--pre-glob`, `--search-zip` (`-z`), `--hostname-bin` and `--follow` (`-L`).
const RG: Options = Options {
  long: &[&[
    "files",
    "hidden",
    "no-ignore",
    "no-ignore-vcs",
    "no-ignore-parent",
    "no-ignore-dot",
    "no-config",
    "no-follow",
    "unrestricted",
    "ignore-case",
    "smart-case",
    "case-sensitive",
    "word-regexp",
    "line-regexp",
    "fixed-strings",
    "line-number",
    "no-line-number",
    "count",
    "count-matches",
    "files-with-matches",
    "files-without-match",
    "invert-match",
    "only-matching",
    "max-count",
    "max-depth",
    "max-filesize",
    "max-columns",
    "context",
    "after-context",
    "before-context",
    "multiline",
    "multiline-dotall",
    "json",
    "heading",
    "no-heading",
    "with-filename",
    "no-filename",
    "column",
    "vimgrep",
    "byte-offset",
    "trim",
    "stats",
    "color",
    "sort",
    "sortr",
    "type",
    "type-not",
    "type-list",
    "glob",
    "iglob",
    "glob-case-insensitive",
    "regexp",
    "file",
    "ignore-file",
    "null",
    "quiet",
    "text",
    "binary",
    "crlf",
    "pcre2",
    "engine",
    "encoding",
    "threads",
    "passthru",
    "replace",
    "no-messages",
  ]],
  flags: ".0abcFHiIlnNopPqsSuUvwx",
  valued: "ABCdeEfgjmMrtT",
};

/// The programs read by [`Options`], with `git`, `find` and `where` ruled separately.
pub(super) const OPTION_PROGRAMS: [(&str, Options); 11] = [
  ("ls", LS),
  ("cat", CAT),
  ("head", HEAD_TAIL),
  ("tail", HEAD_TAIL),
  ("wc", WC),
  ("pwd", PWD),
  ("echo", ECHO),
  ("which", WHICH),
  ("file", FILE),
  ("stat", STAT),
  ("rg", RG),
];

/// `find` tests and actions that only test or print. No leading option is listed: `-L` and `-H` follow
/// links, and BSD `find` reads them as a cluster (`-HL`).
const FIND_FLAGS: [&str; 21] = [
  "-print",
  "-print0",
  "-ls",
  "-prune",
  "-quit",
  "-empty",
  "-true",
  "-false",
  "-not",
  "-o",
  "-or",
  "-a",
  "-and",
  "-depth",
  "-xdev",
  "-mount",
  "-nouser",
  "-nogroup",
  "-readable",
  "-writable",
  "-executable",
];
/// `find` tests that take exactly one value, the next argument, also when it starts with `-`
/// (`-mtime -1`). `-files0-from`, `-exec*`, `-ok*`, `-delete`, `-fprint*`, `-fls` and `-follow` are
/// not among them.
const FIND_VALUED: [&str; 26] = [
  "-name",
  "-iname",
  "-path",
  "-ipath",
  "-wholename",
  "-iwholename",
  "-regex",
  "-iregex",
  "-regextype",
  "-type",
  "-maxdepth",
  "-mindepth",
  "-newer",
  "-mtime",
  "-mmin",
  "-atime",
  "-amin",
  "-ctime",
  "-cmin",
  "-size",
  "-perm",
  "-user",
  "-group",
  "-uid",
  "-gid",
  "-links",
];

/// The arguments of `find` that may name a file, or `None` when one is not listed: its start points,
/// then [`FIND_FLAGS`] and [`FIND_VALUED`] with their values, and `!`, `(`, `)`.
pub(super) fn find(args: &[String]) -> Option<Vec<&str>> {
  let mut candidates = Vec::new();
  let mut args = args.iter();
  while let Some(arg) = args.next() {
    candidates.push(arg.as_str());
    if FIND_VALUED.contains(&arg.as_str()) {
      candidates.push(args.next()?.as_str());
    } else if arg.starts_with('-') && !FIND_FLAGS.contains(&arg.as_str()) {
      return None;
    }
  }
  Some(candidates)
}

/// Windows `where` options that only change what is printed. `/R`, which searches a folder tree, is
/// left out.
const WHERE_FLAGS: [&str; 3] = ["/Q", "/F", "/T"];

/// The arguments of Windows `where`, or `None` when one is not a bare name pattern or a listed option:
/// `$VAR:pattern` searches the folders an environment variable names and `folder:pattern` a folder,
/// neither of which the path check sees.
pub(super) fn where_(args: &[String]) -> Option<Vec<&str>> {
  let allowed = |arg: &String| {
    WHERE_FLAGS.iter().any(|flag| arg.eq_ignore_ascii_case(flag))
      || !(arg.starts_with(['/', '-']) || arg.contains(['$', ':']))
  };
  args
    .iter()
    .all(allowed)
    .then(|| args.iter().map(String::as_str).collect())
}
