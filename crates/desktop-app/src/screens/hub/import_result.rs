//! What an import from the repository created (`03c` `cC55E`), in place of the import page whether Instructions or
//! Skills opened it: the crumb back, a page head with its tile, how many drafts were created and of how many items,
//! the lead line, a group of the drafts each with Open draft, a group of what was not imported with why, the notice
//! on Repository agent files, then Review skills and Open Instructions.
use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  agent_files::{
    RepositoryAgentFiles,
    import::{ImportItem, Unimportable},
  },
  id::ProjectId,
  resource::ResourceKind,
};
use orchester_workspace::skills::repository::CreatedDraft;

use super::{
  page_head,
  parts::{self, boxed, group, head, note},
};
use crate::{
  navigation::{Navigation, link::Open},
  routes::{InstructionsTab, InstructionsView, ProjectPage, Route, SkillsFilter},
  screens::skills::words,
  theme::{ExtraSpacing, ExtraTypography, HubSpacing, HubText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    notice::{Notice, NoticeTone},
  },
};

/// What the result page shows and where its crumb leads.
pub(crate) struct Outcome<'a> {
  /// The page's element id; its parts are named after `id` (`<id>-created-<n>`, `<id>-open-<n>`, …).
  pub root: &'a str,
  pub id: &'a str,
  pub project: ProjectId,
  /// The tab's list the crumb leads back to, by its name.
  pub back: (&'a str, Open),
  pub created: &'a [CreatedDraft],
  /// Every item the scan found, those not imported among them.
  pub items: &'a [ImportItem],
  /// How many items the import was chosen from.
  pub total: usize,
  /// The project's Repository agent files, when it could be read.
  pub agent_files: Option<RepositoryAgentFiles>,
}

impl Outcome<'_> {
  /// The page's pane, to sit in the hub's body without a side.
  pub(crate) fn page(&self, ctx: &mut Ctx) -> Element {
    let id = self.id;
    let instructions = self.of(ResourceKind::Instruction);
    let types = words::types(ctx, instructions, self.created.len() - instructions);
    let mut page = Column::new()
      .id(self.root.to_owned())
      .width(Dimension::Pct(100.0))
      .spacing(HubSpacing::Sections.role())
      .child(self.top(ctx, &types))
      .child(
        Text::new(&ctx.t("skills.importResultDetail"))
          .id(format!("{id}-lead"))
          .variant(HubText::Value.role())
          .color(PaletteColor::TextSecondary)
          .width(Dimension::Pct(100.0)),
      )
      .child(self.created_group(ctx, &types));
    if let Some(omitted) = self.omitted(ctx) {
      page = page.child(omitted);
    }
    if let Some(files) = self.agent_files {
      page = page.child(
        Notice::new(
          format!("{id}-agent-files"),
          NoticeTone::Info,
          ctx.t(if files.is_on() {
            "skills.repositoryOn"
          } else {
            "skills.repositoryOff"
          }),
        )
        .icon(Icon::Info)
        .body(ctx.t("skills.resultRepository")),
      );
    }
    page.child(self.actions(ctx)).into()
  }

  fn of(&self, kind: ResourceKind) -> usize {
    self
      .created
      .iter()
      .filter(|draft| draft.resource.kind() == kind)
      .count()
  }

  /// The crumb, then the tile, *N drafts created* and *3 of 5 items imported · 1 instruction, 2 skills*.
  fn top(&self, ctx: &Ctx, types: &str) -> Column {
    let id = self.id;
    let (list, back) = &self.back;
    let title = ctx.t("skills.import");
    let count = self.created.len();
    let done = words::drafts_created(ctx, count);
    let sub = ctx.t_args(
      "skills.importedTotals",
      [
        ("imported", count.to_string()),
        ("total", self.total.to_string()),
        ("types", types.to_owned()),
      ],
    );
    let head = Row::new()
      .id(format!("{id}-result-head"))
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Center)
      .child(parts::tile_in(Icon::CircleCheck, true, PaletteColor::Success))
      .child(
        Column::new()
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .spacing(ExtraSpacing::Fine.role())
          .child(
            Text::new(&done)
              .id(format!("{id}-result-title"))
              .variant(HubText::PageTitle.role()),
          )
          .child(
            Text::new(&sub)
              .id(format!("{id}-result-sub"))
              .variant(HubText::Hint.role()),
          ),
      );
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(HubSpacing::Row.role())
      .child(page_head::crumb(
        ctx,
        &format!("{id}-result-crumb"),
        (list, back.clone()),
        Some(&title),
      ))
      .child(head)
  }

  /// *Created · 1 instruction, 2 skills*: a row per draft with what it is and where from, and Open draft; then the
  /// note on the skill drafts' scope.
  fn created_group(&self, ctx: &mut Ctx, types: &str) -> Column {
    let id = self.id;
    let navigation = Navigation::of(ctx);
    let rows: Vec<Element> = self
      .created
      .iter()
      .enumerate()
      .map(|(index, draft)| {
        let page = match draft.resource.kind() {
          ResourceKind::Skill => ProjectPage::Skill(draft.resource.id()),
          _ => ProjectPage::Instructions(InstructionsView::rule(
            InstructionsTab::Project,
            Some(draft.resource.id()),
          )),
        };
        let open = ActionButton::new(
          format!("{id}-open-{index}"),
          ButtonKind::Ghost,
          ctx.t("skills.openDraft"),
        )
        .icon(Icon::ArrowRight)
        .opens(navigation.to(Route::Project(self.project, page)));
        let hint = ctx.t_args(
          "skills.resultFromDraft",
          [
            ("draft", draft_label(ctx, draft).to_string()),
            ("source", draft.source.clone()),
          ],
        );
        row(
          &format!("{id}-created-{index}"),
          draft.resource.name().as_str(),
          (&hint, PaletteColor::TextMuted),
          Some(open.into()),
        )
      })
      .collect();
    let title = ctx.t_args("skills.created", [("types", types)]);
    let mut created = group(
      &format!("{id}-created"),
      head(&format!("{id}-created"), &title, None, None),
      boxed(&format!("{id}-created-rows"), rows),
    );
    if self.of(ResourceKind::Skill) > 0 {
      created = created.child(note(
        &format!("{id}-result-note"),
        Icon::Info,
        &ctx.t("skills.resultNote"),
      ));
    }
    created
  }

  /// *Not imported*: each item found that made no draft, with why; none when every item did.
  fn omitted(&self, ctx: &Ctx) -> Option<Column> {
    let id = self.id;
    let rows: Vec<Element> = self
      .items
      .iter()
      .filter(|item| !self.created.iter().any(|draft| draft.source == item.source))
      .enumerate()
      .map(|(index, item)| {
        let (why, color) = match &item.outcome {
          Err(why) => (unimportable(ctx, why), PaletteColor::Warning),
          Ok(_) => (ctx.t("skills.notSelected"), PaletteColor::TextMuted),
        };
        row(&format!("{id}-omitted-{index}"), &item.source, (&why, color), None)
      })
      .collect();
    (!rows.is_empty()).then(|| {
      group(
        &format!("{id}-omitted"),
        head(&format!("{id}-omitted"), &ctx.t("skills.notImported"), None, None),
        boxed(&format!("{id}-omitted-rows"), rows),
      )
    })
  }

  /// Review skills, then Open Instructions.
  fn actions(&self, ctx: &mut Ctx) -> Row {
    let id = self.id;
    let navigation = Navigation::of(ctx);
    let skills = Route::Project(self.project, ProjectPage::Skills(SkillsFilter::default()));
    let instructions = Route::Project(self.project, ProjectPage::Instructions(InstructionsView::default()));
    Row::new()
      .id(format!("{id}-result-actions"))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(
        ActionButton::new(
          format!("{id}-review-skills"),
          ButtonKind::Primary,
          ctx.t("skills.reviewSkills"),
        )
        .icon(Icon::GraduationCap)
        .opens(navigation.to(skills)),
      )
      .child(
        ActionButton::new(
          format!("{id}-open-instructions"),
          ButtonKind::Ghost,
          ctx.t("skills.openInstructions"),
        )
        .icon(Icon::BookOpen)
        .opens(navigation.to(instructions)),
      )
  }
}

/// A row of the result: `label`, and under it `hint` in its color, with `end` at the row's end (`AGENTS` `eZ97L`).
fn row(id: &str, label: &str, (hint, color): (&str, PaletteColor), end: Option<Element>) -> Element {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(HubSpacing::Row.role(), HubSpacing::FieldY.role()))
    .spacing(HubSpacing::FieldGap.role())
    .align_items(Alignment::Center)
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(
          Text::new(label)
            .id(format!("{id}-label"))
            .variant(ExtraTypography::Strong.role())
            .width(Dimension::Pct(100.0)),
        )
        .child(
          Text::new(hint)
            .id(format!("{id}-hint"))
            .variant(HubText::Hint.role())
            .color(color)
            .width(Dimension::Pct(100.0)),
        ),
    )
    .with_children(end)
    .into()
}

/// *Draft project instruction v1*, *Draft skill v1 · 2 files*.
fn draft_label(ctx: &Ctx, draft: &CreatedDraft) -> Arc<str> {
  let key = match (draft.resource.kind(), draft.files) {
    (ResourceKind::Skill, 0) => "skills.resultSkill",
    (ResourceKind::Skill, _) => "skills.resultSkillFiles",
    _ => "skills.resultInstruction",
  };
  ctx.t_args(
    key,
    [
      ("version", draft.version.get().to_string()),
      ("files", words::files(ctx, draft.files).to_string()),
    ],
  )
}

/// Why an item made no draft, the same whether Instructions or Skills imported it (*No SKILL.md was found.*, `IRCx0`).
fn unimportable(ctx: &Ctx, why: &Unimportable) -> Arc<str> {
  match why {
    Unimportable::OnlyImports { targets } => ctx.t_args("skills.onlyImports", [("targets", targets.join(", "))]),
    Unimportable::Empty => ctx.t("skills.emptyFile"),
    Unimportable::NotText => ctx.t("skills.notText"),
    Unimportable::NoSkillFile => ctx.t("skills.noSkillFile"),
    Unimportable::UnsupportedFiles { paths } => ctx.t_args("skills.unsupportedFiles", [("paths", paths.join(", "))]),
  }
}
