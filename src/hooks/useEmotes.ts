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
const RETRY_DELAY_MS = 10_000;
const MAX_RETRIES = 3;

type EmoteSource = "emotes" | "badges" | "twitch";

interface SourceResult<T> {
  data: T | null;
  retryable: boolean;
}

// One failing source must not discard the ones that loaded. A 401 means "not
// logged in", which no amount of retrying fixes.
async function invokeChannel<T>(command: string, channelId: string): Promise<SourceResult<T>> {
  try {
    return { data: await invoke<T>(command, { channelId }), retryable: false };
  } catch (err) {
    const message = String(err);
    logError(`[useEmotes] ${command} failed for ${channelId}: ${message}`);
    return { data: null, retryable: !message.includes("401") };
  }
}

interface ChannelEmoteEntry {
  thirdParty: Map<string, string>;
  twitch: Map<string, string>;
  badges: TwitchBadge[];
  // Sources that failed and are worth another try.
  pending: Set<EmoteSource>;
}

type FetchChannel = (channelId: string, focus: boolean, force: boolean, attempt: number) => Promise<void>;

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
  const retryTimersRef = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map());
  const unmountedRef = useRef(false);

  useEffect(() => {
    unmountedRef.current = false;
    return () => {
      unmountedRef.current = true;
      retryTimersRef.current.forEach(clearTimeout);
      retryTimersRef.current.clear();
    };
  }, []);

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

  const fetchChannel = useCallback<FetchChannel>(async (channelId, focus, force, attempt) => {
    const cache = channelCacheRef.current;

    if (!force && cache.has(channelId)) {
      const entry = cache.get(channelId)!;
      cache.delete(channelId);
      cache.set(channelId, entry);
      if (focus) setFocusedChannelIdInternal(channelId);
      return;
    }

    const existing = inflightRef.current.get(channelId);
    if (existing) {
      return existing;
    }

    // A forced run is a retry: refetch only what failed, keep what loaded.
    const previous = force ? cache.get(channelId) : undefined;
    const load = <T,>(source: EmoteSource, command: string): Promise<SourceResult<T>> =>
      !previous || previous.pending.has(source)
        ? invokeChannel<T>(command, channelId)
        : Promise.resolve({ data: null, retryable: false });

    const promise = (async () => {
      try {
        const [emotes, badges, twitchEmotes] = await Promise.all([
          load<Emote[]>("emotes", "get_channel_emotes"),
          load<GetChannelBadgesResponse>("badges", "get_channel_badges"),
          load<GetTwitchEmotesResponse>("twitch", "get_twitch_channel_emotes")
        ]);

        const thirdParty = emotes.data
          ? new Map(emotes.data.map((e): [string, string] => [e.name, e.url]))
          : previous?.thirdParty ?? new Map<string, string>();

        let twitch = previous?.twitch ?? new Map<string, string>();
        if (twitchEmotes.data?.data) {
          twitch = new Map();
          twitchEmotes.data.data.forEach((e: TwitchEmote) => {
            const url = e.images?.url_2x || e.images?.url_1x;
            if (e.name && url) {
              twitch.set(e.name, url);
            }
          });
          info(`[useEmotes] Loaded ${twitch.size} Twitch channel emotes for ${channelId}`);
        }

        const pending = new Set<EmoteSource>();
        if (emotes.retryable) pending.add("emotes");
        if (badges.retryable) pending.add("badges");
        if (twitchEmotes.retryable) pending.add("twitch");

        const entry: ChannelEmoteEntry = {
          thirdParty,
          twitch,
          badges: badges.data ? badges.data.user?.broadcastBadges ?? [] : previous?.badges ?? [],
          pending,
        };

        if (cache.size >= CHANNEL_CACHE_MAX && !cache.has(channelId)) {
          const oldest = cache.keys().next().value;
          if (oldest !== undefined) cache.delete(oldest);
        }
        cache.set(channelId, entry);
        if (focus) setFocusedChannelIdInternal(channelId);
        setCacheRevision((r) => r + 1);

        // Twitch answers a degraded source with nothing. Refetch it in the
        // background so the channel is not stuck half-loaded until you leave it.
        const shouldRetry = pending.size > 0
          && attempt < MAX_RETRIES
          && !unmountedRef.current
          && !retryTimersRef.current.has(channelId);
        if (shouldRetry) {
          info(`[useEmotes] Partial load for ${channelId}, retrying in ${RETRY_DELAY_MS / 1000}s`);
          const timer = setTimeout(() => {
            retryTimersRef.current.delete(channelId);
            // Evicted meanwhile: nobody is watching this channel any more.
            if (!cache.has(channelId)) return;
            // Never refocus: the user may have moved on since the failure.
            fetchChannel(channelId, false, true, attempt + 1);
          }, RETRY_DELAY_MS);
          retryTimersRef.current.set(channelId, timer);
        }
      } catch (err) {
        logError(`[useEmotes] Failed to load channel emotes for ${channelId}: ${err}`);
      } finally {
        inflightRef.current.delete(channelId);
      }
    })();

    inflightRef.current.set(channelId, promise);
    return promise;
  }, []);

  const loadChannelEmotes = useCallback(
    (channelId: string) => fetchChannel(channelId, true, false, 0),
    [fetchChannel]
  );

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
