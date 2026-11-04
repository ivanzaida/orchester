//! The execution bundle's wire shape (S8, with S7's module, branch and base commit): its fields in canonical order,
//! derived from its inputs and refused when they do not match, unknown fields refused, no credential, path or work,
//! and its hash the same for the same bundle.

use std::{
  collections::{BTreeMap, BTreeSet},
  num::NonZeroU32,
};

use orchester_api::{
  SharedLimits,
  v2::{
    BundleFileError, BundleStart, EXECUTION_BUNDLE_SCHEMA, ExecutionBundle, ExecutionBundleError, ResourceLabel,
    Subject, TaskAtStart,
  },
};
use orchester_core::{
  blob::ContentHash,
  connection::AgentProvider,
  id::{ClaimId, ConnectionId, ModuleId, ProjectId, ResourceId, TaskId, VersionId},
  name::Name,
  permission::StepPermissions,
  resource::{
    AgentProfile, AgentTargets, CommandAction, CommandArg, CommandProgram, Deliverable, InjectionEvent, Instruction,
    InstructionScope, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, Skill, SkillFile,
    SkillFilePath, Slug, Step, StepKey, StepKind, Tool, Version, VersionNumber,
  },
  run::{
    BranchName, CommitId, PublishedVersions, RunSnapshot,
    bundle::{CanonicalExecutionInputs, PinnedBodies, bundle_sha256, canonical_bytes, file_sha256},
  },
  sharing::TaskRevision,
  time::Timestamp,
};
use serde_json::{Value, json};

fn version<B>(number: u32, body: B) -> Version<B> {
  let number = VersionNumber::new(NonZeroU32::new(number).expect("positive version"));
  Version::new(VersionId::generate(), number, body, Timestamp::from_unix_millis(1))
}

fn step(key: &str, kind: StepKind) -> Step {
  Step {
    key: StepKey::new(key).expect("valid key"),
    name: Name::new(key).expect("valid name"),
    kind,
    instructions: PromptText::new("").expect("valid prompt"),
    depends_on: BTreeSet::new(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  }
}

/// A builder step and a check tool step, one instruction and one skill with a reference file.
struct Fixture {
  builder: ResourceId,
  inputs: CanonicalExecutionInputs,
}

fn fixture() -> Fixture {
  let (pipeline, builder, tool) = (ResourceId::generate(), ResourceId::generate(), ResourceId::generate());
  let agent = StepKind::Agent {
    profile: Some(builder),
    repository: RepositoryAccess::Write,
    model: None,
    permissions: StepPermissions::default(),
  };
  let steps = vec![
    step("implement", agent),
    step("check", StepKind::Tool { tool: Some(tool) }),
  ];
  let pipeline_version = version(3, Pipeline::new(steps).expect("valid pipeline"));
  let profile = AgentProfile::new(
    Name::new("Builder").expect("valid role"),
    "Builds.".into(),
    PromptText::new("Work.").expect("valid prompt"),
    ConnectionId::generate(),
    ModelId::new("model-b").expect("valid model"),
    ProviderSettings::default(),
  );
  let program = CommandProgram::new("cargo").expect("valid program");
  let check = Tool::command(
    "Checks.",
    CommandAction::new(program, vec![CommandArg::new("test").expect("valid")]),
  );
  let rule = Instruction::new(
    PromptText::new("Rule.").expect("valid prompt"),
    AgentTargets::All,
    BTreeSet::from([InjectionEvent::Start]),
    InstructionScope::Project,
  )
  .expect("valid instruction");
  let file = SkillFile {
    path: SkillFilePath::new("references/checks.md").expect("valid path"),
    content: file_sha256(b"checks"),
    size: 6,
  };
  let skill = Skill::new(
    "Checks.".into(),
    "Run them.".into(),
    vec![file],
    InstructionScope::Project,
  )
  .expect("skill");
  let profiles = BTreeMap::from([(builder, version(2, profile))]);
  let tools = BTreeMap::from([(tool, version(1, check))]);
  let instructions = BTreeMap::from([(ResourceId::generate(), version(5, rule))]);
  let skills = BTreeMap::from([(ResourceId::generate(), version(4, skill))]);
  let published = PublishedVersions {
    profiles: &profiles,
    tools: &tools,
    instructions: &instructions,
    skills: &skills,
  };
  let snapshot = RunSnapshot::build(pipeline, &pipeline_version, published).expect("published");
  let labels = [
    (pipeline_version.id(), label("feature-delivery", "Feature delivery")),
    (profiles[&builder].id(), label("builder", "Builder")),
    (tools[&tool].id(), label("cargo-check", "Cargo check")),
    (only(&instructions).id(), label("house-rules", "House rules")),
    (only(&skills).id(), label("checks", "Checks")),
  ];
  let bodies = PinnedBodies {
    profiles: &profiles.into_values().collect::<Vec<_>>(),
    instructions: &instructions.into_values().collect::<Vec<_>>(),
    tools: &tools.into_values().collect::<Vec<_>>(),
    skills: &skills.into_values().collect::<Vec<_>>(),
    labels: &labels,
  };
  let inputs = CanonicalExecutionInputs::pin(&snapshot, bodies).expect("the inputs pin");
  Fixture { builder, inputs }
}

fn label(slug: &str, name: &str) -> ResourceLabel {
  ResourceLabel::new(
    Slug::new(slug).expect("valid slug"),
    Name::new(name).expect("valid name"),
  )
}

fn only<B>(versions: &BTreeMap<ResourceId, Version<B>>) -> &Version<B> {
  versions.values().next().expect("one version")
}

const BASE_COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

fn start() -> BundleStart {
  BundleStart {
    project_id: ProjectId::generate(),
    subject: Subject::Task { id: TaskId::generate() },
    claim_id: ClaimId::generate(),
    module_id: ModuleId::generate(),
    branch: BranchName::new("module/github-sign-in").expect("branch"),
    base_commit: CommitId::new(BASE_COMMIT).expect("commit"),
    definition_revision: TaskRevision::try_from(7).expect("revision"),
    task_number: NonZeroU32::new(42).expect("a task number"),
    task: TaskAtStart {
      title: "Add GitHub sign-in".into(),
      brief: "Sign in with GitHub.".into(),
      criteria: vec!["It signs in.".into()],
    },
    limits: SharedLimits {
      budget_tokens: Some(1_000_000),
      max_duration_ms: None,
    },
    delivery: Deliverable::Changes,
  }
}

fn bundle() -> (ExecutionBundle, ResourceId) {
  let Fixture { builder, inputs } = fixture();
  let bundle = ExecutionBundle::new(start(), inputs, &[(builder, AgentProvider::ClaudeCodeCli)]).expect("bundle");
  (bundle, builder)
}

fn read(value: Value) -> Result<ExecutionBundle, serde_json::Error> {
  serde_json::from_value(value)
}

#[test]
fn execution_bundle_wire_shape_lists_its_fields_in_canonical_order() {
  let (bundle, builder) = bundle();
  let bytes = String::from_utf8(canonical_bytes(&bundle).expect("canonical")).expect("UTF-8");
  let fields = [
    "schema_version",
    "project_id",
    "subject",
    "claim_id",
    "module_id",
    "branch",
    "base_commit",
    "definition_revision",
    "task_number",
    "task",
    "pipeline",
    "profiles",
    "permissions",
    "limits",
    "delivery",
    "resources",
    "files",
    "skill_files",
    "bindings",
    "inputs",
  ];
  let places: Vec<usize> = fields
    .iter()
    .map(|field| {
      bytes
        .find(&format!("\"{field}\":"))
        .unwrap_or_else(|| panic!("{field} is sent"))
    })
    .collect();
  assert!(places.windows(2).all(|pair| pair[0] < pair[1]), "{bytes}");
  assert!(bytes.starts_with("{\"schema_version\":2,"), "{bytes}");
  let s7 = format!(
    "\"claim_id\":{},\"module_id\":{},\"branch\":\"module/github-sign-in\",\"base_commit\":\"{BASE_COMMIT}\",\
     \"definition_revision\":7,\"task_number\":42,\"task\":",
    serde_json::to_string(&bundle.start().claim_id).expect("an id serializes"),
    serde_json::to_string(&bundle.start().module_id).expect("an id serializes"),
  );
  assert!(bytes.contains(&s7), "{bytes}");

  let value = serde_json::to_value(&bundle).expect("serializes");
  let keys: BTreeSet<&str> = value
    .as_object()
    .expect("an object")
    .keys()
    .map(String::as_str)
    .collect();
  assert_eq!(keys, fields.into_iter().collect());
  let inputs = bundle.inputs();
  assert_eq!(
    value["pipeline"],
    json!({"resource_id": inputs.pipeline().pinned().resource, "version": 3})
  );
  assert_eq!(
    value["profiles"],
    json!([{"role": "Builder", "resource_id": builder, "version": 2}])
  );
  assert_eq!(
    value["bindings"],
    json!([{"resource_id": builder, "role": "Builder", "cli": "claude_code_cli", "model": "model-b"}])
  );
  assert_eq!(value["permissions"][0]["step"], json!("implement"));
  assert_eq!(value["permissions"].as_array().map(Vec::len), Some(1));
  assert_eq!(value["limits"], json!({"budget_tokens": 1_000_000}));
  assert_eq!(value["delivery"], json!("changes"));
  assert_eq!(value["definition_revision"], json!(7));
  assert_eq!(value["task_number"], json!(42));
  let checks = file_sha256(b"checks");
  assert_eq!(value["files"], json!([{"sha256": checks, "size_bytes": 6}]));
  assert_eq!(
    value["skill_files"],
    json!([{"resource_id": inputs.skills()[0].pinned().resource,
      "files": [{"path": "references/checks.md", "sha256": checks, "size_bytes": 6}]}])
  );
  assert_eq!(value["resources"].as_array().map(Vec::len), Some(3));
  assert_eq!(value["inputs"], serde_json::to_value(inputs).expect("serializes"));
}

#[test]
fn execution_bundle_wire_shape_labels_every_pinned_version() {
  let value = serde_json::to_value(bundle().0).expect("serializes");
  let labels = [
    ("/inputs/pipeline/label", "feature-delivery", "Feature delivery"),
    ("/inputs/profiles/0/label", "builder", "Builder"),
    ("/inputs/tools/0/label", "cargo-check", "Cargo check"),
    ("/inputs/instructions/0/label", "house-rules", "House rules"),
    ("/inputs/skills/0/label", "checks", "Checks"),
  ];
  for (pointer, slug, name) in labels {
    assert_eq!(
      value.pointer(pointer),
      Some(&json!({"slug": slug, "name": name})),
      "{pointer}"
    );
  }
}

#[test]
fn execution_bundle_wire_shape_hashes_the_same_bundle_the_same() {
  let (bundle, _) = bundle();
  let value = serde_json::to_value(&bundle).expect("serializes");
  let read_back = read(value.clone()).expect("reads");
  assert_eq!(read_back, bundle);
  let hash = bundle.sha256().expect("hashes");
  assert_eq!(read_back.sha256().expect("hashes"), hash);
  assert_eq!(bundle_sha256(&read_back).expect("hashes"), hash);
  let reference = bundle.bundle_ref().expect("a reference");
  assert_eq!(
    (reference.sha256, reference.schema_version),
    (hash.clone(), EXECUTION_BUNDLE_SCHEMA)
  );

  let mut reordered = value.clone();
  for list in ["resources", "files", "profiles", "bindings", "skill_files"] {
    reordered[list].as_array_mut().expect("a list").reverse();
  }
  reordered["inputs"]["instructions"]
    .as_array_mut()
    .expect("a list")
    .reverse();
  assert_eq!(read(reordered).expect("reads").sha256().expect("hashes"), hash);

  let changes: [(&str, Value); 19] = [
    ("/claim_id", json!(ClaimId::generate())),
    ("/module_id", json!(ModuleId::generate())),
    ("/branch", json!("module/github-sign-in-1a2b3c4d")),
    ("/base_commit", json!("f".repeat(64))),
    ("/definition_revision", json!(8)),
    ("/task_number", json!(43)),
    ("/task/title", json!("Add GitLab sign-in")),
    ("/limits", json!({})),
    ("/bindings/0/cli", json!("codex_cli")),
    ("/inputs/pipeline/label/slug", json!("delivery")),
    ("/inputs/pipeline/label/name", json!("Delivery")),
    ("/inputs/profiles/0/label/slug", json!("reviewer")),
    ("/inputs/profiles/0/label/name", json!("Reviewer")),
    ("/inputs/tools/0/label/slug", json!("cargo-test")),
    ("/inputs/tools/0/label/name", json!("Cargo test")),
    ("/inputs/instructions/0/label/slug", json!("team-rules")),
    ("/inputs/instructions/0/label/name", json!("Team rules")),
    ("/inputs/skills/0/label/slug", json!("tests")),
    ("/inputs/skills/0/label/name", json!("Tests")),
  ];
  for (pointer, changed) in changes {
    let mut value = value.clone();
    *value.pointer_mut(pointer).unwrap_or_else(|| panic!("{pointer} exists")) = changed;
    assert_ne!(read(value).expect("reads").sha256().expect("hashes"), hash, "{pointer}");
  }
}

#[test]
fn execution_bundle_wire_shape_refuses_unknown_fields() {
  let value = serde_json::to_value(bundle().0).expect("serializes");
  for pointer in [
    "",
    "/task",
    "/pipeline",
    "/profiles/0",
    "/bindings/0",
    "/files/0",
    "/skill_files/0",
    "/inputs",
    "/inputs/pipeline/label",
    "/inputs/profiles/0/label",
  ] {
    let mut value = value.clone();
    value
      .pointer_mut(pointer)
      .and_then(Value::as_object_mut)
      .expect("an object")
      .insert("work_id".into(), json!(1));
    assert!(
      read(value).expect_err("refused").to_string().contains("unknown field"),
      "{pointer}"
    );
  }
}

#[test]
fn execution_bundle_wire_shape_refuses_fields_its_inputs_do_not_decide() {
  let value = serde_json::to_value(bundle().0).expect("serializes");
  let changes: [(&str, Value, &str); 7] = [
    ("/schema_version", json!(1), "schema 1 is not 2"),
    ("/schema_version", json!(3), "schema 3 is not 2"),
    ("/pipeline/version", json!(4), "`pipeline`"),
    ("/profiles/0/role", json!("Reviewer"), "`profiles`"),
    ("/files", json!([]), "`files`"),
    ("/bindings/0/model", json!("model-x"), "`bindings`"),
    ("/permissions", json!([]), "`permissions`"),
  ];
  for (pointer, changed, error) in changes {
    let mut value = value.clone();
    *value.pointer_mut(pointer).unwrap_or_else(|| panic!("{pointer} exists")) = changed;
    assert!(
      read(value).expect_err("refused").to_string().contains(error),
      "{pointer}"
    );
  }
}

#[test]
fn execution_bundle_wire_shape_refuses_a_bad_task_number_or_label() {
  let value = serde_json::to_value(bundle().0).expect("serializes");
  let changes: [(&str, Value, &str); 7] = [
    ("/task_number", json!(0), "nonzero"),
    ("/task_number", json!("ORC-42"), "invalid type"),
    ("/task_number", json!(-1), "invalid value"),
    ("/inputs/pipeline/label/slug", json!("Feature delivery"), "slug must be"),
    ("/inputs/profiles/0/label/slug", json!(""), "slug must be"),
    ("/inputs/skills/0/label/name", json!(" "), "must not be blank"),
    ("/inputs/tools/0/label/name", json!("n".repeat(201)), "the limit is"),
  ];
  for (pointer, changed, error) in changes {
    let mut value = value.clone();
    *value.pointer_mut(pointer).unwrap_or_else(|| panic!("{pointer} exists")) = changed;
    let refused = read(value).expect_err("refused").to_string();
    assert!(refused.contains(error), "{pointer}: {refused}");
  }
  let mut missing = value.clone();
  missing.as_object_mut().expect("an object").remove("task_number");
  let refused = read(missing).expect_err("refused").to_string();
  assert!(refused.contains("missing field `task_number`"), "{refused}");
  let mut unlabelled = value;
  unlabelled["inputs"]["instructions"][0]
    .as_object_mut()
    .expect("a pinned version")
    .remove("label");
  let refused = read(unlabelled).expect_err("refused").to_string();
  assert!(refused.contains("missing field `label`"), "{refused}");
}

#[test]
fn execution_bundle_wire_shape_refuses_an_invalid_branch_or_base_commit() {
  let value = serde_json::to_value(bundle().0).expect("serializes");
  let changes: [(&str, Value, &str); 5] = [
    ("/base_commit", json!(BASE_COMMIT.to_uppercase()), "commit id"),
    ("/base_commit", json!(&BASE_COMMIT[..7]), "commit id"),
    ("/base_commit", json!("main"), "commit id"),
    ("/branch", json!("module/a..b"), "branch name"),
    ("/module_id", json!(1), "invalid type"),
  ];
  for (pointer, changed, error) in changes {
    let mut value = value.clone();
    *value.pointer_mut(pointer).unwrap_or_else(|| panic!("{pointer} exists")) = changed;
    let refused = read(value).expect_err("refused").to_string();
    assert!(refused.contains(error), "{pointer}: {refused}");
  }
  for field in ["module_id", "branch", "base_commit"] {
    let mut value = value.clone();
    value.as_object_mut().expect("an object").remove(field);
    let refused = read(value).expect_err("refused").to_string();
    assert!(refused.contains("missing field"), "{field}: {refused}");
  }
}

/// Neither a credential, an account, a connection nor a path of one computer, and not the work: S7's `work.start`
/// attaches it to the bundle.
#[test]
fn execution_bundle_wire_shape_has_no_credential_path_or_work() {
  let value = serde_json::to_value(bundle().0).expect("serializes");
  let mut keys = BTreeSet::new();
  collect_keys(&value, &mut keys);
  let forbidden = [
    "account",
    "account_id",
    "connection",
    "connection_id",
    "credential",
    "secret_value",
    "token",
    "password",
    "mcp_servers",
    "writable_folders",
    "sources",
    "worktree",
    "work_id",
  ];
  for key in forbidden {
    assert!(!keys.contains(key), "{key}");
  }
}

fn collect_keys(value: &Value, keys: &mut BTreeSet<String>) {
  match value {
    Value::Object(map) => {
      for (key, value) in map {
        keys.insert(key.clone());
        collect_keys(value, keys);
      }
    }
    Value::Array(values) => values.iter().for_each(|value| collect_keys(value, keys)),
    _ => {}
  }
}

#[test]
fn execution_bundle_wire_shape_checks_a_read_file_and_needs_every_binding() {
  let (bundle, _) = bundle();
  let checks = file_sha256(b"checks");
  assert_eq!(bundle.check_file(&checks, b"checks"), Ok(()));
  assert_eq!(
    bundle.check_file(&checks, b"other!"),
    Err(BundleFileError::Mismatch(checks.clone()))
  );
  let unknown = ContentHash::new(&"a".repeat(64)).expect("hash");
  assert_eq!(
    bundle.check_file(&unknown, b"checks"),
    Err(BundleFileError::NotInBundle(unknown))
  );

  let Fixture { builder, inputs } = fixture();
  assert_eq!(
    ExecutionBundle::new(start(), inputs, &[]),
    Err(ExecutionBundleError::MissingBinding(builder))
  );
}
