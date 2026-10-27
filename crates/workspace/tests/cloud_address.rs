//! Orchester Cloud's web and API addresses (`docs/cloud-api.md`, "Base"): the public cloud's pair by default, a test or
//! self-run cloud's from Settings › Orchester Cloud › Advanced with or without an API address of its own, and what an
//! earlier version stored with the public cloud moving to the pair at the start, without signing out.
//!
//! `cargo test -p orchester-workspace --test cloud_address cloud_address_`

#[path = "sharing_support/account_cloud.rs"]
mod account_cloud;
#[path = "sharing_support/desktop.rs"]
mod desktop;
#[path = "server_support/mod.rs"]
mod server_support;
#[path = "sharing_support/project.rs"]
mod support;

use account_cloud::{AccountCloud, EMAIL, PASSWORD};
use desktop::Desktop;
use orchester_api::{
  Credential,
  client::{CloudAddress, ServerAddress},
};
use orchester_core::secret::SecretVault;
use orchester_workspace::{
  secrets::Vault,
  sharing::{
    Servers,
    cloud::{BUILD_ADDRESS, BUILD_API_ADDRESS, Cloud, PUBLIC_ADDRESS, PUBLIC_API_ADDRESS, WebPage, build_address},
  },
};

/// A new start of `desktop`'s session: what the application database says.
fn restarted(desktop: &Desktop) -> Cloud {
  let vault = Vault::with(desktop.vault.clone());
  let servers = Servers::new(desktop.workspace.clone(), vault.clone());
  Cloud::new(desktop.workspace.clone(), vault, servers)
}

#[test]
fn cloud_address_of_a_build_is_the_public_web_address_and_its_api_address() {
  assert_eq!(PUBLIC_ADDRESS, "https://orchester.cloud");
  assert_eq!(PUBLIC_API_ADDRESS, "https://api.orchester.cloud");
  if option_env!("ORCHESTER_CLOUD_URL").is_some() || option_env!("ORCHESTER_CLOUD_API_URL").is_some() {
    return;
  }
  assert_eq!(
    (BUILD_ADDRESS, BUILD_API_ADDRESS),
    (PUBLIC_ADDRESS, Some(PUBLIC_API_ADDRESS))
  );
  let address = build_address().expect("the build's cloud");
  assert_eq!(address.web().as_str(), "https://orchester.cloud");
  assert_eq!(address.api_base(), "https://api.orchester.cloud/v1");
}

#[test]
fn cloud_address_with_an_api_address_signs_in_there_and_opens_pages_on_the_web_address() {
  let cloud = AccountCloud::start();
  let web = ServerAddress::parse("https://cloud.test.lurq.dev").expect("web address");
  let api = ServerAddress::parse(&cloud.url()).expect("API address");
  let desktop = Desktop::at(CloudAddress::new(web, Some(api)));
  desktop
    .block_on(desktop.cloud.sign_in(EMAIL.into(), Credential::new(PASSWORD.into())))
    .expect("signed in at the API address");
  let account = desktop.cloud.account().expect("account").expect("signed in");
  assert_eq!(account.cloud_address, "https://cloud.test.lurq.dev");
  assert_eq!(account.cloud_api_address, Some(cloud.url()));
  assert_eq!(
    desktop.cloud.web_page(WebPage::SignUp).expect("page"),
    "https://cloud.test.lurq.dev/signup"
  );
  let set = restarted(&desktop).address().expect("address");
  assert!(!set.default, "a test cloud");
  assert_eq!(set.address.api_base(), format!("{}/v1", cloud.url()));
  desktop
    .block_on(restarted(&desktop).check_sign_in())
    .expect("the sign-in checked at the API address");
}

#[test]
fn cloud_address_without_an_api_address_uses_the_old_paths_on_the_web_address() {
  let cloud = AccountCloud::start();
  let desktop = Desktop::signed_in(&cloud);
  let account = desktop.cloud.account().expect("account").expect("signed in");
  assert_eq!((account.cloud_address, account.cloud_api_address), (cloud.url(), None));
  let set = desktop.cloud.address().expect("address");
  assert_eq!(set.address.api(), None);
  assert_eq!(set.address.api_base(), format!("{}/api/v1", cloud.url()));
  assert_eq!(
    desktop.cloud.web_page(WebPage::Forgot).expect("page"),
    format!("{}/forgot", cloud.url())
  );
}

#[test]
fn cloud_address_of_the_public_cloud_moves_to_its_api_address_without_signing_out() {
  let cloud = AccountCloud::start();
  let desktop = Desktop::signed_in(&cloud);
  desktop.move_account_to(PUBLIC_ADDRESS);
  desktop.store_cloud_address(PUBLIC_ADDRESS);
  let token = desktop.vault.read(&desktop.device_key()).expect("the token");
  let moved = restarted(&desktop);
  assert!(moved.is_signed_in(), "the sign-in holds across the move");
  let account = moved.account().expect("account").expect("still signed in");
  assert_eq!(account.email, EMAIL);
  assert_eq!(
    (account.cloud_address.as_str(), account.cloud_api_address.as_deref()),
    (PUBLIC_ADDRESS, Some(PUBLIC_API_ADDRESS))
  );
  let set = moved.address().expect("address");
  assert_eq!(
    set.default,
    BUILD_ADDRESS == PUBLIC_ADDRESS && BUILD_API_ADDRESS == Some(PUBLIC_API_ADDRESS),
    "named by hand, the public cloud is the build's own again"
  );
  assert_eq!(set.address.web().as_str(), PUBLIC_ADDRESS);
  assert_eq!(set.address.api_base(), "https://api.orchester.cloud/v1");
  let kept = desktop.vault.read(&desktop.device_key()).expect("the token stays");
  assert_eq!(kept.expose(), token.expose());
}

#[test]
fn cloud_address_of_a_self_run_cloud_without_an_api_address_stays() {
  let cloud = AccountCloud::start();
  let desktop = Desktop::signed_in(&cloud);
  desktop.move_account_to("https://cloud.kontur.lan");
  desktop.store_cloud_address("https://cloud.kontur.lan");
  let stayed = restarted(&desktop);
  let account = stayed.account().expect("account").expect("signed in");
  assert_eq!(
    (account.cloud_address.as_str(), account.cloud_api_address),
    ("https://cloud.kontur.lan", None)
  );
  let set = stayed.address().expect("address");
  assert!(!set.default);
  assert_eq!(set.address.api_base(), "https://cloud.kontur.lan/api/v1");
}
