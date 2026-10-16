import { Steps } from "@orchester/ui/components/steps";

import { t } from "@/lib/i18n";

/** The setup's three steps (`vUL7K`): setup code, your account, Orchester Cloud; `at` is the current one, 4 when done. */
export function SetupSteps({ at }: { at: 1 | 2 | 3 | 4 }) {
  const state = (step: number) => (step < at ? "done" : step === at ? "current" : "next");
  return (
    <Steps
      label={t("setup.steps")}
      steps={[
        { label: t("setup.step1"), state: state(1) },
        { label: t("setup.step2"), state: state(2) },
        { label: t("setup.step3"), state: state(3) },
      ]}
    />
  );
}
