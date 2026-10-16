import styles from "./avatar.module.css";

/** A person's initials in a round tile (`x5rV0`): 28 in a table row, 30 in a header or a card. Raw data, not words. */
export function Avatar({ initials, size }: { initials: string; size: "row" | "header" }) {
  return (
    <span className={`${styles.avatar} ${styles[size]}`} aria-hidden="true">
      {initials}
    </span>
  );
}
