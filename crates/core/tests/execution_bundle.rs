//! The complete resolved inputs an execution bundle carries and their canonical hash (S8): pinned from a run snapshot
//! with the bodies of its versions, without a path of one computer or a connection, read back only when consistent,
//! and hashed the same whenever they are the same.

use std::{
  collections::{BTreeMap, BTreeSet},
  num::NonZeroU32,
};

use orchester_core::{
  connection::AgentProvider,
  id::{ConnectionId, ResourceId, VersionId},
  name::Name,
  permission::{AgentPermissions, StepPermissions, WritableFolders},
  resource::{
    AgentProfile, AgentTargets, CommandAction, CommandArg, CommandProgram, InjectionEvent, Instruction,
    InstructionScope, ModelId, Pipeline, PipelineSources, PromptText, ProviderSettings, RepositoryAccess, Skill,
    SkillFile, SkillFilePath, Slug, Step, StepKey, StepKind, TeamAgentProfile, Tool, Version, VersionNumber,
  },
  run::{
    PublishedVersions, RunSnapshot,
    bundle::{
      BundleError, CanonicalExecutionInputs, ExecutionProfile, MAX_BUNDLE_FILE_BYTES, PinnedBodies, ResourceLabel,
      file_sha256,
    },
  },
  source::SourcePath,
  time::Timestamp,
};
use serde_json::{Value, json};

fn version<B>(number: u32, body: B) -> Version<B> {
  let number = VersionNumber::new(NonZeroU32::new(number).expect("positive version"));
  Version::new(VersionId::generate(), number, body, Timestamp::from_unix_millis(1))
}

fn step(key: &str, kind: StepKind, depends_on: &[&str]) -> Step {
  Step {
    key: StepKey::new(key).expect("valid key"),
    name: Name::new(key).expect("valid name"),
    kind,
    instructions: PromptText::new("").expect("valid prompt"),
    depends_on: depends_on
      .iter()
      .map(|key| StepKey::new(key).expect("valid key"))
      .collect::<BTreeSet<_>>(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  }
}

fn command_tool(program: &str) -> Tool {
  let program = CommandProgram::new(program).expect("valid program");
  Tool::command(
    "Runs the checks.",
    CommandAction::new(program, vec![CommandArg::new("test").expect("valid")]),
  )
}

fn instruction(text: &str) -> Instruction {
  let events = BTreeSet::from([InjectionEvent::Start]);
  let text = PromptText::new(text).expect("valid prompt");
  Instruction::new(text, AgentTargets::All, events, InstructionScope::Project).expect("valid instruction")
}

fn skill(files: &[(&str, &[u8])]) -> Skill {
  let files = files
    .iter()
    .map(|(path, bytes)| SkillFile {
      path: SkillFilePath::new(path).expect("valid path"),
      content: file_sha256(bytes),
      size: bytes.len() as u64,
    })
    .collect();
  Skill::new(
    "Runs the checks.".into(),
    "Run them.".into(),
    files,
    InstructionScope::Project,
  )
  .expect("valid skill")
}

/// A pipeline listing a source, of an agent step and a tool step; the profile with a connection and a writable folder,
/// two instructions and two skills that share a file.
struct Config {
  pipeline: ResourceId,
  pipeline_version: Version<Pipeline>,
  profiles: BTreeMap<ResourceId, Version<AgentProfile>>,
  tools: BTreeMap<ResourceId, Version<Tool>>,
  instructions: BTreeMap<ResourceId, Version<Instruction>>,
  skills: BTreeMap<ResourceId, Version<Skill>>,
}

impl Config {
  fn new(program: &str) -> Self {
    let (builder, tool) = (ResourceId::generate(), ResourceId::generate());
    let agent = StepKind::Agent {
      profile: Some(builder),
      repository: RepositoryAccess::Write,
      model: None,
      permissions: StepPermissions::default(),
    };
    let steps = vec![
      step("implement", agent, &[]),
      step("check", StepKind::Tool { tool: Some(tool) }, &["implement"]),
    ];
    let source = SourcePath::new(env!("CARGO_MANIFEST_DIR")).expect("an absolute path");
    let pipeline = Pipeline::new(steps)
      .expect("valid pipeline")
      .with_sources(PipelineSources::new(vec![source], BTreeMap::new()))
      .expect("pipeline sources");
    let folders: WritableFolders = serde_json::from_value(json!(["/opt/cache"])).expect("folders");
    let mut permissions = AgentPermissions::before_permissions().with_writable_folders(folders);
    permissions.set_repository(RepositoryAccess::Write);
    let profile = AgentProfile::new(
      Name::new("Builder").expect("valid role"),
      "Builds.".into(),
      PromptText::new("Work.").expect("valid prompt"),
      ConnectionId::generate(),
      ModelId::new("model-b").expect("valid model"),
      ProviderSettings::default(),
    )
    .with_permissions(permissions);
    Self {
      pipeline: ResourceId::generate(),
      pipeline_version: version(3, pipeline),
      profiles: BTreeMap::from([(builder, version(2, profile))]),
      tools: BTreeMap::from([(tool, version(1, command_tool(program)))]),
      instructions: (0..2)
        .map(|n| (ResourceId::generate(), version(1, instruction(&format!("Rule {n}.")))))
        .collect(),
      skills: BTreeMap::from([
        (
          ResourceId::generate(),
          version(1, skill(&[("references/a.md", b"a"), ("shared.md", b"shared")])),
        ),
        (ResourceId::generate(), version(4, skill(&[("shared.md", b"shared")]))),
      ]),
    }
  }

  fn snapshot(&self) -> RunSnapshot {
    let published = PublishedVersions {
      profiles: &self.profiles,
      tools: &self.tools,
      instructions: &self.instructions,
      skills: &self.skills,
    };
    RunSnapshot::build(self.pipeline, &self.pipeline_version, published).expect("every reference is published")
  }

  /// A label for every version, the pipeline's too: `kind-n` and `Kind n`.
  fn labels(&self) -> Vec<(VersionId, ResourceLabel)> {
    let ids = [
      ("pipeline", vec![self.pipeline_version.id()]),
      ("profile", self.profiles.values().map(Version::id).collect()),
      ("tool", self.tools.values().map(Version::id).collect()),
      ("instruction", self.instructions.values().map(Version::id).collect()),
      ("skill", self.skills.values().map(Version::id).collect()),
    ];
    ids
      .into_iter()
      .flat_map(|(kind, ids)| {
        ids
          .into_iter()
          .enumerate()
          .map(move |(n, id)| (id, label(&format!("{kind}-{n}"), &format!("{kind} {n}"))))
      })
      .collect()
  }

  /// The inputs of the config's snapshot, with each list of versions reversed when `reversed`.
  fn inputs(&self, reversed: bool) -> Result<CanonicalExecutionInputs, BundleError> {
    let profiles = execution_profiles(&self.profiles, reversed);
    let tools = listed(&self.tools, reversed);
    let instructions = listed(&self.instructions, reversed);
    let skills = listed(&self.skills, reversed);
    let labels = self.labels();
    let bodies = PinnedBodies {
      profiles: &profiles,
      instructions: &instructions,
      tools: &tools,
      skills: &skills,
      labels: &labels,
    };
    CanonicalExecutionInputs::pin(&self.snapshot(), bodies)
  }
}

fn label(slug: &str, name: &str) -> ResourceLabel {
  ResourceLabel::new(
    Slug::new(slug).expect("valid slug"),
    Name::new(name).expect("valid name"),
  )
}

fn listed<B: Clone>(versions: &BTreeMap<ResourceId, Version<B>>, reversed: bool) -> Vec<Version<B>> {
  let mut listed: Vec<Version<B>> = versions.values().cloned().collect();
  if reversed {
    listed.reverse();
  }
  listed
}

fn execution_profiles(
  versions: &BTreeMap<ResourceId, Version<AgentProfile>>,
  reversed: bool,
) -> Vec<(VersionId, ExecutionProfile)> {
  listed(versions, reversed)
    .iter()
    .map(|version| (version.id(), ExecutionProfile::of(version.body())))
    .collect()
}

fn inputs() -> CanonicalExecutionInputs {
  Config::new("cargo").inputs(false).expect("the inputs pin")
}

fn read(value: Value) -> Result<CanonicalExecutionInputs, serde_json::Error> {
  serde_json::from_value(value)
}

#[test]
fn execution_bundle_inputs_hash_the_same_when_they_are_the_same() {
  let config = Config::new("cargo");
  let first = config.inputs(false).expect("the inputs pin");
  let again = config.inputs(false).expect("the inputs pin");
  let hash = first.sha256().expect("hashes");
  assert_eq!(again.sha256().expect("hashes"), hash);
  let read_back = read(serde_json::to_value(&first).expect("serializes")).expect("reads");
  assert_eq!(read_back, first);
  assert_eq!(read_back.sha256().expect("hashes"), hash);
  assert_eq!(hash.as_str().len(), 64);
}

/// The team server pins a team profile as `ExecutionProfile::of_team`, a desktop a local one as `of`; the same profile
/// gives the same bundle bytes either way.
#[test]
fn execution_bundle_inputs_pin_a_team_profile_as_the_local_one_it_came_from() {
  let config = Config::new("cargo");
  let from_local = config.inputs(false).expect("the inputs pin");
  let team: Vec<_> = config
    .profiles
    .values()
    .map(|version| {
      let local = version.body();
      let team = TeamAgentProfile::new(
        local.role().clone(),
        local.responsibilities().to_owned(),
        local.instructions().clone(),
        (AgentProvider::CodexCli, local.model().clone(), local.settings().clone()),
      )
      .with_permissions(
        local
          .permissions()
          .clone()
          .with_writable_folders(WritableFolders::none()),
      );
      (version.id(), ExecutionProfile::of_team(&team))
    })
    .collect();
  let bodies = PinnedBodies {
    profiles: &team,
    instructions: &listed(&config.instructions, false),
    tools: &listed(&config.tools, false),
    skills: &listed(&config.skills, false),
    labels: &config.labels(),
  };

  let from_team = CanonicalExecutionInputs::pin(&config.snapshot(), bodies).expect("the inputs pin");

  assert_eq!(from_team, from_local);
  assert_eq!(
    from_team.sha256().expect("hashes"),
    from_local.sha256().expect("hashes")
  );
}

#[test]
fn execution_bundle_inputs_hash_the_same_whatever_order_their_versions_come_in() {
  let config = Config::new("cargo");
  let sorted = config.inputs(false).expect("the inputs pin");
  let reversed = config.inputs(true).expect("the inputs pin");
  assert_eq!(reversed.sha256().expect("hashes"), sorted.sha256().expect("hashes"));

  let mut value = serde_json::to_value(&sorted).expect("serializes");
  for list in ["instructions", "skills"] {
    value[list].as_array_mut().expect("a list").reverse();
  }
  let read_back = read(value).expect("reads in any order");
  assert_eq!(read_back, sorted);
  assert_eq!(read_back.sha256().expect("hashes"), sorted.sha256().expect("hashes"));
}

#[test]
fn execution_bundle_inputs_hash_differently_when_any_field_changes() {
  let value = serde_json::to_value(inputs()).expect("serializes");
  let hash = inputs_hash(&value);
  let changes: [(&str, Value); 16] = [
    ("/pipeline/body/steps/0/name", json!("Implement it")),
    ("/pipeline/number", json!(9)),
    ("/profiles/0/body/model", json!("model-c")),
    ("/instructions/0/body/text", json!("Another rule.")),
    ("/tools/0/body/action/command/timeout_secs", json!(60)),
    ("/skills/0/body/description", json!("Another skill.")),
    ("/pipeline/label/slug", json!("feature-delivery")),
    ("/pipeline/label/name", json!("Feature delivery")),
    ("/profiles/0/label/slug", json!("reviewer")),
    ("/profiles/0/label/name", json!("Reviewer")),
    ("/instructions/0/label/slug", json!("house-rules")),
    ("/instructions/0/label/name", json!("House rules")),
    ("/tools/0/label/slug", json!("cargo-check")),
    ("/tools/0/label/name", json!("Cargo check")),
    ("/skills/0/label/slug", json!("rust-code-quality")),
    ("/skills/0/label/name", json!("Rust code quality")),
  ];
  for (pointer, changed) in changes {
    let mut value = value.clone();
    *value.pointer_mut(pointer).unwrap_or_else(|| panic!("{pointer} exists")) = changed;
    assert_ne!(inputs_hash(&value), hash, "{pointer}");
  }
}

fn inputs_hash(value: &Value) -> String {
  read(value.clone())
    .expect("reads")
    .sha256()
    .expect("hashes")
    .as_str()
    .to_owned()
}

#[test]
fn execution_bundle_inputs_label_each_pinned_version_as_it_was() {
  let config = Config::new("cargo");
  let inputs = config.inputs(false).expect("the inputs pin");
  let pipeline = inputs.pipeline().label();
  assert_eq!(
    (pipeline.slug().as_str(), pipeline.name().as_str()),
    ("pipeline-0", "pipeline 0")
  );
  let profile = inputs.profiles()[0].label();
  assert_eq!(
    (profile.slug().as_str(), profile.name().as_str()),
    ("profile-0", "profile 0")
  );
  let value = serde_json::to_value(&inputs).expect("serializes");
  assert_eq!(value["tools"][0]["label"], json!({"slug": "tool-0", "name": "tool 0"}));
  let bytes = String::from_utf8(serde_json::to_vec(&inputs).expect("serializes")).expect("UTF-8");
  assert!(
    bytes.contains("\"number\":3,\"label\":{\"slug\":\"pipeline-0\",\"name\":\"pipeline 0\"},\"body\":"),
    "{bytes}"
  );

  let unlabelled = config.labels()[1..].to_vec();
  let bodies = PinnedBodies {
    profiles: &execution_profiles(&config.profiles, false),
    instructions: &listed(&config.instructions, false),
    tools: &listed(&config.tools, false),
    skills: &listed(&config.skills, false),
    labels: &unlabelled,
  };
  let missing = CanonicalExecutionInputs::pin(&config.snapshot(), bodies);
  assert!(matches!(missing, Err(BundleError::MissingLabel { .. })));
}

#[test]
fn execution_bundle_inputs_refuse_a_malformed_label() {
  let value = serde_json::to_value(inputs()).expect("serializes");
  let changes: [(&str, Value, &str); 9] = [
    ("/pipeline/label/slug", json!(""), "slug must be"),
    ("/profiles/0/label/slug", json!("Builder"), "slug must be"),
    ("/tools/0/label/slug", json!("-check"), "slug must be"),
    ("/instructions/0/label/slug", json!("house--rules"), "slug must be"),
    ("/skills/0/label/slug", json!("a".repeat(65)), "slug must be"),
    ("/skills/0/label/name", json!(""), "must not be blank"),
    ("/tools/0/label/name", json!("   "), "must not be blank"),
    ("/profiles/0/label/name", json!("n".repeat(201)), "the limit is"),
    ("/pipeline/label/slug", json!(null), "invalid type"),
  ];
  for (pointer, changed, error) in changes {
    let mut value = value.clone();
    *value.pointer_mut(pointer).unwrap_or_else(|| panic!("{pointer} exists")) = changed;
    let refused = read(value).expect_err("refused").to_string();
    assert!(refused.contains(error), "{pointer}: {refused}");
  }
  for (pointer, field) in [("/pipeline/label", "slug"), ("/skills/0/label", "name")] {
    let mut value = value.clone();
    let label = value
      .pointer_mut(pointer)
      .and_then(Value::as_object_mut)
      .expect("a label");
    label.remove(field);
    let refused = read(value).expect_err("refused").to_string();
    assert!(refused.contains("missing field"), "{pointer}: {refused}");
  }
  let mut unknown = value;
  unknown["tools"][0]["label"]["key"] = json!("CHK");
  let refused = read(unknown).expect_err("refused").to_string();
  assert!(refused.contains("unknown field"), "{refused}");
}

#[test]
fn execution_bundle_inputs_carry_no_path_connection_or_mcp_server() {
  let value = serde_json::to_value(inputs()).expect("serializes");
  let profile = value["profiles"][0]["body"].as_object().expect("a profile body");
  let fields: BTreeSet<&str> = profile.keys().map(String::as_str).collect();
  assert_eq!(
    fields,
    BTreeSet::from([
      "role",
      "responsibilities",
      "instructions",
      "model",
      "settings",
      "permissions"
    ])
  );
  let text = value.to_string();
  for absent in [
    "connection",
    "mcp_servers",
    "writable_folders",
    "sources",
    "/opt/cache",
    env!("CARGO_MANIFEST_DIR"),
  ] {
    let quoted = serde_json::to_string(absent).expect("a string");
    assert!(!text.contains(&quoted[1..quoted.len() - 1]), "{absent}");
  }
  let fields: BTreeSet<&str> = value
    .as_object()
    .expect("an object")
    .keys()
    .map(String::as_str)
    .collect();
  assert_eq!(
    fields,
    BTreeSet::from(["pipeline", "steps", "profiles", "instructions", "tools", "skills"])
  );
}

#[test]
fn execution_bundle_inputs_hold_the_run_snapshot_without_sources() {
  let config = Config::new("cargo");
  let inputs = config.inputs(false).expect("the inputs pin");
  let snapshot = inputs.snapshot();
  let built = config.snapshot();
  assert_eq!(snapshot.pipeline(), built.pipeline());
  assert_eq!(snapshot.steps(), built.steps());
  assert_eq!(snapshot.instructions(), built.instructions());
  assert_eq!(snapshot.tools(), built.tools());
  assert_eq!(snapshot.skills(), built.skills());
  assert!(snapshot.sources().is_empty());
  assert!(snapshot.body().sources().is_empty());
  assert_eq!(snapshot.body().steps(), built.body().steps());
}

#[test]
fn execution_bundle_inputs_list_each_file_once_and_each_agent_steps_permissions() {
  let inputs = inputs();
  let files = inputs.files();
  assert_eq!(files.len(), 2);
  assert!(files.windows(2).all(|pair| pair[0] < pair[1]));
  assert!(files.iter().any(|file| file.matches(b"shared")));
  assert!(!files.iter().any(|file| file.matches(b"other")));
  let skill_files = inputs.skill_files();
  assert_eq!(skill_files.iter().map(|skill| skill.files.len()).sum::<usize>(), 3);
  let permissions = inputs.step_permissions();
  assert_eq!(permissions.len(), 1);
  assert_eq!(permissions[0].step.as_str(), "implement");
  assert!(permissions[0].permissions.writable_folders().is_empty());
}

#[test]
fn execution_bundle_inputs_refuse_what_binds_them_to_one_computer() {
  let absolute = Config::new("/usr/local/bin/check").inputs(false);
  assert!(matches!(absolute, Err(BundleError::MachinePath(_))));
  let drive = Config::new("C:\\tools\\check.exe").inputs(false);
  assert!(matches!(drive, Err(BundleError::MachinePath(_))));

  let value = serde_json::to_value(inputs()).expect("serializes");
  let mut sources = value.clone();
  sources["pipeline"]["body"]["sources"] = json!({"pipeline": [env!("CARGO_MANIFEST_DIR")]});
  assert!(read(sources).expect_err("refused").to_string().contains("sources"));
  let mut folders = value.clone();
  folders["profiles"][0]["body"]["permissions"]["writable_folders"] = json!(["/opt/cache"]);
  assert!(
    read(folders)
      .expect_err("refused")
      .to_string()
      .contains("writable folders")
  );
  let mut connection = value;
  connection["profiles"][0]["body"]["connection"] = json!(ConnectionId::generate());
  assert!(
    read(connection)
      .expect_err("refused")
      .to_string()
      .contains("unknown field")
  );
}

#[test]
fn execution_bundle_inputs_refuse_unknown_fields_and_inconsistent_pins() {
  let value = serde_json::to_value(inputs()).expect("serializes");
  let mut unknown = value.clone();
  unknown["latest"] = json!(true);
  assert!(
    read(unknown)
      .expect_err("refused")
      .to_string()
      .contains("unknown field")
  );
  let mut pinned = value.clone();
  pinned["instructions"][0]["latest"] = json!(true);
  assert!(read(pinned).expect_err("refused").to_string().contains("unknown field"));
  let mut twice = value.clone();
  let first = twice["instructions"][0].clone();
  twice["instructions"].as_array_mut().expect("a list").push(first);
  assert!(read(twice).expect_err("refused").to_string().contains("more than once"));
  let mut no_profile = value.clone();
  no_profile["profiles"] = json!([]);
  assert!(
    read(no_profile)
      .expect_err("refused")
      .to_string()
      .contains("agent profiles")
  );
  let mut no_tool = value;
  no_tool["tools"] = json!([]);
  assert!(read(no_tool).expect_err("refused").to_string().contains("calls a tool"));
}

#[test]
fn execution_bundle_inputs_need_every_pinned_body_and_bounded_files() {
  let config = Config::new("cargo");
  let bodies = PinnedBodies {
    profiles: &execution_profiles(&config.profiles, false),
    instructions: &[],
    tools: &config.tools.values().cloned().collect::<Vec<_>>(),
    skills: &config.skills.values().cloned().collect::<Vec<_>>(),
    labels: &config.labels(),
  };
  let missing = CanonicalExecutionInputs::pin(&config.snapshot(), bodies);
  assert!(matches!(missing, Err(BundleError::MissingVersion { .. })));

  let mut value = serde_json::to_value(inputs()).expect("serializes");
  value["skills"][0]["body"]["files"][0]["size"] = json!(MAX_BUNDLE_FILE_BYTES + 1);
  assert!(
    read(value)
      .expect_err("refused")
      .to_string()
      .contains("more than a bundle file may have")
  );
}
