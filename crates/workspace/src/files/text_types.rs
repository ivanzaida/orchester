//! The kinds of file *Open in editor* opens: text a person reads in an editor — source, configuration,
//! markup, documentation and data formats — by extension, and a few dotfiles by their exact name ([`TEXT_NAMES`]:
//! on Windows their whole name is their extension, and none of them is a kind Windows runs). A file whose name
//! has no dot opens when what it holds is text. Windows opens a file with its default program, which for some of
//! these kinds runs the file (Python, JavaScript under Windows Script Host, shell and batch scripts); those stay
//! closed there. HTML, SVG and XML open there in the browser, inside its sandbox for local files.

/// Extensions of text and code files, lowercase.
const TEXT: &[&str] = &[
  "adoc",
  "astro",
  "bash",
  "bat",
  "bib",
  "c",
  "cc",
  "cfg",
  "cjs",
  "clj",
  "cljs",
  "cmake",
  "cmd",
  "conf",
  "cpp",
  "cs",
  "css",
  "csv",
  "cxx",
  "dart",
  "diff",
  "edn",
  "elm",
  "erl",
  "ex",
  "exs",
  "fish",
  "fs",
  "fsx",
  "gemspec",
  "go",
  "gql",
  "gradle",
  "graphql",
  "groovy",
  "h",
  "hcl",
  "hh",
  "hpp",
  "hrl",
  "hs",
  "htm",
  "html",
  "hxx",
  "ini",
  "ipynb",
  "java",
  "jl",
  "js",
  "json",
  "json5",
  "jsonc",
  "jsx",
  "kt",
  "kts",
  "less",
  "lock",
  "log",
  "lua",
  "m",
  "markdown",
  "md",
  "mdx",
  "mjs",
  "mk",
  "ml",
  "mli",
  "mm",
  "nim",
  "nix",
  "patch",
  "php",
  "pl",
  "pm",
  "properties",
  "proto",
  "ps1",
  "psm1",
  "py",
  "pyi",
  "pyw",
  "r",
  "rb",
  "rs",
  "rst",
  "sass",
  "scala",
  "scss",
  "sh",
  "sql",
  "svelte",
  "svg",
  "swift",
  "tex",
  "tf",
  "toml",
  "ts",
  "tsv",
  "tsx",
  "txt",
  "v",
  "vue",
  "xml",
  "xsd",
  "xsl",
  "yaml",
  "yml",
  "zig",
  "zsh",
];

/// Dotfiles that are text, by their exact name.
const TEXT_NAMES: &[&str] = &[
  ".babelrc",
  ".bashrc",
  ".dockerignore",
  ".editorconfig",
  ".env",
  ".eslintrc",
  ".gitattributes",
  ".gitignore",
  ".gitmodules",
  ".npmrc",
  ".nvmrc",
  ".prettierrc",
  ".profile",
  ".zshrc",
];

/// Of [`TEXT`], the kinds Windows' default programs (or the usual installers of Python, Perl, Ruby and Git for
/// Windows' Bash) run when they are opened.
const RUN_ON_WINDOWS: &[&str] = &["bash", "bat", "cmd", "js", "pl", "ps1", "psm1", "py", "pyw", "rb", "sh"];

/// Whether `extension` names a text or code file.
pub fn is_text(extension: &str) -> bool {
  TEXT.iter().any(|text| text.eq_ignore_ascii_case(extension))
}

/// Whether `name` is one of the dotfiles that are text ([`TEXT_NAMES`]).
pub fn is_text_name(name: &str) -> bool {
  TEXT_NAMES.iter().any(|text| text.eq_ignore_ascii_case(name))
}

/// Whether Windows runs a file with `extension` when it is opened with its default program.
pub fn runs_on_windows(extension: &str) -> bool {
  RUN_ON_WINDOWS.iter().any(|run| run.eq_ignore_ascii_case(extension))
}
