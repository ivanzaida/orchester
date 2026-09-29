//! What Import from repository says about each thing it found: what it becomes, or why nothing
//! (`PvgEz` in `cuocA` rows of `wulRC`, the steps of the found card in `YRaLK`), and what it made
//! (`Z8mGB0`).

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::agent_files::import::{Candidate, ImportItem, ItemKind, Unimportable};

use crate::{
  messages::Message,
  ui::{icon::Icon, message_text},
};

/// "1 file", "3 files".
pub fn file_count(ctx: &Ctx, files: usize) -> Arc<str> {
  let key = if files == 1 {
    "skills.files.one"
  } else {
    "skills.files.other"
  };
  ctx.t_args(key, [("count", files.to_string().as_str())])
}

/// What an item becomes, in the dialog's list: "Draft skill run-checks · SKILL.md and 2 files".
pub fn becomes(ctx: &Ctx, item: &ImportItem) -> Arc<str> {
  match &item.outcome {
    Ok(Candidate::Instruction { name, text }) => {
      let lines = text.as_str().lines().count();
      let key = if lines == 1 {
        "skills.import.becomesInstruction.one"
      } else {
        "skills.import.becomesInstruction.other"
      };
      ctx.t_args(key, [("name", name.as_str()), ("lines", lines.to_string().as_str())])
    }
    Ok(Candidate::Skill { name, files, .. }) if files.is_empty() => {
      ctx.t_args("skills.import.becomesSkill", [("name", name.as_str())])
    }
    Ok(Candidate::Skill { name, files, .. }) => ctx.t_args(
      "skills.import.becomesSkillWithFiles",
      [("name", name.as_str()), ("files", &*file_count(ctx, files.len()))],
    ),
    Err(reason) => unimportable(ctx, reason),
  }
}

/// What an item becomes, as the found card of an empty library says it (`IaMc9` in `mzYus`):
/// "becomes draft skill run-checks · 2 files".
pub fn card_line(ctx: &Ctx, item: &ImportItem) -> Arc<str> {
  match &item.outcome {
    Ok(Candidate::Instruction { .. }) => ctx.t("skills.empty.becomesInstruction"),
    Ok(Candidate::Skill { name, files, .. }) if files.is_empty() => {
      ctx.t_args("skills.empty.becomesSkill", [("name", name.as_str())])
    }
    Ok(Candidate::Skill { name, files, .. }) => ctx.t_args(
      "skills.empty.becomesSkillWithFiles",
      [("name", name.as_str()), ("files", &*file_count(ctx, files.len()))],
    ),
    Err(reason) => unimportable(ctx, reason),
  }
}

/// Why an item cannot be imported, as its list line says it.
pub fn unimportable(ctx: &Ctx, reason: &Unimportable) -> Arc<str> {
  let key = match reason {
    Unimportable::OnlyImports { .. } => "skills.import.reason.onlyImports",
    Unimportable::Empty => "skills.import.reason.empty",
    Unimportable::NotText => "skills.import.reason.notText",
    Unimportable::NoSkillFile => "skills.import.reason.noSkillFile",
    Unimportable::UnsupportedFiles { .. } => "skills.import.reason.unsupportedFiles",
  };
  message_text(ctx, &with_reason(Message::new(key), reason))
}

/// Why an item was not imported, as a sentence of the result (`IdcYF` in `Z8mGB0`).
pub fn not_imported(ctx: &Ctx, item: &ImportItem) -> Arc<str> {
  let message = match &item.outcome {
    Ok(_) => Message::new("skills.import.result.notSelected"),
    Err(reason) => {
      let key = match reason {
        Unimportable::OnlyImports { .. } => "skills.import.result.onlyImports",
        Unimportable::Empty => "skills.import.result.empty",
        Unimportable::NotText => "skills.import.result.notText",
        Unimportable::NoSkillFile => "skills.import.result.noSkillFile",
        Unimportable::UnsupportedFiles { .. } => "skills.import.result.unsupportedFiles",
      };
      with_reason(Message::new(key), reason)
    }
  };
  message_text(ctx, &message.with("source", item.source.clone()))
}

/// `message` with what `reason` names: the imports a file only makes, or the files a skill cannot
/// hold.
fn with_reason(message: Message, reason: &Unimportable) -> Message {
  match reason {
    Unimportable::OnlyImports { targets } => message.with("targets", targets.join(", ")).with(
      "imports",
      targets
        .iter()
        .map(|target| format!("@{target}"))
        .collect::<Vec<_>>()
        .join(" "),
    ),
    Unimportable::UnsupportedFiles { paths } => message.with("paths", paths.join(", ")),
    Unimportable::Empty | Unimportable::NotText | Unimportable::NoSkillFile => message,
  }
}

/// The kind label and icon of an item (`LBzRw` in `cuocA`).
pub fn kind(ctx: &Ctx, kind: ItemKind) -> (Icon, Arc<str>) {
  match kind {
    ItemKind::Instruction => (Icon::BookOpen, ctx.t("skills.import.kind.instruction")),
    ItemKind::Skill => (Icon::Sparkles, ctx.t("skills.import.kind.skill")),
  }
}
