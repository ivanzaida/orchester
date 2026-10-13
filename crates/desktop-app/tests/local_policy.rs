//! Private authority regressions, not live human outcomes or native/Pencil verification.
#[path = "local_policy/client.rs"]
mod client;
#[path = "../../workspace/tests/local_integration/support.rs"]
mod domain_support;
#[path = "local_policy/editor.rs"]
mod editor;
#[path = "local_integration/support.rs"]
mod integration_support;
#[path = "local_policy/legacy.rs"]
mod legacy;
#[path = "local_policy/privacy.rs"]
mod privacy;
#[path = "local_policy/queued.rs"]
mod queued;
#[path = "local_policy/shared_checkout.rs"]
mod shared_checkout;
mod support;
#[path = "local_policy/unsupported.rs"]
mod unsupported;
