"use client";

import type { ComponentProps } from "react";

import * as kit from "@orchester/ui/components/field";
import { ErrorLine as KitErrorLine } from "@orchester/ui/components/status";

import type { Message } from "@/lib/i18n";

import { Say } from "./say";

/** The UI kit's field and error line with the admin's words: errors are `Message`s. */
type WithMessage<P> = Omit<P, "error"> & { error?: Message | undefined };

export function TextField(props: WithMessage<ComponentProps<typeof kit.TextField>>) {
  return <kit.TextField {...props} error={props.error === undefined ? undefined : <Say message={props.error} />} />;
}

export function ErrorLine({ message, id }: { message: Message; id?: string }) {
  return (
    <KitErrorLine id={id}>
      <Say message={message} />
    </KitErrorLine>
  );
}
