import { ActionButton } from "@orchester/ui/components/button";
import { Icon, type IconName } from "@orchester/ui/components/icon";
import { Panel, Row, RowLine, Section, TopLine } from "@orchester/ui/components/list";
import { Reason } from "@orchester/ui/components/status";

import { t } from "@/lib/i18n";

import styles from "./account.module.css";

/**
 * *Get started* (`p4XPUl` compact, `fOZN9` full). Download addresses are not defined yet (the designer's notes), so
 * the download button is shown unavailable with its reason, and the macOS link is left out. *Open Orchester* needs an
 * `orchester://` scheme the desktop does not register yet, so it is left out too (report, decision 1).
 */
function Download({ system, row }: { system: "windows" | "macos"; row: boolean }) {
  const label = system === "macos" ? t("account.getStarted.downloadMac") : t("account.getStarted.downloadWindows");
  return (
    <ActionButton
      kind={row ? "secondary" : "primary"}
      row={row}
      inline={!row}
      icon="download"
      unavailable
      describedBy="downloads-reason"
    >
      {label}
    </ActionButton>
  );
}

function NotYet() {
  return <Reason id="downloads-reason">{t("account.getStarted.downloadsNotYet")}</Reason>;
}

/** Once an Orchester app has signed in to the account the section is one row, so the invites come first. */
function Compact({ apps, system }: { apps: number; system: "windows" | "macos" }) {
  return (
    <Panel>
      <Row glyph="app-window" align="center" fill side={<Download system={system} row />}>
        <TopLine name={t("account.getStarted.signedIn", { count: apps })} weight="medium" />
        <NotYet />
      </Row>
    </Panel>
  );
}

const STEPS: readonly { icon: IconName; title: "step1" | "step2" | "step3" }[] = [
  { icon: "log-in", title: "step1" },
  { icon: "mail-open", title: "step2" },
  { icon: "server", title: "step3" },
];

function Steps({ email }: { email: string }) {
  return (
    <div className={styles.steps}>
      <span className={styles.eyebrow}>{t("account.getStarted.howTitle")}</span>
      {STEPS.map((step, index) => (
        <div key={step.title} className={styles.step}>
          <span className={styles.number}>{String(index + 1).padStart(2, "0")}</span>
          <span className={styles.stepGlyph}>
            <Icon name={step.icon} size="glyph" />
          </span>
          <span className={styles.stepText}>
            <span className={styles.stepTitle}>{t(`account.getStarted.${step.title}Title`)}</span>
            <span className={styles.stepSummary}>{t(`account.getStarted.${step.title}Text`, { email })}</span>
          </span>
        </div>
      ))}
    </div>
  );
}

function Full({ email, system }: { email: string; system: "windows" | "macos" }) {
  return (
    <Panel>
      <div className={styles.start}>
        <div className={styles.get}>
          <span className={styles.getTitle}>{t("account.getStarted.getTitle")}</span>
          <RowLine tone="secondary" wide>
            {t("account.getStarted.getText")}
          </RowLine>
          <Download system={system} row={false} />
          <NotYet />
        </div>
        <Steps email={email} />
      </div>
    </Panel>
  );
}

export function GetStarted(props: { apps: number; email: string; system: "windows" | "macos" }) {
  return (
    <Section id="get-started" title={t("account.getStarted.title")}>
      {props.apps > 0 ? <Compact apps={props.apps} system={props.system} /> : <Full email={props.email} system={props.system} />}
    </Section>
  );
}
