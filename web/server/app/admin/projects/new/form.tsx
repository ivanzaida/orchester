"use client";

import { useState } from "react";

import { ButtonLink } from "@orchester/ui/components/button";
import { Callout } from "@orchester/ui/components/callout";
import { CopyButton } from "@orchester/ui/components/copy";
import { Submit } from "@orchester/ui/components/submit";
import { useCheckedForm } from "@orchester/ui/hooks/use-form";

import { ErrorLine, TextField } from "@/components/fields";
import { CsrfField, Hidden } from "@/components/hidden";
import { t } from "@/lib/i18n";
import { slugFromName } from "@/lib/validation";

import { newProjectAction, type NewProjectState } from "./actions";
import styles from "./new.module.css";

const EMPTY = { name: "", slug: "", hint: "" };

/** *Kontur is created* (`KagFB`): its address, *Invite people* and *Copy address*. */
function Created({ state }: { state: Extract<NewProjectState, { view: "created" }> }) {
  return (
    <div className={styles.created}>
      <Callout tone="success" icon="circle-check" title={t("newProject.createdTitle", { name: state.name })}
        body={t("newProject.createdText", { address: state.address })} />
      <div className={styles.buttons}>
        <ButtonLink kind="primary" row icon="user-plus" href={`/admin/projects/${encodeURIComponent(state.slug)}/invites`}>
          {t("project.invitePeople")}
        </ButtonLink>
        <CopyButton value={state.address} label={t("common.copyAddress")} copiedLabel={t("common.copied")} />
      </div>
    </div>
  );
}

/**
 * New project (`gv7xE`): the slug follows the name until it is edited, and shows the address it makes. A form sent
 * twice makes one project: the idempotency key came with the page.
 */
export function NewProjectForm(props: { csrf: string; idempotencyKey: string; prefix: string; owner: string }) {
  const start: NewProjectState = { view: "form", answer: 0, values: EMPTY, errors: {} };
  const { state, pending, form } = useCheckedForm(newProjectAction, start);
  const [values, setValues] = useState(EMPTY);
  const [slugTouched, setSlugTouched] = useState(false);
  if (state.view === "created") {
    return <Created state={state} />;
  }
  const setName = (name: string) => {
    setValues({ ...values, name, slug: slugTouched ? values.slug : slugFromName(name) });
  };
  return (
    <form className={styles.card} {...form} noValidate>
      <CsrfField token={props.csrf} />
      <Hidden name="key" value={props.idempotencyKey} />
      <TextField name="name" type="text" size="row" label={t("newProject.name")} value={values.name} onChange={setName}
        hint={t("newProject.nameHint")} error={state.errors.name} autoComplete="off" />
      <TextField name="slug" type="text" size="row" label={t("newProject.slug")} value={values.slug} prefix={props.prefix} mono
        onChange={(slug) => { setSlugTouched(true); setValues({ ...values, slug }); }} hint={t("newProject.slugHint")}
        error={state.errors.slug} autoComplete="off" />
      <TextField name="hint" type="text" size="row" label={t("newProject.hint")} value={values.hint}
        onChange={(hint) => { setValues({ ...values, hint }); }} hint={t("newProject.hintHelp")} error={state.errors.hint} autoComplete="off" />
      <div className={styles.owner}>
        <span className={styles.ownerLabel}>{t("newProject.owner")}</span>
        <span className={styles.ownerValue}>{t("newProject.ownerValue", { name: props.owner })}</span>
      </div>
      {state.error !== undefined && <ErrorLine message={state.error} />}
      <div className={styles.footer}>
        <ButtonLink kind="ghost" row href="/admin">
          {t("common.cancel")}
        </ButtonLink>
        <Submit kind="primary" row icon="plus" label={t("newProject.create")} busyLabel={t("newProject.creating")} pending={pending} />
      </div>
    </form>
  );
}
