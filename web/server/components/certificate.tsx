import { CertificateCheck } from "@orchester/ui/components/cert-check";

import { shortFingerprint } from "@/lib/codes";
import { t } from "@/lib/i18n";
import type { ServerInfo } from "@/lib/model";

/**
 * The first-visit certificate check (`nNRnL`) beside the sign-in and setup cards, only for a certificate the server
 * made itself: one from the team's own CA or behind a public domain never needs it.
 */
export function FirstVisitCheck({ server }: { server: ServerInfo }) {
  const certificate = server.certificate;
  if (certificate?.mode !== "self_signed") {
    return null;
  }
  return (
    <CertificateCheck
      title={t("cert.title")}
      body={t("cert.body", { server: server.name })}
      facts={[
        { label: t("cert.sha256"), value: shortFingerprint(certificate.sha256), mono: true },
        { label: t("cert.checkCode"), value: certificate.checkCode, mono: true },
      ]}
      footnote={t("cert.footnote")}
    />
  );
}
