import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { info, error as logError } from "@tauri-apps/plugin-log";
import type { SelfInfo, GetSelfInfoResponse, GetFollowedChannelsResponse, UserInfo } from "../types";

interface UseAuthReturn {
  isLoggedIn: boolean;
  selfInfo: SelfInfo | null;
  followedChannels: UserInfo[];
  isLoadingFollowed: boolean;
  login: () => Promise<void>;
  logout: () => Promise<void>;
  refreshFollowedChannels: (isRefresh?: boolean) => Promise<void>;
}

export function useAuth(): UseAuthReturn {
  const [isLoggedIn, setIsLoggedIn] = useState(false);
  const [selfInfo, setSelfInfo] = useState<SelfInfo | null>(null);
  const [followedChannels, setFollowedChannels] = useState<UserInfo[]>([]);
  const [isLoadingFollowed, setIsLoadingFollowed] = useState(true);

  const refreshFollowedChannels = useCallback(async (isRefresh?: boolean) => {
    if (!isRefresh) {
      setIsLoadingFollowed(true);
    }
    try {
      if (selfInfo?.id) {
        const data = await invoke<GetFollowedChannelsResponse>("get_followed_channels", { userId: selfInfo.id });
        if (data?.user?.followedLiveUsers) {
          const channels = data.user.followedLiveUsers.edges.map((e) => e.node);
          setFollowedChannels(channels);
        }
      } else {
        setFollowedChannels([]);
      }
    } catch (err) {
      logError(`[useAuth] Failed to load followed channels: ${err}`);
      setFollowedChannels([]);
    } finally {
      if (!isRefresh) {
        setIsLoadingFollowed(false);
      }
    }
  }, [selfInfo?.id]);

  const checkLoginStatus = useCallback(async () => {
    try {
      const loggedIn = await invoke<boolean>("is_logged_in");
      if (loggedIn) {
        setIsLoggedIn(true);
        const data = await invoke<GetSelfInfoResponse>("get_self_info");
        setSelfInfo(data.viewer);
      } else {
        setIsLoggedIn(false);
        setFollowedChannels([]);
        setIsLoadingFollowed(false);
      }
    } catch (err) {
      logError(`[useAuth] Failed to check login status: ${err}`);
      setIsLoadingFollowed(false);
    }
  }, []);

  const login = useCallback(async () => {
    try {
      await invoke("login");
    } catch (err) {
      logError(`[useAuth] Login error: ${err}`);
    }
  }, []);

  const logout = useCallback(async () => {
    try {
      await invoke("logout");
      setIsLoggedIn(false);
      setSelfInfo(null);
      setFollowedChannels([]);
    } catch (err) {
      logError(`[useAuth] Logout error: ${err}`);
    }
  }, []);

  // Check login status on mount
  useEffect(() => {
    checkLoginStatus();
  }, [checkLoginStatus]);

  // Load followed channels when selfInfo changes
  useEffect(() => {
    if (selfInfo?.id) {
      refreshFollowedChannels();
    }
  }, [selfInfo?.id, refreshFollowedChannels]);

  // Listen for login success events
  useEffect(() => {
    const unlisten = listen<string>("login-success", async () => {
      info("[useAuth] Login Success!");
      setIsLoggedIn(true);
      try {
        const data = await invoke<GetSelfInfoResponse>("get_self_info");
        setSelfInfo(data.viewer);
      } catch (err) {
        logError(`[useAuth] Failed to get self info after login: ${err}`);
      }
    });
    return () => { unlisten.then(f => f()); };
  }, []);

  return {
    isLoggedIn,
    selfInfo,
    followedChannels,
    isLoadingFollowed,
    login,
    logout,
    refreshFollowedChannels,
  };
}
