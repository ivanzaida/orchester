import styles from "./checkbox.module.css";

/** The file's checkbox (`q3JN1`): the browser's own input in the design's box, with its label. */
export function Checkbox(props: { name: string; label: string; checked: boolean; onChange: (checked: boolean) => void }) {
  return (
    <label className={styles.checkbox}>
      <input
        className={styles.input}
        type="checkbox"
        name={props.name}
        checked={props.checked}
        onChange={(event) => { props.onChange(event.target.checked); }}
      />
      <span>{props.label}</span>
    </label>
  );
}
