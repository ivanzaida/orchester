import { Submit } from "@orchester/ui/components/submit";

import { t } from "@/lib/i18n";

import { CsrfField, Hidden } from "./hidden";

/**
 * *Link to Orchester Cloud*: a plain form to the trip route, which asks the server for its signed link statement and
 * sends the browser to the cloud with it (`CeBNR`, Settings › not linked in `WV4AT`).
 */
export function LinkForm({ csrf, back, kind }: { csrf: string; back: "setup" | "settings"; kind: "primary" | "secondary" }) {
  return (
    <form action="/admin/trip" method="post">
      <CsrfField token={csrf} />
      <Hidden name="purpose" value="link" />
      <Hidden name="back" value={back} />
      <Submit kind={kind} row={kind === "secondary"} icon={kind === "primary" ? "cloud" : "link"} label={t("settings.linkButton")}
        busyLabel={t("settings.linking")} />
    </form>
  );
}
