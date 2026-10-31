//! The contract of accepting an invite in the browser (`docs/architecture/server.md`, "Accepting outside Orchester",
//! row 77): the token's purpose, the cloud's listing and receipt endpoint, the team server's `POST /auth/cloud/accept`
//! and its `410 invite_ended`, and the server's signed receipt. Older payloads without the new fields still read.
//!
//! `cargo test -p orchester-api --features identity --test invites_accept invites_accept`

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use orchester_api::{
  CloudAccept, CloudAccepted, ErrorCode, InviteKind, MemberRole, SharingError, SharingErrorDetails,
  cloud::{CloudInvite, IdentityTokenRequest, InviteJoined, TokenPurpose},
  identity::{IdentityError, PrivateKey, SigningKey, verify_invite_acceptance, verify_statement},
  statements::{INVITE_ACCEPTANCE_TYPE, INVITE_REVOCATION_TYPE, InviteAcceptance},
};
use orchester_core::id::{InviteId, TeamServerId};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

const INVITE: &str = "01a183cf-b73c-71db-8a62-bd509770cd4d";
const OTHER_INVITE: &str = "01a183cf-b73c-71db-8a62-bd509770cd4f";
const PROJECT: &str = "01a183cf-b73c-71db-8a62-bd509770cd50";
const USER: &str = "01a183cf-b73c-71db-8a62-bd509770cd4e";
const SESSION: &str = "01a183cf-b73c-71db-8a62-bd509770cd51";
const ACCOUNT: &str = "0191c7c0-0000-7000-8000-000000000001";
const SERVER: &str = "0199f0a0-0000-7000-8000-000000000001";
/// Ed25519 PKCS#8 documents made for tests only (the same as `test_support::cloud_keys`); they sign nothing outside.
const SERVER_KEY: &str = "3053020101300506032b657004220420adfab66f382aaa72a53a8960fee4a52fd00b01bde82e007de7d6259e3a2543d0a12303210052ee3b44c98237fedeb12e5ab420d43fdfe90383bb571607cc41b66eaf272c3e";
const OTHER_KEY: &str = "3053020101300506032b65700422042096a1366df61a8a963045c82480e9ff10671dd04653fc8f5b46fe763f8f2ffc28a12303210086e961cfbd6dd6506beba7150c72841a453d6be4d129ab870dc2f56e01def44d";

fn key(hex: &str) -> SigningKey {
  let bytes = (0..hex.len())
    .step_by(2)
    .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).expect("hex"))
    .collect();
  SigningKey::new(&PrivateKey::from_pkcs8(bytes)).expect("a fixed key")
}

fn round_trip<T: Serialize + DeserializeOwned>(value: &Value) -> T {
  let read: T = serde_json::from_value(value.clone()).unwrap_or_else(|error| panic!("{value}: {error}"));
  assert_eq!(&serde_json::to_value(&read).expect("serialize"), value);
  read
}

fn invite_id(text: &str) -> InviteId {
  text.parse().expect("an invite id")
}

fn receipt(server: TeamServerId) -> InviteAcceptance {
  InviteAcceptance {
    id: invite_id(INVITE),
    server,
    subject: ACCOUNT.parse().expect("a cloud account id"),
    issued_at: 1_791_000_000_000,
  }
}

#[test]
fn invites_accept_token_request_takes_an_invite_with_purpose_accept() {
  let read: IdentityTokenRequest = round_trip(&json!({ "audience": "server", "purpose": "accept", "invite": INVITE }));
  assert_eq!(read.purpose, Some(TokenPurpose::Accept));
  assert_eq!(read.invite, Some(invite_id(INVITE)));

  let older: IdentityTokenRequest =
    serde_json::from_value(json!({ "audience": "server", "invite": INVITE })).expect("a join's request");
  assert_eq!(older.purpose, None);
}

#[test]
fn invites_accept_listed_invite_says_when_it_was_accepted_and_older_listings_read() {
  let listed = json!({
    "id": INVITE, "kind": "project", "role": "member", "server": { "id": SERVER, "name": "Kontur", "address":
    "https://team.example.test" }, "project": { "address": "https://team.example.test/kontur", "name": "Kontur" },
    "inviter_display_name": "Lurq", "expires_at": 20, "needs_two_factor": false, "accepted_at": 15
  });
  let read: CloudInvite = round_trip(&listed);
  assert_eq!(read.accepted_at, Some(15));

  let mut older = listed;
  let fields = older.as_object_mut().expect("an object");
  fields.remove("accepted_at");
  fields.remove("needs_two_factor");
  let read: CloudInvite = serde_json::from_value(older).expect("an older listing");
  assert_eq!(read.accepted_at, None);
}

#[test]
fn invites_accept_receipt_delivery_reads_back_and_refuses_unknown_fields() {
  let read: InviteJoined = round_trip(&json!({ "receipt": "signed-receipt" }));
  assert_eq!(read.receipt, "signed-receipt");
  assert!(serde_json::from_value::<InviteJoined>(json!({ "receipt": "r", "invite": INVITE })).is_err());
}

#[test]
fn invites_accept_server_request_reads_back_with_and_without_a_key_set() {
  let read: CloudAccept = round_trip(&json!({ "identity_token": "token", "key_set": "key-set" }));
  assert_eq!(read.key_set.as_deref(), Some("key-set"));
  let read: CloudAccept = round_trip(&json!({ "identity_token": "token" }));
  assert_eq!(read.key_set, None);
  assert!(serde_json::from_value::<CloudAccept>(json!({ "identity_token": "t", "device": "laptop" })).is_err());
}

#[test]
fn invites_accept_server_answer_reads_back_signed_in_or_waiting_for_approval() {
  let signed_in = json!({
    "browser_session": {
      "token": "orch_browser",
      "session": { "id": SESSION, "device": "Browser", "created_at": 1, "last_used_at": 1, "current": true,
        "via": "cloud", "mfa": true },
      "user": { "id": USER, "username": "yuri", "display_name": "Yuri", "admin": false, "disabled": false,
        "cloud": true }
    },
    "membership": { "kind": "project", "project": PROJECT, "role": "member" },
    "receipt": "signed-receipt",
    "approval_required": false
  });
  let read: CloudAccepted = round_trip(&signed_in);
  assert_eq!(read.membership.kind, InviteKind::Project);
  assert_eq!(read.membership.role, Some(MemberRole::Member));
  assert!(read.browser_session.is_some());

  let waiting = json!({ "membership": { "kind": "server" }, "receipt": "signed-receipt", "approval_required": true });
  let read: CloudAccepted = serde_json::from_value(waiting.clone()).expect("an answer without a sign-in");
  assert!(read.browser_session.is_none() && read.approval_required);
  assert_eq!(read.membership.project, None);
  let mut written = serde_json::to_value(&read).expect("serialize");
  written.as_object_mut().expect("an object").remove("browser_session");
  assert_eq!(written, waiting);
}

#[test]
fn invites_accept_invite_ended_carries_the_signed_revocation() {
  let ended = json!({ "error": "invite_ended", "message": "This invite has ended", "revocation": "signed-revocation" });
  let read: SharingError = round_trip(&ended);
  assert_eq!(read.error.error, ErrorCode::InviteEnded);
  assert_eq!(
    read.details,
    SharingErrorDetails::Revocation {
      revocation: "signed-revocation".into()
    }
  );
}

#[test]
fn invites_accept_receipt_signs_and_verifies_with_fixed_keys() {
  let server = key(SERVER_KEY);
  let id = server.public_key().server_id();
  let statement = receipt(id);
  let signed = server.sign_invite_acceptance(&statement).expect("sign");
  assert_eq!(signed, server.sign_invite_acceptance(&statement).expect("sign again"));
  let checked = verify_invite_acceptance(&signed, &server.public_key(), (invite_id(INVITE), id)).expect("verifies");
  assert_eq!(checked, statement);
  let generic: InviteAcceptance =
    verify_statement(&signed, INVITE_ACCEPTANCE_TYPE, &server.public_key()).expect("a statement of its type");
  assert_eq!(generic, statement);
}

#[test]
fn invites_accept_receipt_refuses_tampering_other_keys_types_and_invites() {
  let server = key(SERVER_KEY);
  let id = server.public_key().server_id();
  let expected = (invite_id(INVITE), id);
  let signed = server.sign_invite_acceptance(&receipt(id)).expect("sign");

  let parts: Vec<&str> = signed.split('.').collect();
  let mut changed = receipt(id);
  changed.subject = "0191c7c0-0000-7000-8000-000000000002".parse().expect("an account id");
  let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&changed).expect("payload"));
  let tampered = format!("{}.{payload}.{}", parts[0], parts[2]);
  assert_eq!(
    verify_invite_acceptance(&tampered, &server.public_key(), expected),
    Err(IdentityError::BadSignature)
  );

  let other = key(OTHER_KEY);
  assert_eq!(
    verify_invite_acceptance(&signed, &other.public_key(), expected),
    Err(IdentityError::UnknownKey)
  );
  assert_eq!(
    verify_statement::<InviteAcceptance>(&signed, INVITE_REVOCATION_TYPE, &server.public_key()),
    Err(IdentityError::Malformed)
  );
  assert_eq!(
    verify_invite_acceptance(&signed, &server.public_key(), (invite_id(OTHER_INVITE), id)),
    Err(IdentityError::WrongAudience)
  );
  let other_server = other.public_key().server_id();
  assert_eq!(
    verify_invite_acceptance(&signed, &server.public_key(), (invite_id(INVITE), other_server)),
    Err(IdentityError::WrongAudience)
  );
}
