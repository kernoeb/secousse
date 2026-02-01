import { useState, useCallback, useRef } from "react";
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
  const isLoadingRef = useRef(false);

  const loadTopStreams = useCallback(async (isRefresh?: boolean) => {
    // Guard against concurrent loading
    if (isLoadingRef.current) {
      return;
    }
    
    isLoadingRef.current = true;
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
      isLoadingRef.current = false;
    }
  }, []);

  return {
    topStreams,
    isLoading,
    loadTopStreams,
  };
}
