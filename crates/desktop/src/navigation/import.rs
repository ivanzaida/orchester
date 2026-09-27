//! Import resources (06.04 `W4zGKx`) is a dialog over the page it was opened from. The page's path
//! carries it as the `import` query, naming the tab it opened on, so going back closes it and the
//! page under it stays as it was; the typed routes ([`super::route`]) stay plain pages.

use orchester_core::resource::ResourceKind;

const QUERY: &str = "import";

/// The kind tabs of Import resources (`RDOlV`). Secrets are not imported, so their tab is only
/// drawn, never opened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportTab {
  All,
  Pipelines,
  Agents,
  Instructions,
  Tools,
}

impl lurq::app::component::DevtoolsInspectable for ImportTab {}

impl ImportTab {
  pub const ALL: [Self; 5] = [
    Self::All,
    Self::Pipelines,
    Self::Agents,
    Self::Instructions,
    Self::Tools,
  ];

  /// The kind the tab lists; `None` for All.
  pub const fn kind(self) -> Option<ResourceKind> {
    match self {
      Self::All => None,
      Self::Pipelines => Some(ResourceKind::Pipeline),
      Self::Agents => Some(ResourceKind::AgentProfile),
      Self::Instructions => Some(ResourceKind::Instruction),
      Self::Tools => Some(ResourceKind::Tool),
    }
  }

  const fn name(self) -> &'static str {
    match self {
      Self::All => "all",
      Self::Pipelines => "pipelines",
      Self::Agents => "agents",
      Self::Instructions => "instructions",
      Self::Tools => "tools",
    }
  }
}

/// `path` with Import resources open over it on `tab`.
pub fn open(path: &str, tab: ImportTab) -> String {
  let under = close(path);
  let separator = if under.contains('?') { '&' } else { '?' };
  format!("{under}{separator}{QUERY}={}", tab.name())
}

/// The tab Import resources shows over `path`; `None` while it is closed. A tab that does not
/// exist opens on All.
pub fn tab(path: &str) -> Option<ImportTab> {
  let (_, query) = path.split_once('?')?;
  let value = query
    .split('&')
    .find_map(|pair| pair.strip_prefix(QUERY)?.strip_prefix('='))?;
  Some(
    ImportTab::ALL
      .into_iter()
      .find(|tab| tab.name() == value)
      .unwrap_or(ImportTab::All),
  )
}

/// `path` without Import resources over it.
pub fn close(path: &str) -> String {
  let Some((page, query)) = path.split_once('?') else {
    return path.to_owned();
  };
  let rest: Vec<&str> = query
    .split('&')
    .filter(|pair| pair.split('=').next() != Some(QUERY))
    .collect();
  if rest.is_empty() {
    page.to_owned()
  } else {
    format!("{page}?{}", rest.join("&"))
  }
}
