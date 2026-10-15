"use client";

import { useEffect, useState } from "react";

const TICK_MS = 1000;

/**
 * Milliseconds left of a wait the server measured (`ms` when the page was rendered), counted down by the browser's
 * own clock from the moment the page appears, so a clock that differs from the server's does not matter.
 */
export function useRemaining(ms: number): number {
  const [left, setLeft] = useState(ms);
  const [measured, setMeasured] = useState(ms);
  // A new wait (a new answer from the server) shows at once, not only at the next tick.
  if (measured !== ms) {
    setMeasured(ms);
    setLeft(ms);
  }
  useEffect(() => {
    if (ms <= 0) {
      return undefined;
    }
    const started = Date.now();
    const timer = setInterval(() => {
      const remaining = Math.max(0, ms - (Date.now() - started));
      setLeft(remaining);
      if (remaining === 0) {
        clearInterval(timer);
      }
    }, TICK_MS);
    return () => { clearInterval(timer); };
  }, [ms]);
  return left;
}
