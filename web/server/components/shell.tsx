import type { ReactNode } from "react";

import { Avatar } from "@orchester/ui/components/avatar";
import { HeaderLines, SiteHeader } from "@orchester/ui/components/header";
import { Icon } from "@orchester/ui/components/icon";
import { SideNav } from "@orchester/ui/components/nav";
import { Tabs } from "@orchester/ui/components/tabs";

import { initials } from "@/lib/codes";
import { t } from "@/lib/i18n";
import type { AdminContext } from "@/lib/server/admin";
import { formToken } from "@/lib/server/visitor";

import { AdminBrand, AdminFooter, AdminMark } from "./page";
import { SignOut } from "./sign-out";
import styles from "./shell.module.css";

export type Area = "projects" | "people" | "devices" | "settings";

interface Item {
  readonly area: Area;
  readonly href: string;
  readonly nav: string;
  readonly tab: string;
}

/** The admin's pages for this person: all four for admins; Projects only for owners who are not admins. */
function items(context: AdminContext): Item[] {
  const projects = context.projects.length;
  const list: Item[] = [
    { area: "projects", href: "/admin", nav: t("nav.projects", { count: projects }), tab: t("nav.tabProjects", { count: projects }) },
  ];
  if (context.people !== null && context.devices !== null) {
    const people = context.people.length;
    const devices = context.devices.length;
    list.push(
      { area: "people", href: "/admin/people", nav: t("nav.people", { count: people }), tab: t("nav.tabPeople", { count: people }) },
      { area: "devices", href: "/admin/devices", nav: t("nav.devices", { count: devices }), tab: t("nav.tabDevices", { count: devices }) },
      { area: "settings", href: "/admin/settings", nav: t("nav.settings"), tab: t("nav.tabSettings") },
    );
  }
  return list;
}

function roleLine(context: AdminContext): string {
  const role = context.me.admin ? t("header.admin") : t("header.owner");
  return context.via === "cloud" ? t("header.viaCloud", { role }) : t("header.viaCode", { role });
}

/** On a phone the person and *Sign out* move into this menu (`YtDIe`); it opens without script. */
function Menu({ name, line, csrf }: { name: string; line: string; csrf: string }) {
  return (
    <details className={styles.menu}>
      <summary className={styles.menuButton} aria-label={t("header.menu")}>
        <Icon name="menu" size="field" />
      </summary>
      <div className={styles.menuPanel}>
        <HeaderLines title={name} line={line} />
        <SignOut csrf={csrf} />
      </div>
    </details>
  );
}

/**
 * The signed-in pages (`FCQCr`): the header (`NNrob`), the server's links at the left (`EJ8tl`) or as tabs on a
 * narrow window (`mYBka`), the page, the footer.
 */
export async function AdminShell({ context, area, children }: { context: AdminContext; area: Area; children: ReactNode }) {
  const csrf = await formToken();
  const { server, me } = context;
  const line = roleLine(context);
  const avatar = <Avatar initials={initials(me.displayName)} size="header" />;
  const links = items(context);
  return (
    <>
      <SiteHeader
        brand={<AdminBrand />}
        mark={<AdminMark />}
        context={<HeaderLines title={server.name} line={context.place.host} mono />}
        who={<>{avatar}<HeaderLines title={me.displayName} line={line} /></>}
        avatar={avatar}
        action={<SignOut csrf={csrf} />}
        menu={<Menu name={me.displayName} line={line} csrf={csrf} />}
      />
      <div className={styles.body}>
        <div className={styles.wrap}>
          <div className={styles.nav}>
            <SideNav title={server.name} items={links.map((item) => ({ href: item.href, label: item.nav, current: item.area === area }))} />
          </div>
          <main className={styles.main}>
            {links.length > 1 && (
              <div className={styles.tabs}>
                <Tabs label={t("nav.label")} items={links.map((item) => ({ href: item.href, label: item.tab, current: item.area === area }))} />
              </div>
            )}
            {children}
          </main>
        </div>
      </div>
      <AdminFooter server={server} />
    </>
  );
}
