import styles from "./switch.module.css";

/**
 * The file's switch (`XC5zU` on, `H6LlDD` off) as a form's submit button: pressing it sends the form that flips the
 * setting, so it works without script. Its words say the state beside it (*On*, *Accepted*).
 */
export function SwitchButton(props: { on: boolean; label: string; state: string; busy?: boolean; disabled?: boolean }) {
  return (
    <button
      type="submit"
      role="switch"
      aria-checked={props.on}
      aria-label={props.label}
      className={`${styles.line}${props.busy ? ` ${styles.busy}` : ""}`}
      disabled={props.disabled ?? props.busy ?? false}
    >
      <span className={`${styles.track}${props.on ? ` ${styles.on}` : ""}`} aria-hidden="true">
        <span className={styles.knob} />
      </span>
      <span className={styles.state}>{props.state}</span>
    </button>
  );
}
