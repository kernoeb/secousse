import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { error as logError } from "@tauri-apps/plugin-log";
import type { UserInfo, GetUserInfoResponse } from "../types";

interface UseUserInfoOptions {
  refreshMs?: number;
  skip?: boolean;
}

export function useUserInfo(channel: string | null, options: UseUserInfoOptions = {}): { userInfo: UserInfo | null } {
  const { refreshMs = 60_000, skip = false } = options;
  const [userInfo, setUserInfo] = useState<UserInfo | null>(null);

  useEffect(() => {
    if (!channel || skip) return;
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let failures = 0;

    const fetchInfo = async () => {
      let delay = refreshMs;
      try {
        const data = await invoke<GetUserInfoResponse>("get_user_info", { login: channel });
        failures = 0;
        if (!cancelled) setUserInfo(data.user);
      } catch (err) {
        logError(`[useUserInfo] get_user_info failed for ${channel}: ${err}`);
        // Until it succeeds the player cannot tell live from offline, so retry soon.
        delay = Math.min(refreshMs, 2_000 * 2 ** failures++);
      }
      if (!cancelled) timer = setTimeout(fetchInfo, delay);
    };

    fetchInfo();
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [channel, skip, refreshMs]);

  return { userInfo };
}
