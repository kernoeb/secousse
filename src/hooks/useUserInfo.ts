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

    const fetchInfo = async () => {
      try {
        const data = await invoke<GetUserInfoResponse>("get_user_info", { login: channel });
        if (!cancelled) setUserInfo(data.user);
      } catch (err) {
        logError(`[useUserInfo] get_user_info failed for ${channel}: ${err}`);
      }
    };

    fetchInfo();
    const interval = setInterval(fetchInfo, refreshMs);
    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, [channel, skip, refreshMs]);

  return { userInfo };
}
