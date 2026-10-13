use orchester_core::integration::local::LocalError;
use orchester_exec::local_git::LocalGitError;
use orchester_workspace::tasks::local_integration::IntegrationError;

#[test]
fn local_policy_storage_ownership_unsupported_has_its_dedicated_presentation() {
  let key = "local_integration.error.storageOwnership";
  let direct =
    orchester_app::ui::local_integration_error::problem(IntegrationError::State(LocalError::StoreOwnershipUnsupported));
  let execution = orchester_app::ui::local_integration_error::problem(IntegrationError::Git(LocalGitError::State(
    LocalError::StoreOwnershipUnsupported,
  )));
  assert_eq!(direct.0, key);
  assert_eq!(execution.0, key);
  let locale: serde_json::Value =
    serde_json::from_str(include_str!("../../assets/locales/en/local_integration.json")).expect("shipped locale");
  let dedicated = locale["local_integration"]["error"]["storageOwnership"]
    .as_str()
    .expect("dedicated unsupported explanation");
  assert!(dedicated.contains("on this platform"));
  assert_ne!(
    dedicated,
    locale["local_integration"]["error"]["io"]
      .as_str()
      .expect("IO explanation")
  );
}
