"use client";

import { startTransition, useActionState, type SubmitEvent } from "react";

/**
 * A form posted to a server action. With script the form is checked first (`check` answers whether to send) and
 * sent in a transition, so the fields keep their values; without script the browser posts it and the server checks
 * the same rules.
 */
export function useCheckedForm<S>(
  action: (state: Awaited<S>, form: FormData) => Promise<S>,
  initial: Awaited<S>,
  check: (form: FormData) => boolean = () => true,
) {
  const [state, dispatch, pending] = useActionState<S, FormData>(action, initial);
  const onSubmit = (event: SubmitEvent<HTMLFormElement>) => {
    event.preventDefault();
    const form = new FormData(event.currentTarget);
    if (check(form)) {
      startTransition(() => { dispatch(form); });
    }
  };
  return { state, pending, form: { action: dispatch, onSubmit } };
}
