import type { Metadata } from "next";

import { CloudBrand, CloudFooter } from "@/components/page";
import { appDevices } from "@/lib/account";
import { t } from "@/lib/i18n";
import { formToken } from "@/lib/server/visitor";

import { Devices } from "./devices";
import { GetStarted } from "./get-started";
import { AccountHeader } from "./header";
import { loadAccount } from "./load";
import { AccountNav } from "./nav";
import { Invites, Servers } from "./registry";
import { Settings } from "./settings";
import styles from "./account.module.css";

export const metadata: Metadata = { title: t("account.pageTitle") };

/**
 * The Account page (`H47EAc`; new account `Z4Q3F`, password open `ZyAsL`, a device signed out `wcFhu`, phone `F9QYD`):
 * where the web lands after sign-in.
 */
export default async function AccountPage() {
  const data = await loadAccount();
  const csrf = await formToken();
  const now = Date.now();
  const { account } = data;
  const invites = data.invites.kind === "list" ? data.invites.items.length : 0;
  // The servers this account linked, which deleting it leaves with no owner; `null` when the list did not load.
  const owned = data.servers.kind === "list" ? data.servers.items.filter((s) => s.owner).map((s) => s.name) : null;
  return (
    <>
      <AccountHeader brand={<CloudBrand />} name={account.displayName} email={account.email} csrf={csrf} />
      <div className={styles.body}>
        <div className={styles.wrap}>
          <AccountNav invites={invites} />
          <main className={styles.main}>
            <GetStarted apps={appDevices(data.devices)} email={account.email} system={data.system} />
            <Invites listing={data.invites} email={account.email} csrf={csrf} now={now} />
            <Servers listing={data.servers} email={account.email} />
            <Devices devices={data.devices} csrf={csrf} now={now} />
            <Settings name={account.displayName} email={account.email} csrf={csrf} owned={owned} />
          </main>
        </div>
      </div>
      <CloudFooter />
    </>
  );
}
