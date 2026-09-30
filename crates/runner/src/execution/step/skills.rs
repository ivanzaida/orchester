//! Skills and the repository's own agent files at an agent's start (ORC-15,
//! `orchester_core::agent_files`).
//!
//! - Orchester's skills: the published versions the run pinned that reach the agent (by scope) are written to a folder
//!   of the step execution's own, `<repo>/.orchester/skills/<execution-id>/<slug>/` (the `SKILL.md` with its name and
//!   description, and the reference files from the project's blobs), outside every worktree. It is written again at
//!   each start of the execution, so what an agent changed there does not last, and kept afterwards: a resumed session
//!   reads the paths it was told when it started. Both CLIs can read it: Codex's sandbox reads the whole disk, and
//!   Claude Code is given the folder with `--add-dir`.
//! - Orchester's internal `orchester-codebase-index` skill, embedded in the runner, is placed the same way for an agent that gets
//!   the code index (`super::super::codebase_index`), under `_orchester/`, which no project skill's slug can name. It
//!   is not a project resource and no screen lists it.
//! - The repository's own skills: while the project has its agent files off, every `SKILL.md` under
//!   `<worktree>/.agents/skills` (at any depth, through links, as Codex finds them) is listed for Codex to disable. A
//!   folder that cannot be read stops the agent from starting rather than letting Codex load what it holds.

use std::{
  collections::HashSet,
  fs, io,
  path::{Path, PathBuf},
};

use orchester_core::{
  agent_files::frontmatter,
  connection::stream::RunnerProblem,
  id::StepExecutionId,
  resource::{InstructionTarget, SKILL_FILE, Skill, Slug, Step},
  run::{ProfileInfo, SkillListing},
  store::{BlobStore, ResourceStore},
};

use super::super::run::RunTask;

/// The Codex skills folder of a repository.
const REPOSITORY_SKILLS: [&str; 2] = [".agents", "skills"];
/// Where Orchester's internal skills go in an execution's skills folder: `_` starts no slug.
const INTERNAL_SKILLS: &str = "_orchester";
/// The internal skill that explains the code index, as agents get it.
const CODE_INDEX_SKILL: &str = include_str!("../../../assets/codebase-index/SKILL.md");

fn problem(detail: String) -> RunnerProblem {
  RunnerProblem::CouldNotStart { detail }
}

/// A skill placed for an agent: its name, description and `SKILL.md`.
pub(super) struct PlacedSkill {
  pub(super) name: String,
  pub(super) description: String,
  pub(super) path: String,
}

/// The skills placed for one agent, and the folder holding them; `None` when it gets none.
#[derive(Default)]
pub(super) struct PlacedSkills {
  pub(super) folder: Option<PathBuf>,
  pub(super) skills: Vec<PlacedSkill>,
}

impl PlacedSkills {
  /// The skills as the prompt lists them.
  pub(super) fn listings(&self) -> Vec<SkillListing<'_>> {
    self
      .skills
      .iter()
      .map(|skill| SkillListing {
        name: &skill.name,
        description: &skill.description,
        path: &skill.path,
      })
      .collect()
  }
}

impl RunTask {
  /// Places the skills the run pinned that reach the agent of `step` run by `profile`, for
  /// `execution`, replacing what an earlier start placed.
  pub(super) fn place_skills(
    &self,
    execution: StepExecutionId,
    step: &Step,
    profile: ProfileInfo<'_>,
  ) -> Result<PlacedSkills, RunnerProblem> {
    let target = InstructionTarget {
      pipeline: self.run.snapshot().pipeline().resource,
      step: &step.key,
      profile: profile.id,
      role: profile.body.role(),
    };
    let mut reaching = Vec::new();
    for pin in self.run.snapshot().skills() {
      let resource = self
        .db
        .resource(pin.resource)
        .map_err(|error| problem(error.to_string()))?
        .ok_or_else(|| problem(format!("skill {} no longer exists", pin.resource)))?;
      let version = self
        .db
        .version::<Skill>(pin.resource, pin.number)
        .map_err(|error| problem(error.to_string()))?
        .ok_or_else(|| problem(format!("skill version {} is missing", pin.number)))?;
      if version.body().reaches(target).is_ok() {
        reaching.push((resource.slug().clone(), version.body().clone()));
      }
    }
    let folder = self.skills_folder(execution);
    remove_folder(&folder).map_err(|error| problem(format!("cannot clear {}: {error}", folder.display())))?;
    if reaching.is_empty() {
      return Ok(PlacedSkills::default());
    }
    reaching.sort_by(|(left, _), (right, _)| left.cmp(right));
    let mut skills = Vec::with_capacity(reaching.len());
    for (slug, skill) in reaching {
      let path = self
        .write_skill(&folder.join(slug.as_str()), &slug, &skill)
        .map_err(|detail| problem(format!("cannot place skill {}: {detail}", slug.as_str())))?;
      skills.push(PlacedSkill {
        name: slug.as_str().to_owned(),
        description: skill.description().to_owned(),
        path: path.to_string_lossy().into_owned(),
      });
    }
    Ok(PlacedSkills {
      folder: Some(folder),
      skills,
    })
  }

  /// The skills folder of `execution`, outside every worktree.
  fn skills_folder(&self, execution: StepExecutionId) -> PathBuf {
    self.root.join(".orchester").join("skills").join(execution.to_string())
  }

  /// Places the internal `orchester-codebase-index` skill for `execution`, beside what `placed` holds, and lists
  /// it; its `SKILL.md`.
  pub(super) fn place_code_map(
    &self,
    execution: StepExecutionId,
    placed: &mut PlacedSkills,
  ) -> Result<PathBuf, RunnerProblem> {
    let folder = self.skills_folder(execution);
    let skill = folder.join(INTERNAL_SKILLS).join("codebase-index");
    let document = skill.join(SKILL_FILE);
    fs::create_dir_all(&skill)
      .and_then(|()| fs::write(&document, CODE_INDEX_SKILL))
      .map_err(|error| problem(format!("cannot place the orchester-codebase-index skill: {error}")))?;
    let markdown = frontmatter::parse(CODE_INDEX_SKILL);
    placed.skills.push(PlacedSkill {
      name: markdown.name.expect("the embedded orchester-codebase-index skill has a name"),
      description: markdown
        .description
        .expect("the embedded orchester-codebase-index skill has a description"),
      path: document.to_string_lossy().into_owned(),
    });
    placed.folder = Some(folder);
    Ok(document)
  }

  /// Writes `skill`'s `SKILL.md` and reference files into `folder`; the `SKILL.md` written.
  fn write_skill(&self, folder: &Path, slug: &Slug, skill: &Skill) -> Result<PathBuf, String> {
    fs::create_dir_all(folder).map_err(|error| error.to_string())?;
    let document = folder.join(SKILL_FILE);
    fs::write(&document, skill.document(slug)).map_err(|error| error.to_string())?;
    for file in skill.files() {
      let content = self.db.blob(&file.content).map_err(|error| error.to_string())?;
      let path = file
        .path
        .segments()
        .fold(folder.to_path_buf(), |path, name| path.join(name));
      if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
      }
      fs::write(&path, content).map_err(|error| format!("{}: {error}", file.path.as_str()))?;
    }
    Ok(document)
  }
}

fn remove_folder(folder: &Path) -> io::Result<()> {
  match fs::remove_dir_all(folder) {
    Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
    _ => Ok(()),
  }
}

/// Every `SKILL.md` under `<worktree>/.agents/skills`, for Codex to disable; none when the folder
/// does not exist.
pub(super) fn repository_skills(worktree: &Path) -> Result<Vec<PathBuf>, RunnerProblem> {
  let root = REPOSITORY_SKILLS
    .iter()
    .fold(worktree.to_path_buf(), |path, name| path.join(name));
  let mut found = Vec::new();
  let mut visited = HashSet::new();
  collect(&root, &mut found, &mut visited).map_err(|error| {
    problem(format!(
      "cannot read the repository's skills in {}: {error}",
      root.display()
    ))
  })?;
  found.sort();
  Ok(found)
}

/// Collects the `SKILL.md` files under `folder`, following links; a folder reached twice (a link
/// cycle) is read once.
fn collect(folder: &Path, found: &mut Vec<PathBuf>, visited: &mut HashSet<PathBuf>) -> io::Result<()> {
  let entries = match fs::read_dir(folder) {
    Ok(entries) => entries,
    Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
    Err(error) => return Err(error),
  };
  if !visited.insert(fs::canonicalize(folder)?) {
    return Ok(());
  }
  for entry in entries {
    let path = entry?.path();
    let metadata = match fs::metadata(&path) {
      Ok(metadata) => metadata,
      // A link to nothing holds no skill.
      Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
      Err(error) => return Err(error),
    };
    if metadata.is_dir() {
      collect(&path, found, visited)?;
    } else if path.file_name().is_some_and(|name| name == SKILL_FILE) {
      found.push(path);
    }
  }
  Ok(())
}
