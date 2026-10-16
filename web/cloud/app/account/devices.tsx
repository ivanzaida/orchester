"use client";

import { useActionState, useEffect, useState } from "react";

import { Chip, DoneRow, Panel, Row, RowLine, Section, TopLine } from "@orchester/ui/components/list";
import { Submit } from "@orchester/ui/components/submit";

import { ErrorLine } from "@/components/fields";
import { CsrfField, Hidden } from "@/components/hidden";
import { LastActive } from "@/components/moment";
import type { Device } from "@/lib/account";
import { t } from "@/lib/i18n";

import { signOutDeviceAction, type DeviceState } from "./actions";
import { SignOutForm } from "./header";
import { markSection } from "./nav";
import styles from "./account.module.css";

/** A browser keeps the name it signed in with (*Firefox on Windows*); an app is *Orchester on <computer>*. */
export function deviceName(device: Device): string {
  return device.platform === "web" ? device.name : t("account.devices.appName", { name: device.name });
}

function kind(device: Device): string {
  return device.platform === "web"
    ? t("account.devices.browser")
    : t("account.devices.app", { platform: t(`account.devices.${device.platform}`) });
}

function DeviceRow(props: { device: Device; csrf: string; now: number; action: (form: FormData) => void }) {
  const { device } = props;
  const signOut = device.current ? (
    <SignOutForm csrf={props.csrf} kind="secondary" icon={false} label={t("account.devices.signOut")} />
  ) : (
    <form action={props.action}>
      <CsrfField token={props.csrf} />
      <Hidden name="device" value={device.id} />
      <Submit kind="secondary" row label={t("account.devices.signOut")} busyLabel={t("account.devices.signingOut")} />
    </form>
  );
  const side = (
    <span className={styles.deviceSide}>
      <span className={styles.lastActive}>
        <RowLine tone="secondary">
          <LastActive at={device.lastUsedAt} now={props.now} current={device.current} />
        </RowLine>
      </span>
      {signOut}
    </span>
  );
  return (
    <Row glyph={device.platform === "web" ? "globe" : "app-window"} align="center" side={side}>
      <TopLine name={deviceName(device)} weight="medium">
        {device.current && <Chip>{t("account.devices.thisDevice")}</Chip>}
      </TopLine>
      <RowLine>{kind(device)}</RowLine>
    </Row>
  );
}

/**
 * *Devices* (`J6xiP`). Another device is signed out at once, and its row is replaced in place by a success line that
 * stays until the page is reloaded (`wcFhu`); this browser's own *Sign out* signs out as a whole.
 */
export function Devices(props: { devices: readonly Device[]; csrf: string; now: number }) {
  const [state, action] = useActionState<DeviceState, FormData>(signOutDeviceAction, {});
  const [signedOut, setSignedOut] = useState<readonly string[]>([]);
  if (state.signedOut !== undefined && !signedOut.includes(state.signedOut)) {
    setSignedOut([...signedOut, state.signedOut]);
  }
  useEffect(() => {
    if (signedOut.length > 0) {
      markSection("devices");
    }
  }, [signedOut.length]);
  // Counted from the list as loaded: a reload (after a password change) no longer has the devices signed out here.
  const count = props.devices.filter((device) => !signedOut.includes(device.id)).length;
  return (
    <Section
      id="devices"
      title={t("account.devices.title")}
      meta={t("account.devices.count", { count })}
      description={t("account.devices.description")}
    >
      <Panel>
        {props.devices.map((device) =>
          signedOut.includes(device.id) ? (
            <DoneRow
              key={device.id}
              title={t("account.devices.signedOutTitle", { name: deviceName(device) })}
              text={t("account.devices.signedOutText")}
            />
          ) : (
            <DeviceRow key={device.id} device={device} csrf={props.csrf} now={props.now} action={action} />
          ),
        )}
      </Panel>
      {state.error && <ErrorLine message={state.error} />}
    </Section>
  );
}
