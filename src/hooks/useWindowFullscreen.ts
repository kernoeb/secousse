import { useCallback, useEffect, useState } from "react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { error as logError } from "@tauri-apps/plugin-log";

export function useWindowFullscreen(): [boolean, (next: boolean) => void] {
  const [isFullscreen, setIsFullscreenState] = useState(false);

  const setIsFullscreen = useCallback((next: boolean) => {
    setIsFullscreenState(next);
    getCurrentWebviewWindow()
      .setFullscreen(next)
      .catch((err) => logError(`[useWindowFullscreen] setFullscreen failed: ${err}`));
  }, []);

  // Sync state back when the user exits fullscreen via OS UI
  // (macOS green button, F11 on Windows, system shortcuts).
  // onResized fires on every drag-resize tick; debounce the IPC roundtrip.
  useEffect(() => {
    const win = getCurrentWebviewWindow();
    let cancelled = false;
    let unlisten: (() => void) | null = null;
    let pollTimer: ReturnType<typeof setTimeout> | null = null;

    const syncFromNative = () => {
      win.isFullscreen().then((native) => {
        if (cancelled) return;
        setIsFullscreenState((prev) => (prev === native ? prev : native));
      }).catch(() => {});
    };

    syncFromNative();

    win
      .onResized(() => {
        if (pollTimer) clearTimeout(pollTimer);
        pollTimer = setTimeout(syncFromNative, 150);
      })
      .then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      })
      .catch((err) => logError(`[useWindowFullscreen] onResized failed: ${err}`));

    return () => {
      cancelled = true;
      if (pollTimer) clearTimeout(pollTimer);
      unlisten?.();
    };
  }, []);

  return [isFullscreen, setIsFullscreen];
}
