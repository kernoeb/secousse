import { useEffect } from "react";
import { info, error as logError } from "@tauri-apps/plugin-log";
import { check as checkUpdate, type Update } from "@tauri-apps/plugin-updater";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

let pendingUpdate: Update | null = null;

async function releaseUpdate(update: Update): Promise<void> {
  try { await update.close(); } catch { /* already released */ }
}

export function useUpdater(): void {
  useEffect(() => {
    if (import.meta.env.DEV) return;

    let disposed = false;
    let unlisten: (() => void) | undefined;

    (async () => {
      let update: Update | null = null;
      try {
        update = await checkUpdate();
        if (!update || disposed) return;
        info(`[useUpdater] Found update ${update.version}, downloading...`);
        await update.download();
        if (disposed) return;
        pendingUpdate = update;
        update = null; // ownership transferred — don't release in finally
        info("[useUpdater] Update downloaded, will install when app closes");
      } catch (err) {
        logError(`[useUpdater] check/download failed: ${err}`);
      } finally {
        if (update) await releaseUpdate(update);
      }
    })();

    (async () => {
      try {
        const win = getCurrentWebviewWindow();
        const fn = await win.onCloseRequested(async (event) => {
          const update = pendingUpdate;
          if (!update) return;
          event.preventDefault();
          pendingUpdate = null;
          try {
            info("[useUpdater] Installing pending update before close");
            await update.install();
          } catch (err) {
            logError(`[useUpdater] install failed: ${err}`);
          }
          await releaseUpdate(update);
          await win.destroy();
        });
        if (disposed) fn();
        else unlisten = fn;
      } catch (err) {
        logError(`[useUpdater] close handler registration failed: ${err}`);
      }
    })();

    return () => {
      disposed = true;
      unlisten?.();
      if (pendingUpdate) {
        const u = pendingUpdate;
        pendingUpdate = null;
        releaseUpdate(u);
      }
    };
  }, []);
}
