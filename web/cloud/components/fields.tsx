"use client";

import type { ComponentProps } from "react";

import * as kit from "@orchester/ui/components/field";
import { ErrorLine as KitErrorLine } from "@orchester/ui/components/status";

import { t, type Message } from "@/lib/i18n";

import { Say } from "./say";

/** The UI kit's fields and error line with the Cloud's words: errors are `Message`s, the eye's labels come from here. */
type WithMessage<P> = Omit<P, "error"> & { error?: Message | undefined };

function said(error: Message | undefined) {
  return error === undefined ? undefined : <Say message={error} />;
}

export function TextField(props: WithMessage<ComponentProps<typeof kit.TextField>>) {
  return <kit.TextField {...props} error={said(props.error)} />;
}

export function PasswordField(props: WithMessage<Omit<ComponentProps<typeof kit.PasswordField>, "revealLabel" | "concealLabel">>) {
  return (
    <kit.PasswordField
      {...props}
      error={said(props.error)}
      revealLabel={t("field.showPassword")}
      concealLabel={t("field.hidePassword")}
    />
  );
}

export function ErrorLine({ message, id }: { message: Message; id?: string }) {
  return (
    <KitErrorLine id={id}>
      <Say message={message} />
    </KitErrorLine>
  );
}

export { AsideLink } from "@orchester/ui/components/field";
