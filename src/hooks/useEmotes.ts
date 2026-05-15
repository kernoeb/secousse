import { useState, useEffect, useMemo, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { info, error as logError } from "@tauri-apps/plugin-log";
import type {
  Emote,
  TwitchBadge,
  TwitchEmote,
  GetGlobalBadgesResponse,
  GetChannelBadgesResponse,
  GetTwitchEmotesResponse
} from "../types";

const CHANNEL_CACHE_MAX = 5;

interface ChannelEmoteEntry {
  thirdParty: Map<string, string>;
  twitch: Map<string, string>;
  badges: TwitchBadge[];
}

interface UseEmotesReturn {
  allEmotes: Map<string, string>;
  globalBadges: TwitchBadge[];
  channelBadges: TwitchBadge[];
  loadChannelEmotes: (channelId: string) => Promise<void>;
  setFocusedChannelId: (channelId: string | null) => void;
}

export function useEmotes(): UseEmotesReturn {
  const [globalEmotes, setGlobalEmotes] = useState<Map<string, string>>(new Map());
  const [twitchGlobalEmotes, setTwitchGlobalEmotes] = useState<Map<string, string>>(new Map());
  const [globalBadges, setGlobalBadges] = useState<TwitchBadge[]>([]);

  const channelCacheRef = useRef<Map<string, ChannelEmoteEntry>>(new Map());
  const inflightRef = useRef<Map<string, Promise<void>>>(new Map());
  const [focusedChannelId, setFocusedChannelIdInternal] = useState<string | null>(null);
  const [cacheRevision, setCacheRevision] = useState(0);

  useEffect(() => {
    loadGlobalEmotes();
    loadTwitchGlobalEmotes();
    loadGlobalBadges();
  }, []);

  async function loadGlobalEmotes() {
    try {
      const emoteList: Emote[] = await invoke("get_global_emotes");
      const emoteMap = new Map<string, string>();
      emoteList.forEach(e => emoteMap.set(e.name, e.url));
      setGlobalEmotes(emoteMap);
    } catch (err) {
      logError(`[useEmotes] Failed to load global emotes: ${err}`);
    }
  }

  async function loadTwitchGlobalEmotes() {
    try {
      const data = await invoke<GetTwitchEmotesResponse>("get_twitch_global_emotes");
      if (data?.data) {
        const emoteMap = new Map<string, string>();
        data.data.forEach((e: TwitchEmote) => {
          const url = e.images?.url_2x || e.images?.url_1x;
          if (e.name && url) {
            emoteMap.set(e.name, url);
          }
        });
        info(`[useEmotes] Loaded ${emoteMap.size} Twitch global emotes`);
        setTwitchGlobalEmotes(emoteMap);
      }
    } catch (err) {
      logError(`[useEmotes] Failed to load Twitch global emotes: ${err}`);
    }
  }

  async function loadGlobalBadges() {
    try {
      const data = await invoke<GetGlobalBadgesResponse>("get_global_badges");
      setGlobalBadges(data.badges);
    } catch (err) {
      logError(`[useEmotes] Failed to load global badges: ${err}`);
    }
  }

  const loadChannelEmotes = useCallback(async (channelId: string) => {
    const cache = channelCacheRef.current;

    if (cache.has(channelId)) {
      const entry = cache.get(channelId)!;
      cache.delete(channelId);
      cache.set(channelId, entry);
      setFocusedChannelIdInternal(channelId);
      return;
    }

    const existing = inflightRef.current.get(channelId);
    if (existing) {
      return existing;
    }

    const promise = (async () => {
      try {
        const [emoteList, badges, twitchEmotes] = await Promise.all([
          invoke<Emote[]>("get_channel_emotes", { channelId }),
          invoke<GetChannelBadgesResponse>("get_channel_badges", { channelId }),
          invoke<GetTwitchEmotesResponse>("get_twitch_channel_emotes", { channelId })
        ]);

        const thirdParty = new Map<string, string>();
        emoteList.forEach(e => thirdParty.set(e.name, e.url));

        const twitch = new Map<string, string>();
        if (twitchEmotes?.data) {
          twitchEmotes.data.forEach((e: TwitchEmote) => {
            const url = e.images?.url_2x || e.images?.url_1x;
            if (e.name && url) {
              twitch.set(e.name, url);
            }
          });
          info(`[useEmotes] Loaded ${twitch.size} Twitch channel emotes for ${channelId}`);
        }

        const entry: ChannelEmoteEntry = {
          thirdParty,
          twitch,
          badges: badges.user.broadcastBadges,
        };

        if (cache.size >= CHANNEL_CACHE_MAX) {
          const oldest = cache.keys().next().value;
          if (oldest !== undefined) cache.delete(oldest);
        }
        cache.set(channelId, entry);
        setFocusedChannelIdInternal(channelId);
        setCacheRevision((r) => r + 1);
      } catch (err) {
        logError(`[useEmotes] Failed to load channel emotes for ${channelId}: ${err}`);
      } finally {
        inflightRef.current.delete(channelId);
      }
    })();

    inflightRef.current.set(channelId, promise);
    return promise;
  }, []);

  const setFocusedChannelId = useCallback((channelId: string | null) => {
    setFocusedChannelIdInternal(channelId);
  }, []);

  const allEmotes = useMemo(() => {
    const combined = new Map(twitchGlobalEmotes);
    globalEmotes.forEach((v, k) => combined.set(k, v));
    if (focusedChannelId) {
      const entry = channelCacheRef.current.get(focusedChannelId);
      if (entry) {
        entry.twitch.forEach((v, k) => combined.set(k, v));
        entry.thirdParty.forEach((v, k) => combined.set(k, v));
      }
    }
    return combined;
  }, [twitchGlobalEmotes, globalEmotes, focusedChannelId, cacheRevision]);

  const channelBadges = useMemo<TwitchBadge[]>(() => {
    if (!focusedChannelId) return [];
    return channelCacheRef.current.get(focusedChannelId)?.badges ?? [];
  }, [focusedChannelId, cacheRevision]);

  return {
    allEmotes,
    globalBadges,
    channelBadges,
    loadChannelEmotes,
    setFocusedChannelId,
  };
}
