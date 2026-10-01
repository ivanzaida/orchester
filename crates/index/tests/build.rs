//! What the codebase index of a fixture repository holds: its tracked files, symbols, outlines,
//! document headings and package map, never an ignored or untracked file; and an index built from
//! an earlier one after a commit equals one built from scratch.

mod support;

use std::fs;

use orchester_index::{BuildMode, Outcome};
use support::{GUIDE_MD, INDEX_TS, LIB_RS, Repo, files, read};

fn lines(text: &str) -> usize {
  text.lines().count()
}

#[test]
fn tree_lists_every_tracked_file_with_its_language_and_lines() {
  let repo = Repo::new();
  let index = repo.ensure(&repo.head()).path;
  let expected = [
    ".gitignore\tother\t2".to_owned(),
    "Cargo.toml\ttoml\t5".to_owned(),
    "README.md\tmarkdown\t1".to_owned(),
    "crates/app/Cargo.toml\ttoml\t3".to_owned(),
    format!("crates/app/src/lib.rs\trust\t{}", lines(LIB_RS)),
    "crates/app/src/main.rs\trust\t1".to_owned(),
    "crates/app/tests/it.rs\trust\t2".to_owned(),
    format!("docs/guide.md\tmarkdown\t{}", lines(GUIDE_MD)),
    "logo.png\tbinary\t-".to_owned(),
    "svc/go.mod\tother\t3".to_owned(),
    "svc/main.go\tgo\t17".to_owned(),
    "tools/app.py\tpython\t8".to_owned(),
    "web/package.json\tjson\t1".to_owned(),
    format!("web/src/index.ts\ttypescript\t{}", lines(INDEX_TS)),
    "web/src/legacy.js\tjavascript\t11".to_owned(),
  ];
  assert_eq!(read(&index, "tree.txt").lines().collect::<Vec<_>>(), expected);
}

#[test]
fn ignored_and_untracked_files_never_appear() {
  let repo = Repo::new();
  let index = repo.ensure(&repo.head()).path;
  for (name, content) in files(&index) {
    for hidden in [
      "target/",
      "debug.rs",
      "secret.env",
      "TOKEN",
      "hidden",
      "untracked",
      ".orchester",
    ] {
      assert!(!content.contains(hidden), "{name} mentions {hidden}");
      assert!(!name.contains(hidden), "{name} is {hidden}");
    }
  }
}

#[test]
fn symbols_name_rust_and_typescript_definitions_at_their_lines() {
  let repo = Repo::new();
  let symbols = read(&repo.ensure(&repo.head()).path, "symbols.tsv");
  let rows: Vec<&str> = symbols.lines().collect();
  for expected in [
    "Result\tcrates/app/src/lib.rs\t7\ttype\t",
    "Store\tcrates/app/src/lib.rs\t10\tstruct\t",
    "Error\tcrates/app/src/lib.rs\t14\tenum\t",
    "Named\tcrates/app/src/lib.rs\t18\ttrait\t",
    "shout\tcrates/app/src/lib.rs\t21\tmethod\ttrait Named",
    "Store\tcrates/app/src/lib.rs\t26\timpl\t",
    "helper\tcrates/app/src/lib.rs\t31\tmethod\timpl Store",
    "Named for Store\tcrates/app/src/lib.rs\t40\timpl\t",
    "name\tcrates/app/src/lib.rs\t41\tmethod\timpl Named for Store",
    "twice\tcrates/app/src/lib.rs\t46\tmacro\t",
    "deep\tcrates/app/src/lib.rs\t53\tfunction\tmodule nested",
    "Shape\tweb/src/index.ts\t1\tinterface\t",
    "area\tweb/src/index.ts\t2\tmethod\tinterface Shape",
    "Id\tweb/src/index.ts\t6\ttype\t",
    "Color\tweb/src/index.ts\t8\tenum\t",
    "double\tweb/src/index.ts\t15\tfunction\t",
    "area\tweb/src/index.ts\t20\tmethod\tclass Circle",
    "describe\tweb/src/index.ts\t29\tfunction\t",
    "Geometry\tweb/src/index.ts\t34\tnamespace\t",
    "origin\tweb/src/index.ts\t35\tfunction\tnamespace Geometry",
    "Widget\tweb/src/legacy.js\t1\tclass\t",
    "run\ttools/app.py\t2\tmethod\tclass Service",
    "Start\tsvc/main.go\t13\tmethod\t",
  ] {
    assert!(rows.contains(&expected), "missing {expected:?} in\n{symbols}");
  }
  // Nothing defined inside a function body, and no struct field or interface property.
  for absent in ["inner\t", "local\t", "items\t", "radius\t"] {
    assert!(!rows.iter().any(|row| row.starts_with(absent)), "{absent} listed");
  }
  let mut sorted = rows.clone();
  sorted.sort_by_key(|row| row.split('\t').next().expect("a name"));
  assert_eq!(rows, sorted, "rows are sorted by name");
}

#[test]
fn constants_and_test_definitions_are_listed_apart_from_the_code() {
  let repo = Repo::new();
  let index = repo.ensure(&repo.head()).path;
  let main = read(&index, "symbols.tsv");
  for row in main.lines() {
    let fields: Vec<&str> = row.split('\t').collect();
    assert!(!["const", "static"].contains(&fields[3]), "{row} in symbols.tsv");
    assert!(!fields[1].contains("/tests/"), "{row} in symbols.tsv");
  }
  assert_eq!(
    read(&index, "symbols-consts.tsv"),
    "LIMIT\tcrates/app/src/lib.rs\t3\tconst\t\n\
     LIMIT\tweb/src/index.ts\t13\tconst\t\n\
     NAME\tcrates/app/src/lib.rs\t5\tstatic\t\n\
     Port\tsvc/main.go\t11\tconst\t\n"
  );
  assert_eq!(
    read(&index, "symbols-tests.tsv"),
    "works\tcrates/app/tests/it.rs\t2\tfunction\t\n"
  );
  // The map still counts every definition.
  let all = [
    main,
    read(&index, "symbols-consts.tsv"),
    read(&index, "symbols-tests.tsv"),
  ]
  .iter()
  .map(|list| list.lines().count())
  .sum::<usize>();
  assert!(read(&index, "map.md").contains(&format!("{all} symbols, 3 packages.")));
}

#[test]
fn outlines_nest_items_with_line_ranges_and_signatures() {
  let repo = Repo::new();
  let index = repo.ensure(&repo.head()).path;
  let rust = read(&index, "outline/crates/app/src/lib.rs.md");
  assert_eq!(
    rust,
    "# crates/app/src/lib.rs\nrust, 56 lines, 16 symbols\n\n\
     - L3-3 pub const LIMIT: usize\n\
     - L5-5 static NAME: &str\n\
     - L7-7 pub type Result<T>\n\
     - L10-12 pub struct Store\n\
     - L14-16 pub enum Error\n\
     - L18-24 pub trait Named\n\
     \x20 - L19-19 fn name(&self) -> &str\n\
     \x20 - L21-23 fn shout(&self) -> String\n\
     - L26-38 impl Store\n\
     \x20 - L27-29 pub fn new() -> Self\n\
     \x20 - L31-37 fn helper(&self, index: usize) -> Option<&String>\n\
     - L40-44 impl Named for Store\n\
     \x20 - L41-43 fn name(&self) -> &str\n\
     - L46-50 macro_rules! twice\n\
     - L52-56 pub mod nested\n\
     \x20 - L53-55 pub fn deep() -> usize\n"
  );
  let typescript = read(&index, "outline/web/src/index.ts.md");
  assert_eq!(
    typescript,
    "# web/src/index.ts\ntypescript, 38 lines, 13 symbols\n\n\
     - L1-4 interface Shape\n\
     \x20 - L2-2 area(): number\n\
     - L6-6 type Id\n\
     - L8-11 enum Color\n\
     - L13-13 const LIMIT\n\
     - L15-15 const double = (value: number): number\n\
     - L17-27 class Circle implements Shape\n\
     \x20 - L18-18 constructor(private radius: number)\n\
     \x20 - L20-22 area(): number\n\
     \x20 - L24-26 get name(): string\n\
     - L29-32 function describe(shape: Shape): string\n\
     - L34-38 namespace Geometry\n\
     \x20 - L35-37 function origin()\n"
  );
  assert!(read(&index, "outline/svc/main.go.md").contains("- L13-15 func (s *Server) Start() error\n"));
  assert!(read(&index, "outline/tools/app.py.md").contains("- L1-3 class Service\n  - L2-3 def run(self)\n"));
  assert!(!index.join("outline/docs").exists(), "Markdown files have no outline");
}

#[test]
fn docs_lists_markdown_headings_outside_code_and_front_matter() {
  let repo = Repo::new();
  let docs = read(&repo.ensure(&repo.head()).path, "docs.md");
  assert_eq!(
    docs,
    "README.md:1:# Fixture\n\
     docs/guide.md:4:# Guide\n\
     docs/guide.md:8:## Setup\n\
     docs/guide.md:14:## Usage\n\
     docs/guide.md:20:### Details ###\n"
  );
}

#[test]
fn map_describes_packages_and_the_folders_outside_them() {
  let repo = Repo::new();
  let head = repo.head();
  let map = read(&repo.ensure(&head).path, "map.md");
  assert_eq!(
    map,
    format!(
      "# Repository map\n\n\
       Revision {head}: 15 files, 168 lines, 42 symbols, 3 packages.\n\n\
       ## crates/app: app (rust)\n\n\
       The fixture workspace.\n\n\
       - files: 4, lines: 62, symbols: 18\n\
       - entry: src/lib.rs (lib), src/main.rs (bin app)\n\
       - tests: tests/ (1 file)\n\
       - modules in crates/app/src/:\n\
       \x20 - lib.rs (56 lines)\n\
       \x20 - main.rs (1 line)\n\
       - also: Cargo.toml\n\n\
       ## svc: example.com/svc (go)\n\n\
       - files: 2, lines: 20, symbols: 5\n\
       - entry: main.go (main)\n\
       - modules:\n\
       \x20 - go.mod (3 lines)\n\
       \x20 - main.go (17 lines)\n\n\
       ## web: web (node)\n\n\
       The fixture web package.\n\n\
       - files: 3, lines: 50, symbols: 16\n\
       - entry: src/index.ts (main)\n\
       - modules in web/src/:\n\
       \x20 - index.ts (38 lines)\n\
       \x20 - legacy.js (11 lines)\n\
       - also: package.json\n\n\
       ## Outside packages\n\n\
       - docs/ (1 file)\n\
       - tools/ (1 file)\n\
       - root files: .gitignore, Cargo.toml, README.md, logo.png\n"
    )
  );
}

#[test]
fn incremental_build_reads_only_changed_files_and_equals_a_full_build() {
  let repo = Repo::new();
  let first = repo.head();
  repo.ensure(&first);
  repo.write(
    "crates/app/src/lib.rs",
    &LIB_RS.replace("pub const LIMIT", "pub const MAXIMUM"),
  );
  repo.write("crates/app/src/extra.rs", "pub fn extra() {}\n");
  repo.write("docs/second.md", "# Second\n");
  repo.git(&["rm", "--quiet", "tools/app.py"]);
  repo.git(&["mv", "README.md", "docs/README.md"]);
  repo.git(&["add", "crates", "docs"]);
  repo.git(&["commit", "--quiet", "-m", "change"]);
  let second = repo.head();

  let incremental = repo.ensure(&second);
  let Outcome::Built(report) = &incremental.outcome else {
    panic!("the second revision was built");
  };
  // lib.rs changed, extra.rs and second.md were added, app.py was deleted, README.md moved.
  assert_eq!(
    report.mode,
    BuildMode::Incremental {
      from: first,
      changed: 6
    }
  );
  assert_eq!((report.read, report.reused, report.files), (4, 12, 16));
  assert!(read(&incremental.path, "symbols-consts.tsv").contains("MAXIMUM\tcrates/app/src/lib.rs\t3\tconst\t"));
  assert!(!read(&incremental.path, "symbols.tsv").contains("tools/app.py"));
  let incremental_files = files(&incremental.path);

  fs::remove_dir_all(repo.index_root()).expect("remove the indexes");
  let full = repo.ensure(&second);
  let Outcome::Built(report) = &full.outcome else {
    panic!("the index was built again");
  };
  assert_eq!(report.mode, BuildMode::Full);
  assert_eq!(report.read, 16);
  assert_eq!(files(&full.path), incremental_files);
}
