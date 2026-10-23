//! Create drafts (`A5Av4` in Import resources `Dzzks`): what the import creates, which of this project's own
//! resources, connections and secrets it uses instead, what it leaves unresolved, and which callers the copied
//! tools keep.

use orchester_core::{
  id::ResourceId,
  import::{CatalogResource, Content, Entity, Item, Pick, Plan, Resolution},
  resource::{StepKind, ToolKind, VersionNumber},
};

use super::{
  data::Catalogs,
  review::{self, Subject},
};

/// A local draft the import creates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Created {
  pub subject: Subject,
  /// The copy's name.
  pub name: String,
  pub source_name: String,
  /// The content it takes.
  pub pick: Pick,
  /// Under a name typed for it.
  pub renamed: bool,
}

/// This project's own resource, connection, MCP server or secret used in place of the source's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reused {
  pub subject: Subject,
  /// The name in this project.
  pub name: String,
  pub source_name: String,
  /// The content of the source's resource the copies would have taken.
  pub source_pick: Option<Pick>,
  /// This project's latest published version of the resource; `None` for a draft, a connection or a secret.
  pub published: Option<VersionNumber>,
}

/// A connection, MCP server or secret the copies keep naming, which this project does not have.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Missing {
  pub subject: Subject,
  pub name: String,
  /// The copies that refer to it, by their names.
  pub users: Vec<String>,
}

/// A copied pipeline's step that may call a tool copied with it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Allowed {
  pub tool: String,
  pub tool_kind: ToolKind,
  /// The step's number in the pipeline, from 1, and its name.
  pub step: usize,
  pub step_name: String,
}

/// The callers a copied pipeline's tools keep, under the pipeline's copy's name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Access {
  pub pipeline: String,
  pub allowed: Vec<Allowed>,
}

/// What Create drafts lists.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Drafts {
  pub created: Vec<Created>,
  pub reused: Vec<Reused>,
  pub missing: Vec<Missing>,
  /// Each copied pipeline that calls a copied tool, with the steps that may.
  pub access: Vec<Access>,
}

pub fn drafts(catalogs: &Catalogs, plan: &Plan) -> Drafts {
  let mut drafts = Drafts::default();
  for item in plan.items() {
    let subject = review::subject(&catalogs.source, item.source(), item.pick());
    let source_name = review::name(&catalogs.source, item.source());
    match (item.resolution(), subject) {
      (Resolution::Copy { name, .. } | Resolution::Rename(Some((name, _))), Some(subject)) => {
        if let Some(pick) = item.pick() {
          drafts.created.push(Created {
            subject,
            name: name.as_str().to_owned(),
            source_name,
            pick,
            renamed: matches!(item.resolution(), Resolution::Rename(_)),
          });
        }
      }
      (Resolution::Use(target), Some(subject)) => drafts.reused.push(Reused {
        subject,
        name: review::name(&catalogs.target, *target),
        source_name,
        source_pick: item.pick(),
        published: match target {
          Entity::Resource(id) => catalogs.target.resource(*id).and_then(CatalogResource::published),
          Entity::Connection(_) | Entity::McpServer(_) | Entity::Secret(_) => None,
        },
      }),
      (Resolution::Unresolved, Some(subject)) => drafts.missing.push(Missing {
        subject,
        name: source_name,
        users: item
          .used_by()
          .iter()
          .map(|user| user_name(catalogs, plan, *user))
          .collect(),
      }),
      _ => {}
    }
  }
  drafts.access = access(catalogs, plan);
  drafts
}

/// The name the import gives the resource `id` of the source: its copy's, or its own when it is not copied.
fn user_name(catalogs: &Catalogs, plan: &Plan, id: ResourceId) -> String {
  let entity = Entity::Resource(id);
  plan
    .item(entity)
    .and_then(review::copy_name)
    .map_or_else(|| review::name(&catalogs.source, entity), str::to_owned)
}

/// The content an item's copy takes, if it is copied.
fn copied<'a>(catalogs: &'a Catalogs, item: &Item) -> Option<&'a Content> {
  let (Entity::Resource(id), Some(pick), true) = (item.source(), item.pick(), item.copies()) else {
    return None;
  };
  catalogs.source.resource(id)?.content(pick)
}

/// For every copied pipeline, its steps that call a tool copied with it and allowed to be called there.
fn access(catalogs: &Catalogs, plan: &Plan) -> Vec<Access> {
  plan
    .items()
    .iter()
    .filter_map(|item| {
      let Entity::Resource(pipeline) = item.source() else {
        return None;
      };
      let Some(Content::Pipeline(body)) = copied(catalogs, item) else {
        return None;
      };
      let allowed: Vec<Allowed> = body
        .steps()
        .iter()
        .enumerate()
        .filter_map(|(index, step)| {
          let StepKind::Tool { tool: Some(tool) } = step.kind else {
            return None;
          };
          let tool_item = plan.item(Entity::Resource(tool))?;
          let Some(Content::Tool(body)) = copied(catalogs, tool_item) else {
            return None;
          };
          let access = body.access()?;
          access.allows(pipeline, &step.key).then(|| Allowed {
            tool: review::copy_name(tool_item).unwrap_or_default().to_owned(),
            tool_kind: body.kind(),
            step: index + 1,
            step_name: step.name.as_str().to_owned(),
          })
        })
        .collect();
      (!allowed.is_empty()).then(|| Access {
        pipeline: review::copy_name(item).unwrap_or_default().to_owned(),
        allowed,
      })
    })
    .collect()
}
