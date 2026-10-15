"use client";

import { useTimeZone } from "@orchester/ui/hooks/browser";

import { INVITE_LIFETIME_MS } from "@/lib/account";
import { clockTime, sameDay, shortDate } from "@/lib/dates";
import { t } from "@/lib/i18n";

/**
 * The Account page's dates in the reader's time zone (*Oct 13*, *Today, 09:12*, *Last active today, 14:02*). `now` is
 * the server's time when the page was made, so server and browser agree on what *today* is.
 */
function useWhen() {
  const zone = useTimeZone();
  return {
    date: (at: number) => shortDate(at, zone),
    time: (at: number) => clockTime(at, zone),
    today: (at: number, now: number) => sameDay(at, now, zone),
  };
}

export function LastActive({ at, now, current }: { at: number; now: number; current: boolean }) {
  const when = useWhen();
  if (current) {
    return <>{t("account.devices.activeNow")}</>;
  }
  return (
    <>
      {when.today(at, now)
        ? t("account.devices.lastActiveToday", { time: when.time(at) })
        : t("account.devices.lastActive", { date: when.date(at) })}
    </>
  );
}

/** An invite's *sent · expires* line; it was sent its lifetime before it expires. */
export function InviteDates({ expiresAt, now }: { expiresAt: number; now: number }) {
  const when = useWhen();
  const sentAt = expiresAt - INVITE_LIFETIME_MS;
  const sent = when.today(sentAt, now) ? t("account.today", { time: when.time(sentAt) }) : when.date(sentAt);
  return <>{t("account.invites.when", { sent, expires: when.date(expiresAt) })}</>;
}

export function Since({ at, owner }: { at: number; owner: boolean }) {
  const date = useWhen().date(at);
  return <>{owner ? t("account.servers.connected", { date }) : t("account.servers.joined", { date })}</>;
}
