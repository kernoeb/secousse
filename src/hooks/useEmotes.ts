import { useState, useEffect, useMemo, useCallback } from "react";
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

interface UseEmotesReturn {
  allEmotes: Map<string, string>;
  globalBadges: TwitchBadge[];
  channelBadges: TwitchBadge[];
  loadChannelEmotes: (channelId: string) => Promise<void>;
}

export function useEmotes(): UseEmotesReturn {
  // Global emotes (loaded once)
  const [globalEmotes, setGlobalEmotes] = useState<Map<string, string>>(new Map());
  const [twitchGlobalEmotes, setTwitchGlobalEmotes] = useState<Map<string, string>>(new Map());
  const [globalBadges, setGlobalBadges] = useState<TwitchBadge[]>([]);
  
  // Channel-specific emotes
  const [channelEmotes, setChannelEmotes] = useState<Map<string, string>>(new Map());
  const [twitchChannelEmotes, setTwitchChannelEmotes] = useState<Map<string, string>>(new Map());
  const [channelBadges, setChannelBadges] = useState<TwitchBadge[]>([]);

  // Load global emotes on mount
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
    try {
      const [emoteList, badges, twitchEmotes] = await Promise.all([
        invoke<Emote[]>("get_channel_emotes", { channelId }),
        invoke<GetChannelBadgesResponse>("get_channel_badges", { channelId }),
        invoke<GetTwitchEmotesResponse>("get_twitch_channel_emotes", { channelId })
      ]);

      // 7TV/BTTV/FFZ channel emotes
      const emoteMap = new Map<string, string>();
      emoteList.forEach(e => emoteMap.set(e.name, e.url));
      setChannelEmotes(emoteMap);
      setChannelBadges(badges.user.broadcastBadges);

      // Twitch channel emotes (subscriber emotes)
      if (twitchEmotes?.data) {
        const twitchEmoteMap = new Map<string, string>();
        twitchEmotes.data.forEach((e: TwitchEmote) => {
          const url = e.images?.url_2x || e.images?.url_1x;
          if (e.name && url) {
            twitchEmoteMap.set(e.name, url);
          }
        });
        info(`[useEmotes] Loaded ${twitchEmoteMap.size} Twitch channel emotes`);
        setTwitchChannelEmotes(twitchEmoteMap);
      }
    } catch (err) {
      logError(`[useEmotes] Failed to load channel emotes: ${err}`);
    }
  }, []);

  // Combine all emote sources
  const allEmotes = useMemo(() => {
    const combined = new Map(twitchGlobalEmotes);
    globalEmotes.forEach((v, k) => combined.set(k, v));
    twitchChannelEmotes.forEach((v, k) => combined.set(k, v));
    channelEmotes.forEach((v, k) => combined.set(k, v));
    return combined;
  }, [twitchGlobalEmotes, globalEmotes, twitchChannelEmotes, channelEmotes]);

  return {
    allEmotes,
    globalBadges,
    channelBadges,
    loadChannelEmotes,
  };
}
