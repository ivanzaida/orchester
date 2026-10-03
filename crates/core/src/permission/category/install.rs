//! Where an install comes from, as the approval dialog names it (*crates.io · serde*): the registry
//! (the host of one an option names, else the manager's), the first package the arguments name, or
//! the file that lists the packages (`pip install -r requirements.txt`). Values of options are never
//! taken for a package (`cargo add --features derive serde` installs `serde`).

/// Options of package managers that take a value: the next argument is not a package.
const VALUED: [&str; 31] = [
  "--features",
  "-F",
  "--rename",
  "--path",
  "--git",
  "--branch",
  "--tag",
  "--rev",
  "--manifest-path",
  "-p",
  "--package",
  "--target",
  "--target-dir",
  "--vers",
  "--version",
  "--root",
  "--profile",
  "-c",
  "--constraint",
  "-e",
  "--editable",
  "--extra-index-url",
  "--save-prefix",
  "--prefix",
  "-w",
  "--workspace",
  "--filter",
  "--python",
  "--source",
  "--channel",
  "-C",
];

/// Options that name the registry the packages come from.
const REGISTRY: [&str; 4] = ["--registry", "--index", "--index-url", "-i"];

/// Options that name a file listing the packages.
const LISTED: [&str; 3] = ["-r", "--requirement", "--requirements"];

/// The host of a registry an option names (`https://user:token@pypi.example/simple?token=x` is
/// `pypi.example`): a person sees where packages come from, never a user name or token written in it.
/// `None` when it names no host (`file:///…`, `@`), which leaves the manager's own registry.
fn host_of(registry: &str) -> Option<String> {
  let rest = registry.split_once("://").map_or(registry, |(_, rest)| rest);
  let authority = rest.split(['/', '?', '#']).next().unwrap_or(rest);
  let host = authority.rsplit_once('@').map_or(authority, |(_, host)| host);
  (!host.is_empty()).then(|| host.to_owned())
}

/// Where an install comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct InstallSource {
  pub(super) registry: String,
  pub(super) package: Option<String>,
  pub(super) listed_in: Option<String>,
}

impl InstallSource {
  /// The source of an install with `args` (after its subcommand) from `registry` unless an option
  /// names another.
  pub(super) fn of(registry: &str, args: &[String]) -> Self {
    let mut source = Self {
      registry: registry.to_owned(),
      package: None,
      listed_in: None,
    };
    let mut args = args.iter();
    while let Some(arg) = args.next() {
      let (option, attached) = match arg.split_once('=') {
        Some((option, value)) if arg.starts_with('-') => (option, Some(value.to_owned())),
        _ => (arg.as_str(), None),
      };
      let takes = |names: &[&str]| names.contains(&option);
      if takes(&REGISTRY) || takes(&LISTED) || takes(&VALUED) {
        let value = attached.or_else(|| args.next().cloned());
        match value {
          Some(value) if takes(&REGISTRY) => {
            if let Some(host) = host_of(&value) {
              source.registry = host;
            }
          }
          Some(value) if takes(&LISTED) && source.listed_in.is_none() => source.listed_in = Some(value),
          _ => {}
        }
      } else if !arg.starts_with('-') && source.package.is_none() {
        source.package = Some(arg.clone());
      }
    }
    source
  }
}
