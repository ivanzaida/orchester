//! Sources (M12): absolute paths listed on the project, a task, a pipeline (in its versions) and its steps; a run
//! snapshots their union when it starts, each path once with where it was listed first, and every step reads them;
//! a call outside the worktree runs unasked only for a path inside one.

use std::collections::BTreeMap;

use orchester_core::{
  resource::{PipelineEdit, PipelineSources, PipelineSourcesError},
  run::RunSnapshot,
  source::{ResolvedSources, RunSources, SourceError, SourceOrigin, SourcePath},
};
use serde_json::json;

use crate::fixture::{Config, key, version};

fn source(path: &std::path::Path) -> SourcePath {
  SourcePath::new(&path.to_string_lossy()).expect("an absolute path")
}

fn sample(name: &str) -> SourcePath {
  source(&std::env::temp_dir().join(name))
}

#[test]
fn sources_are_absolute_paths() {
  let path = sample("deps");
  assert_eq!(SourcePath::new(&format!("  {}  ", path.as_str())), Ok(path.clone()));
  assert_eq!(SourcePath::new("  "), Err(SourceError::Blank));
  assert_eq!(
    SourcePath::new("deps/serde"),
    Err(SourceError::NotAbsolute("deps/serde".to_owned()))
  );
  assert_eq!(
    SourcePath::new(&format!("{}\nx", path.as_str())),
    Err(SourceError::ControlCharacter)
  );
  assert_eq!(
    SourcePath::new(&format!("{}{}", path.as_str(), "x".repeat(5000))),
    Err(SourceError::TooLong)
  );
  let stored: Result<SourcePath, _> = serde_json::from_value(json!("relative"));
  assert!(stored.is_err());
}

/// A network or device path is no source: resolving the sources when an agent starts must not reach the network.
#[cfg(windows)]
#[test]
fn sources_are_local_paths() {
  for path in [
    r"\\server\share\deps",
    "//server/share/deps",
    r"\\?\UNC\server\share",
    r"\\?\C:\deps",
  ] {
    assert_eq!(
      SourcePath::new(path),
      Err(SourceError::NotLocal(path.to_owned())),
      "{path}"
    );
  }
  assert!(SourcePath::new(r"C:\deps").is_ok());
}

/// A pipeline lists its own sources and its steps' in its versions; a step it does not have is refused, and a version
/// saved before sources read as listing none.
#[test]
fn sources_live_in_pipeline_versions() {
  let config = Config::feature_delivery();
  let pipeline = config.pipeline_version.body().clone();
  assert!(pipeline.sources().is_empty());
  let stored = serde_json::to_value(&pipeline).expect("serializes");
  assert!(stored.get("sources").is_none(), "{stored}");
  let listed = PipelineSources::new(
    vec![sample("deps"), sample("deps")],
    BTreeMap::from([(key("review"), vec![sample("docs")]), (key("plan"), Vec::new())]),
  );
  assert_eq!(listed.pipeline(), [sample("deps")]);
  let pipeline = pipeline.with_sources(listed).expect("its own steps");
  assert_eq!(pipeline.sources().step(&key("review")), [sample("docs")]);
  assert!(pipeline.sources().step(&key("plan")).is_empty());
  let again: orchester_core::resource::Pipeline =
    serde_json::from_value(serde_json::to_value(&pipeline).expect("serializes")).expect("reads back");
  assert_eq!(again, pipeline);
  let unknown = PipelineSources::new(Vec::new(), BTreeMap::from([(key("deploy"), vec![sample("x")])]));
  assert_eq!(
    pipeline.clone().with_sources(unknown),
    Err(PipelineSourcesError::UnknownStep(key("deploy")))
  );
  // Editing keeps the sources; a removed step's go with it.
  let mut edit = PipelineEdit::new(pipeline);
  assert!(edit.remove_step(&key("review")));
  let edited = edit.validate().expect("valid");
  assert_eq!(edited.sources().pipeline(), [sample("deps")]);
  assert!(edited.sources().step(&key("review")).is_empty());
}

/// A run snapshots the union of the project's, the task's, the pipeline's and its steps' sources when it starts, each
/// path once with where it was listed first; a snapshot stored before sources reads as having none.
#[test]
fn sources_of_a_run_are_the_union_it_snapshotted() {
  let mut config = Config::feature_delivery();
  let listed = PipelineSources::new(
    vec![sample("pipeline"), sample("shared")],
    BTreeMap::from([(key("review"), vec![sample("review"), sample("task")])]),
  );
  let body = config
    .pipeline_version
    .body()
    .clone()
    .with_sources(listed)
    .expect("its own steps");
  config.pipeline_version = version(3, body);
  let snapshot = config
    .snapshot()
    .with_sources(&[sample("project"), sample("shared")], &[sample("task")]);
  let entries: Vec<(&str, &SourceOrigin)> = snapshot
    .sources()
    .entries()
    .iter()
    .map(|source| (source.path.as_str(), &source.origin))
    .collect();
  assert_eq!(
    entries,
    [
      (sample("project").as_str(), &SourceOrigin::Project),
      (sample("shared").as_str(), &SourceOrigin::Project),
      (sample("task").as_str(), &SourceOrigin::Task),
      (sample("pipeline").as_str(), &SourceOrigin::Pipeline),
      (sample("review").as_str(), &SourceOrigin::Step { step: key("review") }),
    ]
  );
  let stored = serde_json::to_value(&snapshot).expect("serializes");
  let read: RunSnapshot = serde_json::from_value(stored.clone()).expect("reads back");
  assert_eq!(read, snapshot);
  let mut earlier = stored;
  earlier.as_object_mut().expect("an object").remove("sources");
  let earlier: RunSnapshot = serde_json::from_value(earlier).expect("an earlier snapshot reads");
  assert!(earlier.sources().is_empty());
}

/// A call stays in the sources when its resolved path is a source or lies in one, compared by whole names.
#[test]
fn sources_cover_what_lies_in_them() {
  let root = tempfile::tempdir().expect("a folder");
  let deps = root.path().join("deps");
  std::fs::create_dir_all(deps.join("serde")).expect("a folder");
  std::fs::write(root.path().join("notes.txt"), "x").expect("a file");
  let sources = RunSources::union([
    (&source(&deps), SourceOrigin::Project),
    (&source(&root.path().join("notes.txt")), SourceOrigin::Task),
  ]);
  let resolved = ResolvedSources::resolve(&sources);
  let canonical =
    |path: &std::path::Path| orchester_core::native::scope::resolve_local(&path.to_string_lossy()).expect("exists");
  assert!(resolved.covers(&canonical(&deps)));
  assert!(resolved.covers(&canonical(&deps.join("serde"))));
  assert!(resolved.covers(&canonical(&root.path().join("notes.txt"))));
  assert!(!resolved.covers(&canonical(root.path())));
  std::fs::create_dir_all(root.path().join("deps-b")).expect("a folder");
  assert!(!resolved.covers(&canonical(&root.path().join("deps-b"))));
  assert!(!ResolvedSources::default().covers(&canonical(&deps)));
}
