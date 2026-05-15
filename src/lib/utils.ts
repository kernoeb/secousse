import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

/** HLS.js sentinel value for "let ABR pick the level" — surfaces as our Auto option. */
export const AUTO_QUALITY = -1;

/** Merge Tailwind classes with clsx */
export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

/** Format viewer count with K/M suffixes */
export function formatViewers(count: number): string {
  if (count >= 1000000) return (count / 1000000).toFixed(1) + "M";
  if (count >= 1000) return (count / 1000).toFixed(1) + "K";
  return count.toString();
}

/** Read+parse a localStorage entry, falling back when missing or unparseable. */
function readPersisted<T>(key: string, fallback: T, parse: (raw: string) => T | undefined): T {
  if (typeof window === "undefined") return fallback;
  const raw = localStorage.getItem(key);
  if (raw === null) return fallback;
  const v = parse(raw);
  return v === undefined ? fallback : v;
}

const parseBool = (r: string) => (r === "true" ? true : r === "false" ? false : undefined);

export function getInitialChannel(): string | null {
  if (typeof window === "undefined") return null;
  return sessionStorage.getItem("currentChannel") || localStorage.getItem("lastChannel") || null;
}

export function persistChannel(channel: string | null) {
  if (channel) {
    sessionStorage.setItem("currentChannel", channel);
    localStorage.setItem("lastChannel", channel);
  } else {
    sessionStorage.removeItem("currentChannel");
  }
}

export function getInitialActiveTab(): "following" | "browse" {
  return readPersisted("activeTab", "browse", (r) =>
    r === "following" || r === "browse" ? r : undefined,
  );
}

export function persistActiveTab(tab: "following" | "browse") {
  localStorage.setItem("activeTab", tab);
}

export function getInitialSidebarOpen(): boolean {
  return readPersisted("sidebarOpen", true, parseBool);
}

export function persistSidebarOpen(open: boolean) {
  localStorage.setItem("sidebarOpen", String(open));
}

export function getInitialChatOpen(): boolean {
  return readPersisted("chatOpen", true, parseBool);
}

export function persistChatOpen(open: boolean) {
  localStorage.setItem("chatOpen", String(open));
}

export function getInitialVolume(): number {
  return readPersisted("playerVolume", 1, (r) => {
    const n = parseFloat(r);
    return !Number.isNaN(n) && n >= 0 && n <= 1 ? n : undefined;
  });
}

export function persistVolume(volume: number) {
  localStorage.setItem("playerVolume", String(volume));
}

export function getInitialMuted(): boolean {
  return readPersisted("playerMuted", false, parseBool);
}

export function persistMuted(muted: boolean) {
  localStorage.setItem("playerMuted", String(muted));
}

/** Returns AUTO_QUALITY when no preference is set or the saved value is invalid. */
export function getInitialPreferredQualityHeight(): number {
  return readPersisted("preferredQualityHeight", AUTO_QUALITY, (r) => {
    const n = parseInt(r, 10);
    return Number.isNaN(n) ? undefined : n;
  });
}

export function persistPreferredQualityHeight(height: number) {
  localStorage.setItem("preferredQualityHeight", String(height));
}

export const GRID_MAX_TILES = 4;

/** Read the grid layout from localStorage. Empty array if absent or invalid. */
export function getInitialGridChannels(): string[] {
  return readPersisted<string[]>("gridChannels", [], (r) => {
    try {
      const parsed = JSON.parse(r);
      if (!Array.isArray(parsed)) return undefined;
      const filtered = parsed.filter((v): v is string => typeof v === "string");
      return filtered.slice(0, GRID_MAX_TILES);
    } catch {
      return undefined;
    }
  });
}

export function persistGridChannels(channels: string[]) {
  if (channels.length === 0) {
    localStorage.removeItem("gridChannels");
  } else {
    localStorage.setItem("gridChannels", JSON.stringify(channels.slice(0, GRID_MAX_TILES)));
  }
}

export function getInitialGridOrLegacyChannel(): string[] {
  const grid = getInitialGridChannels();
  if (grid.length > 0) return grid;
  const last = getInitialChannel();
  return last ? [last] : [];
}

export function getInitialFocusedIndex(): number {
  return readPersisted("focusedIndex", 0, (r) => {
    const n = parseInt(r, 10);
    return Number.isNaN(n) || n < 0 ? undefined : n;
  });
}

export function persistFocusedIndex(idx: number) {
  localStorage.setItem("focusedIndex", String(idx));
}

export const POPOUT_QUERY_PARAM = "popout";

export function getPopoutChannel(): string | null {
  if (typeof window === "undefined") return null;
  const params = new URLSearchParams(window.location.search);
  const c = params.get(POPOUT_QUERY_PARAM);
  return c && c.length > 0 ? c : null;
}

/** Build a Twitch emote CDN URL from an emote ID. */
export function twitchEmoteUrl(id: string, size: "1.0" | "2.0" | "3.0" = "2.0"): string {
  return `https://static-cdn.jtvnw.net/emoticons/v2/${id}/default/dark/${size}`;
}
