import { useEffect, useRef, useState } from "react";

export function useIdleTimer(delayMs: number) {
  const [isActive, setIsActive] = useState(false);
  const timerRef = useRef<number | null>(null);

  const clear = () => {
    if (timerRef.current !== null) {
      window.clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  };

  const markActive = () => {
    setIsActive((prev) => (prev ? prev : true));
    clear();
    timerRef.current = window.setTimeout(() => {
      setIsActive(false);
      timerRef.current = null;
    }, delayMs);
  };

  const reset = () => {
    clear();
    setIsActive(false);
  };

  useEffect(() => clear, []);

  return { isActive, markActive, reset };
}
