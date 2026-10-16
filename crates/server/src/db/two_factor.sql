-- Schema 5, two-factor authentication (docs/architecture/server.md, "Invites"): a device token issued for a cloud
-- identity token whose device signed in with two-factor authentication keeps that, and only such a token sends invites
-- by email.
ALTER TABLE sessions ADD COLUMN mfa INTEGER NOT NULL DEFAULT 0 CHECK (mfa IN (0,1));
-- A code entered in the app needs the app's cloud identity token with that claim. A pairing code made by such a token
-- keeps it, so a browser signing in to the admin with it may send invites too.
ALTER TABLE codes ADD COLUMN mfa INTEGER NOT NULL DEFAULT 0 CHECK (mfa IN (0,1));
UPDATE meta SET schema_version=5;
