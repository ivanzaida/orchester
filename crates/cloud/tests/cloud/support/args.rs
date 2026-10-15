//! The command line libtest would take, as far as this runner needs it (review 1, L7): name filters, `--skip`,
//! `--exact` and `--list`; the values of other options (`--test-threads 4`) are never taken for filters.

/// libtest's options that take a value as the next argument.
const WITH_VALUE: &[&str] = &[
  "--test-threads",
  "--skip",
  "--format",
  "--color",
  "--logfile",
  "--shuffle-seed",
  "-Z",
];

#[derive(Default)]
pub struct Selection {
  filters: Vec<String>,
  skips: Vec<String>,
  exact: bool,
  pub list: bool,
}

impl Selection {
  pub fn parse(arguments: impl IntoIterator<Item = String>) -> Self {
    let mut selection = Self::default();
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
      let (option, inline) = match argument.split_once('=') {
        Some((option, value)) if option.starts_with('-') => (option.to_owned(), Some(value.to_owned())),
        _ => (argument.clone(), None),
      };
      match option.as_str() {
        "--list" => selection.list = true,
        "--exact" => selection.exact = true,
        option if WITH_VALUE.contains(&option) => {
          let value = inline.or_else(|| arguments.next());
          if option == "--skip" {
            selection.skips.extend(value);
          }
        }
        option if option.starts_with('-') => {}
        _ => selection.filters.push(argument),
      }
    }
    selection
  }

  /// No name filter: the database tests run only when asked for by name (review 1, H2).
  pub fn unfiltered(&self) -> bool {
    self.filters.is_empty()
  }

  pub fn selects(&self, name: &str) -> bool {
    let matches = |filter: &String| {
      if self.exact {
        name == filter
      } else {
        name.contains(filter.as_str())
      }
    };
    self.filters.iter().any(matches) && !self.skips.iter().any(|skip| name.contains(skip.as_str()))
  }
}
