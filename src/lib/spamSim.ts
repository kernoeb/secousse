import { emit } from "@tauri-apps/api/event";
import type { ChatMessage } from "../types";

export type SpamMode = "emotes" | "long" | "mixed";

export interface SpamOptions {
  rate?: number;
  emotesPerMsg?: number;
  durationSec?: number;
  mode?: SpamMode;
}

export interface SpamApi {
  start: (opts?: SpamOptions) => void;
  stop: () => void;
}

declare global {
  interface Window {
    __spam?: SpamApi;
  }
}

const COLORS = ["#ff8280", "#7cf6ff", "#a970ff", "#ff8a3b", "#0ed463", "#ff5e5e", "#9aff5b", "#ff77c6"];
const USERS = ["Kappa42", "PogChamp_99", "DansGame", "monkaW", "FeelsGoodMan", "OMEGALUL", "WeirdChamp", "PepeLaugh", "Sadge", "NotLikeThis"];
const SHORT_PHRASES = ["lol", "wp", "gg", "no way", "bro", "wait what", "lmao", "ez", "sheeesh", "omg"];
const LONG_PHRASES = [
  "Bro this is actually one of the most insane things I have ever witnessed live on this stream and I have been watching for years",
  "POV you just walked in on the most chaotic moment in the entire stream history and you have absolutely no idea what is going on",
  "I cannot believe what just happened here this has to be the play of the season if not the entire decade let me tell you",
  "Wait wait wait did everyone else just see that or am I completely losing my mind because that did not look real at all",
  "Ok so hear me out because I have been thinking about this for the last twenty minutes and it actually makes a lot of sense if you consider the context",
  "The way this whole situation is unfolding right now is exactly why I never miss a single stream from this channel honestly",
];

let timer: ReturnType<typeof setInterval> | null = null;
let stopAt: number | null = null;

const pick = <T>(arr: T[]): T => arr[Math.floor(Math.random() * arr.length)];
const pickN = <T>(arr: T[], n: number): T[] => Array.from({ length: n }, () => pick(arr));

function buildMessage(mode: SpamMode, emoteNames: string[], emotesPerMsg: number): string {
  const concrete: Exclude<SpamMode, "mixed"> | "shortText" | "longWithEmotes" =
    mode === "mixed"
      ? pick(["emotes", "shortText", "long", "longWithEmotes"] as const)
      : mode;

  switch (concrete) {
    case "emotes":
      return pickN(emoteNames, emotesPerMsg).join(" ");
    case "shortText":
      return pick(SHORT_PHRASES);
    case "long":
      return pick(LONG_PHRASES);
    case "longWithEmotes": {
      const phrase = pick(LONG_PHRASES);
      const emotes = pickN(emoteNames, Math.max(1, Math.floor(emotesPerMsg / 2))).join(" ");
      // Sprinkle emotes at start and end so wrap behavior is exercised.
      return `${pickN(emoteNames, 2).join(" ")} ${phrase} ${emotes}`;
    }
  }
}

export function startSpam(channel: string, emoteNames: string[], opts: SpamOptions = {}) {
  if (timer !== null) stopSpam();
  const mode: SpamMode = opts.mode ?? "emotes";
  if (emoteNames.length === 0 && mode !== "long") {
    console.warn("[spamSim] no emotes loaded — load a channel first or use mode='long'");
    return;
  }

  const rate = opts.rate ?? 50;
  const emotesPerMsg = opts.emotesPerMsg ?? 8;
  const intervalMs = Math.max(1, 1000 / rate);
  stopAt = opts.durationSec ? Date.now() + opts.durationSec * 1000 : null;

  let n = 0;
  timer = setInterval(() => {
    if (stopAt !== null && Date.now() >= stopAt) {
      stopSpam();
      return;
    }
    const msg: ChatMessage = {
      id: `sim-${Date.now()}-${n++}`,
      user: pick(USERS),
      message: buildMessage(mode, emoteNames, emotesPerMsg),
      color: pick(COLORS),
      badges: [],
      emotes: [],
      timestamp: Date.now(),
      channel,
    };
    emit("chat-message", msg);
  }, intervalMs);

  console.log(`[spamSim] started: ${rate}/s mode=${mode} on #${channel}${opts.durationSec ? ` for ${opts.durationSec}s` : ""}`);
}

export function stopSpam() {
  if (timer !== null) {
    clearInterval(timer);
    timer = null;
    stopAt = null;
    console.log("[spamSim] stopped");
  }
}

if (import.meta.hot) {
  import.meta.hot.dispose(() => stopSpam());
}
