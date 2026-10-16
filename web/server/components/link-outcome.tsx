import { Callout } from "@orchester/ui/components/callout";

import { t, type MessageKey } from "@/lib/i18n";

/** What a trip to link the server ended with, from the page address (`?link=…`): fixed words only. */
const OUTCOMES: Readonly<Record<string, MessageKey>> = {
  done: "link.done",
  conflict: "link.conflict",
  invalid: "link.invalid",
  forbidden: "link.forbidden",
  refused: "link.refused",
  unavailable: "link.unavailable",
  trip: "link.trip",
  expired: "link.expired",
};

export function LinkOutcome({ outcome, server }: { outcome: string | undefined; server: string }) {
  const key = outcome === undefined ? undefined : OUTCOMES[outcome];
  if (key === undefined) {
    return null;
  }
  const done = outcome === "done";
  return (
    <Callout tone={done ? "success" : "danger"} icon={done ? "circle-check" : "circle-alert"} role={done ? "status" : "alert"}
      title={t(key, { server })} />
  );
}
