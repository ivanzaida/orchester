//! The language of a tracked file, from its name: the languages with a grammar get outlines and
//! symbols, Markdown gets its headings read, and every other file is named for `tree.txt` only.

/// Languages without a grammar, by file extension (lowercase).
const BY_EXTENSION: &[(&str, &str)] = &[
  ("bash", "shell"),
  ("bat", "batch"),
  ("c", "c"),
  ("cc", "cpp"),
  ("cmd", "batch"),
  ("cpp", "cpp"),
  ("cs", "csharp"),
  ("css", "css"),
  ("csv", "csv"),
  ("cxx", "cpp"),
  ("gradle", "gradle"),
  ("h", "c"),
  ("hpp", "cpp"),
  ("htm", "html"),
  ("html", "html"),
  ("ini", "ini"),
  ("java", "java"),
  ("json", "json"),
  ("jsonc", "json"),
  ("jsonl", "jsonl"),
  ("kt", "kotlin"),
  ("less", "css"),
  ("lock", "lock"),
  ("lua", "lua"),
  ("php", "php"),
  ("ps1", "powershell"),
  ("psm1", "powershell"),
  ("rb", "ruby"),
  ("scss", "css"),
  ("sh", "shell"),
  ("sql", "sql"),
  ("svg", "svg"),
  ("swift", "swift"),
  ("toml", "toml"),
  ("txt", "text"),
  ("vue", "vue"),
  ("xml", "xml"),
  ("yaml", "yaml"),
  ("yml", "yaml"),
  ("zsh", "shell"),
];

/// Languages without a grammar, by whole file name.
const BY_NAME: &[(&str, &str)] = &[("Dockerfile", "dockerfile"), ("Makefile", "make"), ("LICENSE", "text")];

/// What `tree.txt` says of a file that is no text, a link or a submodule, and of text of no other
/// language.
const SPECIAL: &[&str] = &["binary", "symlink", "submodule", "other"];

/// A file's language.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Language {
  Rust,
  TypeScript,
  Tsx,
  JavaScript,
  Python,
  Go,
  Markdown,
  /// A language named in `tree.txt` only.
  Plain(&'static str),
}

impl Language {
  pub(crate) const BINARY: Self = Self::Plain("binary");
  pub(crate) const SYMLINK: Self = Self::Plain("symlink");
  pub(crate) const SUBMODULE: Self = Self::Plain("submodule");
  pub(crate) const OTHER: Self = Self::Plain("other");

  /// The language of the text file at `path`.
  pub(crate) fn of(path: &str) -> Self {
    let name = path.rsplit('/').next().unwrap_or(path);
    if let Some((_, language)) = BY_NAME.iter().find(|(known, _)| *known == name) {
      return Self::Plain(language);
    }
    let Some((_, extension)) = name.rsplit_once('.') else {
      return Self::OTHER;
    };
    let extension = extension.to_ascii_lowercase();
    match extension.as_str() {
      "rs" => Self::Rust,
      "ts" | "mts" | "cts" => Self::TypeScript,
      "tsx" => Self::Tsx,
      "js" | "jsx" | "mjs" | "cjs" => Self::JavaScript,
      "py" | "pyi" => Self::Python,
      "go" => Self::Go,
      "md" | "markdown" | "mdx" => Self::Markdown,
      other => BY_EXTENSION
        .iter()
        .find(|(known, _)| *known == other)
        .map_or(Self::OTHER, |(_, language)| Self::Plain(language)),
    }
  }

  pub(crate) const fn name(self) -> &'static str {
    match self {
      Self::Rust => "rust",
      Self::TypeScript => "typescript",
      Self::Tsx => "tsx",
      Self::JavaScript => "javascript",
      Self::Python => "python",
      Self::Go => "go",
      Self::Markdown => "markdown",
      Self::Plain(name) => name,
    }
  }

  /// The language `tree.txt` names `name`.
  pub(crate) fn from_name(name: &str) -> Option<Self> {
    let parsed = [
      Self::Rust,
      Self::TypeScript,
      Self::Tsx,
      Self::JavaScript,
      Self::Python,
      Self::Go,
      Self::Markdown,
    ];
    if let Some(language) = parsed.into_iter().find(|language| language.name() == name) {
      return Some(language);
    }
    BY_EXTENSION
      .iter()
      .chain(BY_NAME)
      .map(|(_, language)| *language)
      .chain(SPECIAL.iter().copied())
      .find(|known| *known == name)
      .map(Self::Plain)
  }

  /// Whether files of this language have an outline and symbols.
  pub(crate) const fn has_grammar(self) -> bool {
    matches!(
      self,
      Self::Rust | Self::TypeScript | Self::Tsx | Self::JavaScript | Self::Python | Self::Go
    )
  }
}
