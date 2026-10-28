//! What an item found in the repository becomes, or why it cannot be imported, in the same words whether Instructions
//! or Skills opened the import (02c `P21bwh`, 03c `cC55E`).
use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::agent_files::import::{Candidate, ImportItem, Unimportable};

/// `Draft project instruction “AGENTS” · 84 lines`, or the reason the item cannot be imported.
pub(crate) fn item_detail(ctx: &Ctx, item: &ImportItem) -> Arc<str> {
  match &item.outcome {
    Err(Unimportable::OnlyImports { targets }) => ctx.t_args(
      "instructions.importOnlyPoints",
      [
        ("targets", targets.join(", ")),
        (
          "references",
          targets
            .iter()
            .map(|target| format!("@{target}"))
            .collect::<Vec<_>>()
            .join(", "),
        ),
      ],
    ),
    Ok(Candidate::Instruction { name, text }) => {
      let lines = text.as_str().lines().count();
      ctx.t_args(
        if lines == 1 {
          "instructions.importInstructionOne"
        } else {
          "instructions.importInstruction"
        },
        [("name", name.as_str().to_owned()), ("lines", lines.to_string())],
      )
    }
    Ok(Candidate::Skill { name, files, .. }) => ctx.t_args(
      match files.len() {
        0 => "instructions.importSkillNone",
        1 => "instructions.importSkillOne",
        _ => "instructions.importSkill",
      },
      [("name", name.as_str().to_owned()), ("files", files.len().to_string())],
    ),
    Err(Unimportable::Empty) => ctx.t("instructions.importBlank"),
    Err(Unimportable::NotText) => ctx.t("instructions.importNotText"),
    Err(Unimportable::NoSkillFile) => ctx.t("instructions.importNoSkill"),
    Err(Unimportable::UnsupportedFiles { .. }) => ctx.t("instructions.importUnsupported"),
  }
}
