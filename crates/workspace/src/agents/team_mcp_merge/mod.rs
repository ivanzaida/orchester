//! The three-way merge of a team MCP server's form (`Agents · MCP server · admin · draft` A18 `bk4i9`, A19 `DV8OI`,
//! `docs/design/team-configuration.md` rule 18): the version a draft was edited from (its *base*), the version the
//! team has since (*theirs*) and the draft (*mine*), settled field by field. A field only one side changed takes that
//! side; a field both changed to the same value is that value; a field both changed to different values is the
//! person's to settle, and nothing here picks for them: [`merge`] refuses until each such field has a [`Pick`]. Pure:
//! it reads and writes nothing.

mod fields;

use self::fields::MergeFields;
use crate::app_mcp::setup::NewTeamMcpServer;

/// One field of the form the merge settles on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum McpField {
  /// What the server is called; the form does not edit it, so a rename the team made is taken.
  Name,
  /// A program each computer starts, or a server reached at an address.
  RunsAs,
  Program,
  Arguments,
  Folder,
  Variables,
  Url,
  /// The one header that carries a secret.
  Header,
  /// The tools that run without asking.
  ExemptTools,
}

impl McpField {
  /// Every field, in the order the form lists them.
  pub const ALL: [Self; 9] = [
    Self::Name,
    Self::RunsAs,
    Self::Program,
    Self::Arguments,
    Self::Folder,
    Self::Variables,
    Self::Url,
    Self::Header,
    Self::ExemptTools,
  ];
}

/// How a field stands among the base, theirs and mine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
  /// Neither side changed it.
  Unchanged,
  /// Both changed it, to the same value.
  Same,
  /// Only the person changed it: the merge keeps it.
  KeepsMine,
  /// Only the team changed it: the merge takes it.
  TakesTheirs,
  /// Both changed it, differently: the person picks.
  BothChanged,
}

/// The side a field both changed is settled with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
  Mine,
  Theirs,
}

/// How `mine` and `theirs` stand against `base`, which is `None` when the version the draft was edited from is not kept
/// here: nothing then says who changed what, so any difference is both's.
pub fn settle<T: PartialEq>(base: Option<&T>, theirs: &T, mine: &T) -> Verdict {
  match base {
    _ if mine == theirs => {
      if base == Some(mine) {
        Verdict::Unchanged
      } else {
        Verdict::Same
      }
    }
    Some(base) if mine == base => Verdict::TakesTheirs,
    Some(base) if theirs == base => Verdict::KeepsMine,
    _ => Verdict::BothChanged,
  }
}

/// One field that is not the same in all three, as the comparison shows it: each value as the list of words it is
/// (empty for none).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
  pub field: McpField,
  pub base: Option<Vec<String>>,
  pub theirs: Vec<String>,
  pub mine: Vec<String>,
  pub verdict: Verdict,
}

/// What the three versions hold differently, field by field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Comparison {
  /// Only the fields that are not the same in all three, those both changed first.
  pub rows: Vec<Row>,
}

impl Comparison {
  /// The fields both sides changed, which the person picks.
  pub fn both_changed(&self) -> Vec<McpField> {
    self.with(Verdict::BothChanged).map(|row| row.field).collect()
  }

  /// How many rows have `verdict`.
  pub fn count(&self, verdict: Verdict) -> usize {
    self.with(verdict).count()
  }

  fn with(&self, verdict: Verdict) -> impl Iterator<Item = &Row> {
    self.rows.iter().filter(move |row| row.verdict == verdict)
  }
}

/// Compares `mine` with `theirs` against `base`. The fields that do not apply to how the merge makes the server run (a
/// URL for a program) are left out.
pub fn compare(base: Option<&NewTeamMcpServer>, theirs: &NewTeamMcpServer, mine: &NewTeamMcpServer) -> Comparison {
  let (base, theirs, mine) = (
    base.map(MergeFields::of),
    MergeFields::of(theirs),
    MergeFields::of(mine),
  );
  let mut rows: Vec<Row> = McpField::ALL
    .into_iter()
    .filter_map(|field| {
      let verdict = settle(
        base.as_ref().map(|base| base.value(field)).as_ref(),
        &theirs.value(field),
        &mine.value(field),
      );
      (verdict != Verdict::Unchanged && applies(field, [&theirs, &mine])).then(|| Row {
        field,
        base: base.as_ref().map(|base| base.value(field)),
        theirs: theirs.value(field),
        mine: mine.value(field),
        verdict,
      })
    })
    .collect();
  rows.sort_by_key(|row| rank(row.verdict));
  Comparison { rows }
}

/// Why a merge is not made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unresolved(pub Vec<McpField>);

/// The server `mine` and `theirs` make together over `base`: what only one side changed, and for each field both
/// changed differently the side `picks` names. Refused with the fields that have no pick, so neither side is dropped
/// unseen.
pub fn merge(
  base: Option<&NewTeamMcpServer>,
  theirs: &NewTeamMcpServer,
  mine: &NewTeamMcpServer,
  picks: &[(McpField, Pick)],
) -> Result<NewTeamMcpServer, Unresolved> {
  let (base, theirs, mine) = (
    base.map(MergeFields::of),
    MergeFields::of(theirs),
    MergeFields::of(mine),
  );
  let mut merged = mine.clone();
  let mut unresolved = Vec::new();
  for field in McpField::ALL {
    let verdict = settle(
      base.as_ref().map(|base| base.value(field)).as_ref(),
      &theirs.value(field),
      &mine.value(field),
    );
    let side = match verdict {
      Verdict::Unchanged | Verdict::Same | Verdict::KeepsMine => Pick::Mine,
      Verdict::TakesTheirs => Pick::Theirs,
      // A field that is no part of the server the merge makes (a URL once it runs a program) asks nothing.
      Verdict::BothChanged if !merged.applies(field) => continue,
      Verdict::BothChanged => match picks.iter().find(|(picked, _)| *picked == field) {
        Some((_, pick)) => *pick,
        None => {
          unresolved.push(field);
          continue;
        }
      },
    };
    if side == Pick::Theirs {
      merged.take(field, &theirs);
    }
  }
  if unresolved.is_empty() {
    Ok(merged.into_server())
  } else {
    Err(Unresolved(unresolved))
  }
}

/// Whether `field` is part of how either side makes the server run.
fn applies(field: McpField, sides: [&MergeFields; 2]) -> bool {
  sides.iter().any(|side| side.applies(field))
}

/// The order rows are shown in: what the person must settle first.
const fn rank(verdict: Verdict) -> u8 {
  match verdict {
    Verdict::BothChanged => 0,
    Verdict::TakesTheirs => 1,
    Verdict::KeepsMine => 2,
    Verdict::Same | Verdict::Unchanged => 3,
  }
}
