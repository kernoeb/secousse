import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { error as logError } from "@tauri-apps/plugin-log";
import type { CollaborationMap } from "../types";

/** Poll the Stream Together sessions of these channels. One request covers every id. */
export function useCollaborations(channelIds: string[], refreshMs = 60_000): CollaborationMap {
  const [collabs, setCollabs] = useState<CollaborationMap>({});
  // Depend on the set of ids, not the array: callers rebuild it on each render, and the
  // followed list is re-sorted by viewers on every refresh.
  const key = [...channelIds].sort().join(",");

  useEffect(() => {
    if (!key) {
      setCollabs({});
      return;
    }
    let cancelled = false;
    const fetchCollabs = async () => {
      try {
        const data = await invoke<CollaborationMap>("get_collaborations", { channelIds: key.split(",") });
        if (!cancelled) setCollabs(data);
      } catch (err) {
        logError(`[useCollaborations] get_collaborations failed: ${err}`);
      }
    };
    fetchCollabs();
    const interval = setInterval(fetchCollabs, refreshMs);
    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, [key, refreshMs]);

  return collabs;
}
