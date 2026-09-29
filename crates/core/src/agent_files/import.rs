//! Import from repository (ORC-15 `wulRC`, `Z8mGB0`): what a repository's own agent files become
//! in Orchester. Each `AGENTS.md` / `CLAUDE.md` becomes a draft project instruction, each skill
//! folder of `.claude/skills` and `.agents/skills` a draft skill of the project's scope, with its
//! front matter's name and description, its body and its other files as reference files. Only
//! drafts are made: nothing is published, no agent gets them until a person publishes them, and
//! the repository is only read. Later changes in the repository do not reach them.
//!
//! What the desktop found is passed in ([`Found`]); what it may become is decided here
//! ([`candidates`]), and a chosen candidate is named and turned into a draft ([`Candidate::draft`]).

use std::{collections::BTreeSet, sync::Arc};

use thiserror::Error;

use super::frontmatter;
use crate::{
  name::Name,
  resource::{
    AgentTargets, InjectionEvent, Instruction, InstructionScope, PromptText, Skill, SkillError, SkillFile,
    SkillFilePath, Slug,
  },
  time::Timestamp,
};

/// Instruction files at the repository's root, in the order the import lists them.
pub const INSTRUCTION_FILES: [&str; 2] = ["AGENTS.md", "CLAUDE.md"];
/// Skill folders, each holding one folder per skill, in the order the import lists them.
pub const SKILL_FOLDERS: [&str; 2] = [".claude/skills", ".agents/skills"];

/// A file of a skill folder besides its `SKILL.md`, as read from the repository.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoundFile {
  pub path: SkillFilePath,
  pub content: Arc<[u8]>,
}

/// What the desktop found in the repository, as read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Found {
  /// `AGENTS.md` or `CLAUDE.md` at the root.
  Instructions { path: String, content: Vec<u8> },
  /// A folder of a skills folder, e.g. `.claude/skills/run-checks`: its `SKILL.md` (`None` when it
  /// has none), its other files, and those whose path a skill cannot hold (relative to the folder).
  Skill {
    folder: String,
    markdown: Option<Vec<u8>>,
    files: Vec<FoundFile>,
    unsupported: Vec<String>,
  },
}

/// Why something found cannot be imported.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unimportable {
  /// Every line of the file is an `@path` import, e.g. a `CLAUDE.md` that only says `@AGENTS.md`:
  /// it has nothing of its own.
  OnlyImports { targets: Vec<String> },
  /// The file has no text.
  Empty,
  /// The file is not UTF-8 text.
  NotText,
  /// The folder has no `SKILL.md`, so it is no skill.
  NoSkillFile,
  /// Files of the folder have paths a skill cannot hold ([`SkillFilePath`]); importing the rest
  /// would lose them.
  UnsupportedFiles { paths: Vec<String> },
}

/// What something found becomes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Candidate {
  /// A project instruction, injected into every agent when it starts, with the file's text.
  Instruction { name: Name, text: PromptText },
  /// A project skill.
  Skill {
    name: Name,
    description: String,
    body: String,
    files: Vec<FoundFile>,
  },
}

/// Which group of the import list an entry is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ItemKind {
  Instruction,
  Skill,
}

/// One entry of the import list: where it is in the repository and what it becomes, or why nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportItem {
  pub kind: ItemKind,
  /// The file or folder, relative to the repository's root with `/` between names.
  pub source: String,
  /// The instruction file, or the skill's `SKILL.md`, as it reads; `None` when there is none or it
  /// is not text.
  pub text: Option<String>,
  pub outcome: Result<Candidate, Unimportable>,
}

/// What each thing `found` becomes: instruction files first, then the skill folders of
/// `.claude/skills`, then of `.agents/skills`, each by name.
pub fn candidates(found: Vec<Found>) -> Vec<ImportItem> {
  let mut items: Vec<ImportItem> = found
    .into_iter()
    .map(|found| match found {
      Found::Instructions { path, content } => {
        let text = String::from_utf8(content).ok();
        ImportItem {
          kind: ItemKind::Instruction,
          outcome: text
            .as_deref()
            .map_or(Err(Unimportable::NotText), |text| instruction(&path, text)),
          source: path,
          text,
        }
      }
      Found::Skill {
        folder,
        markdown,
        files,
        unsupported,
      } => {
        let markdown_found = markdown.is_some();
        let text = markdown.and_then(|markdown| String::from_utf8(markdown).ok());
        let outcome = match (&text, markdown_found) {
          (Some(text), _) => skill(&folder, text, files, unsupported),
          (None, true) => Err(Unimportable::NotText),
          (None, false) => Err(Unimportable::NoSkillFile),
        };
        ImportItem {
          kind: ItemKind::Skill,
          outcome,
          source: folder,
          text,
        }
      }
    })
    .collect();
  items.sort_by_key(|item| {
    let order = INSTRUCTION_FILES
      .iter()
      .chain(&SKILL_FOLDERS)
      .position(|place| *place == item.source || item.source.starts_with(&format!("{place}/")))
      .unwrap_or(usize::MAX);
    (item.kind, order, item.source.clone())
  });
  items
}

fn instruction(path: &str, text: &str) -> Result<Candidate, Unimportable> {
  let lines: Vec<&str> = text.lines().map(str::trim).filter(|line| !line.is_empty()).collect();
  if lines.is_empty() {
    return Err(Unimportable::Empty);
  }
  let imports: Vec<String> = lines
    .iter()
    .filter_map(|line| {
      line
        .strip_prefix('@')
        .filter(|target| !target.contains(char::is_whitespace))
    })
    .map(str::to_owned)
    .collect();
  if imports.len() == lines.len() {
    return Err(Unimportable::OnlyImports { targets: imports });
  }
  Ok(Candidate::Instruction {
    name: Name::new(path).expect("an instruction file's name is not blank"),
    text: literal(text),
  })
}

/// `text` as prompt text that says exactly what the file says: every `{{` is escaped, so nothing
/// in it reads as a variable.
fn literal(text: &str) -> PromptText {
  PromptText::new(&text.replace("{{", "\\{{")).expect("text whose every `{{` is escaped has no placeholders")
}

fn skill(
  folder: &str,
  markdown: &str,
  files: Vec<FoundFile>,
  unsupported: Vec<String>,
) -> Result<Candidate, Unimportable> {
  if !unsupported.is_empty() {
    return Err(Unimportable::UnsupportedFiles { paths: unsupported });
  }
  let parsed = frontmatter::parse(markdown);
  let folder_name = folder.rsplit('/').next().unwrap_or(folder);
  let name = parsed
    .name
    .as_deref()
    .and_then(|name| Name::new(name).ok())
    .unwrap_or_else(|| Name::new(folder_name).expect("a found folder has a name"));
  Ok(Candidate::Skill {
    name,
    description: parsed.description.unwrap_or_default(),
    body: parsed.body,
    files,
  })
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RepositoryDraftError {
  #[error(transparent)]
  Skill(#[from] SkillError),
  /// A skill's stored reference files are not those it was found with.
  #[error("the skill's reference files were not all stored")]
  FilesMismatch,
}

/// The first draft of an imported resource.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DraftContent {
  Instruction(Instruction),
  Skill(Skill),
}

/// A resource an import creates: its name and slug, its first draft and where it came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepositoryDraft {
  pub name: Name,
  pub slug: Slug,
  pub content: DraftContent,
  /// The file or folder it was made from, relative to the repository's root.
  pub source: String,
}

/// Where an imported resource came from, for display only: it does not follow its source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepositoryOrigin {
  pub source: String,
  pub imported_at: Timestamp,
}

impl Candidate {
  pub const fn name(&self) -> &Name {
    match self {
      Self::Instruction { name, .. } | Self::Skill { name, .. } => name,
    }
  }

  /// This candidate as the draft of a new resource made from `source`, under a slug of its name not
  /// in `taken` (of its kind), which then takes it. A skill's reference files are `stored`, one
  /// per found file in the same order, their content already put in the project's blobs.
  pub fn draft(
    &self,
    source: &str,
    taken: &mut Vec<Slug>,
    stored: Vec<SkillFile>,
  ) -> Result<RepositoryDraft, RepositoryDraftError> {
    let (fallback, content) = match self {
      Self::Instruction { text, .. } => {
        let instruction = Instruction::new(
          text.clone(),
          AgentTargets::All,
          BTreeSet::from([InjectionEvent::Start]),
          InstructionScope::Project,
        )
        .expect("every agent at start is a valid selection for the project");
        ("instruction", DraftContent::Instruction(instruction))
      }
      Self::Skill {
        description,
        body,
        files,
        ..
      } => {
        let same = stored.len() == files.len()
          && stored
            .iter()
            .zip(files)
            .all(|(stored, found)| stored.path == found.path);
        if !same {
          return Err(RepositoryDraftError::FilesMismatch);
        }
        let skill = Skill::new(description.clone(), body.clone(), stored, InstructionScope::Project)?;
        ("skill", DraftContent::Skill(skill))
      }
    };
    let fallback = Slug::new(fallback).expect("the fallback slugs are valid");
    let slug = Slug::unique_for(self.name(), &fallback, taken);
    taken.push(slug.clone());
    Ok(RepositoryDraft {
      name: self.name().clone(),
      slug,
      content,
      source: source.to_owned(),
    })
  }
}
