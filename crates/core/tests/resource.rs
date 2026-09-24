use std::num::NonZeroU32;

use orchester_core::{
  resource::{Draft, PublishError, Slug, VersionNumber, publish},
  time::Timestamp,
};

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

#[test]
fn first_publish_creates_v1_and_later_drafts_continue_the_sequence() {
  let v1 = publish(Draft::new("first", None, at(1)), None, at(2)).unwrap();
  assert_eq!(v1.number(), VersionNumber::FIRST);
  assert_eq!(v1.published_at(), at(2));

  let mut draft = v1.to_draft(at(3));
  draft.edit("second", at(4));
  let v2 = publish(draft, Some(v1.number()), at(5)).unwrap();
  assert_eq!(v2.number().get(), 2);
  assert_eq!(*v2.body(), "second");
  assert_ne!(v1.id(), v2.id());
}

#[test]
fn publishing_a_stale_draft_is_rejected() {
  let v1 = VersionNumber::FIRST;
  let v2 = VersionNumber::new(NonZeroU32::new(2).unwrap());
  let draft = Draft::new("edited v1", Some(v1), at(1));
  assert_eq!(
    publish(draft, Some(v2), at(2)),
    Err(PublishError::StaleDraft {
      based_on: Some(v1),
      latest: Some(v2),
    })
  );
}

#[test]
fn a_new_resource_draft_cannot_overwrite_existing_versions() {
  let draft = Draft::new("fresh", None, at(1));
  assert!(matches!(
    publish(draft, Some(VersionNumber::FIRST), at(2)),
    Err(PublishError::StaleDraft { .. })
  ));
}

#[test]
fn version_numbers_have_an_upper_bound() {
  let last = VersionNumber::new(NonZeroU32::MAX);
  assert_eq!(
    publish(Draft::new((), Some(last), at(1)), Some(last), at(2)),
    Err(PublishError::VersionLimit)
  );
}

#[test]
fn slug_accepts_kebab_case_only() {
  assert_eq!(Slug::new("feature-delivery").unwrap().as_str(), "feature-delivery");
  for invalid in ["", "Feature", "feature_delivery", "-lead", "trail-", "double--hyphen"] {
    assert!(Slug::new(invalid).is_err(), "{invalid} should be rejected");
  }
}

#[test]
fn unique_slug_follows_the_name_and_avoids_taken_ones() {
  use orchester_core::{name::Name, resource::Slug};
  let fallback = Slug::new("agent").expect("valid slug");
  let name = |value: &str| Name::new(value).expect("valid name");
  let slug = |value: &str| Slug::new(value).expect("valid slug");
  assert_eq!(
    Slug::unique_for(&name("Code Reviewer"), &fallback, &[]),
    slug("code-reviewer")
  );
  assert_eq!(Slug::unique_for(&name("Ревьюер"), &fallback, &[]), fallback);
  let taken = [slug("builder"), slug("builder-2")];
  assert_eq!(
    Slug::unique_for(&name("Builder!"), &fallback, &taken),
    slug("builder-3")
  );
  let long = Slug::unique_for(&name(&"a".repeat(120)), &fallback, &[slug(&"a".repeat(64))]);
  assert_eq!(long.as_str().len(), Slug::MAX_CHARS);
  assert!(long.as_str().ends_with("-2"));
}
