"use client";

import { useEffect, useRef } from "react";

import { buttonClass, ButtonContent } from "@orchester/ui/components/button";

import { t } from "@/lib/i18n";

import { Hidden } from "./hidden";

/**
 * Posts a trip to Orchester Cloud as soon as the page has it (security review 1, B1: the link statement goes in this
 * form's body, never in a URL); without script the person presses *Continue*.
 */
export function Carry({ action, fields }: { action: string; fields: readonly (readonly [string, string])[] }) {
  const form = useRef<HTMLFormElement>(null);
  useEffect(() => {
    form.current?.submit();
  }, []);
  return (
    <form ref={form} method="post" action={action}>
      {fields.map(([name, value]) => (
        <Hidden key={name} name={name} value={value} />
      ))}
      <button type="submit" className={buttonClass("primary")}>
        <ButtonContent icon="cloud">{t("trip.continue")}</ButtonContent>
      </button>
    </form>
  );
}
