import { useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { info, error as logError } from "@tauri-apps/plugin-log";
import type { TopStream, GetTopStreamsResponse } from "../types";

interface UseTopStreamsReturn {
  topStreams: TopStream[];
  isLoading: boolean;
  loadTopStreams: (isRefresh?: boolean) => Promise<void>;
}

export function useTopStreams(): UseTopStreamsReturn {
  const [topStreams, setTopStreams] = useState<TopStream[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const loadTopStreams = useCallback(async (isRefresh?: boolean) => {
    info("[useTopStreams] Loading top streams...");
    if (!isRefresh) {
      setIsLoading(true);
    }
    
    try {
      const data = await invoke<GetTopStreamsResponse>("get_top_streams", { limit: 30 });
      
      if (data?.streams?.edges) {
        const streams = data.streams.edges.map((e) => e.node);
        info(`[useTopStreams] Loaded ${streams.length} top streams`);
        setTopStreams(streams);
      } else {
        logError(`[useTopStreams] Failed to load - unexpected format: ${JSON.stringify(data)}`);
      }
    } catch (err) {
      logError(`[useTopStreams] Failed to load: ${err}`);
    } finally {
      if (!isRefresh) {
        setIsLoading(false);
      }
    }
  }, []);

  return {
    topStreams,
    isLoading,
    loadTopStreams,
  };
}
