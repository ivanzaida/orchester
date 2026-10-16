import { Icon } from "./icon";
import styles from "./steps.module.css";

export interface Step {
  readonly label: string;
  readonly state: "done" | "current" | "next";
}

/** Numbered steps joined by lines (`MkJiU` done, `oFevv` current, `MhTGv` next), like the setup's three. */
export function Steps({ label, steps }: { label: string; steps: readonly Step[] }) {
  return (
    <ol className={styles.steps} aria-label={label}>
      {steps.map((step, index) => (
        <li key={step.label} className={`${styles.step} ${styles[step.state]}`} aria-current={step.state === "current" ? "step" : undefined}>
          {index > 0 && <span className={styles.line} aria-hidden="true" />}
          <span className={styles.dot} aria-hidden="true">
            {step.state === "done" ? <Icon name="check" size="step" /> : index + 1}
          </span>
          <span className={styles.label}>{step.label}</span>
        </li>
      ))}
    </ol>
  );
}
