// Standalone harness reproducing src/components/Chat.tsx's scroll + freeze
// logic with synthetic messages. No Tauri runtime — run `bun run dev` and
// open http://localhost:1420/dev/chat-test/. Drive scenarios from
// chrome-devtools via `window.__test`.
import { createRoot } from "react-dom/client";
import {
  useState,
  memo,
  useRef,
  useEffect,
  useCallback,
} from "react";
import { VList, type VListHandle } from "virtua";

type Token = string | { emote: string; alt: string };

interface TestMessage {
  id: string;
  user: string;
  color: string;
  parts: Token[];
  ts: number;
}

const SCROLL_NOISE_PX = 16;
const CAP = 300;

const USERS = ["Kappa42", "PogChamp_99", "DansGame", "monkaW", "FeelsGoodMan", "OMEGALUL", "WeirdChamp", "PepeLaugh"];
const COLORS = ["#ff8280", "#7cf6ff", "#a970ff", "#ff8a3b", "#0ed463", "#ff5e5e", "#9aff5b", "#ff77c6"];
const SHORT = ["lol", "wp", "gg", "no way", "bro", "ez", "sheeesh", "omg", "👀", "🤣"];
const LONG = [
  "Bro this is actually one of the most insane things I have ever witnessed live on this stream and I have been watching for years",
  "POV you just walked in on the most chaotic moment in the entire stream history and you have absolutely no idea what is going on",
  "I cannot believe what just happened here this has to be the play of the season if not the entire decade let me tell you",
  "Wait wait wait did everyone else just see that or am I completely losing my mind because that did not look real at all",
];
// Unicode emojis only (no emotes/images), variable byte-length / pictographic
// width — the kind that browsers re-measure asynchronously and that the user
// reports as triggering spurious unfollows in the real app.
const EMOJI_POOL = [
  "😀", "😁", "😂", "🤣", "😅", "😆", "😉", "😊", "😎", "🥰", "😍", "🤩", "🥳",
  "😭", "😢", "🥺", "😱", "😨", "😰", "🥶", "🥵", "🤬", "💀", "☠️", "👻", "🤡",
  "🔥", "✨", "🌟", "⭐", "💯", "💥", "💢", "❤️", "💔", "💖", "💕", "🫶", "👀",
  "🎉", "🎊", "🎁", "🎂", "🍾", "🍕", "🍔", "🍟", "🌮", "🍣", "🍩", "☕", "🍺",
  "👍", "👎", "👏", "🙌", "🤝", "🫡", "🤙", "✊", "🤘", "🤞", "👉", "👈", "🤔",
  "🏆", "🎮", "🎯", "🚀", "💸", "🏅", "🥇", "💎", "👑", "📈", "📉", "⚡", "☄️",
];
const EMOJI_STORM_BURSTS = [5, 8, 12, 16, 24];

// Real emote URLs from public CDNs (BetterTTV + Twitch native). Mix of static
// and animated. They load asynchronously and can reflow rows mid-stream —
// the kind of thing that exposes scroll bugs.
const EMOTES: { alt: string; url: string }[] = [
  // BetterTTV (mostly animated webp)
  { alt: "catJAM", url: "https://cdn.betterttv.net/emote/5f1b0186cf6d2144653d2970/3x.webp" },
  { alt: "monkaS", url: "https://cdn.betterttv.net/emote/56e9f494fff3cc5c35e5287e/3x.webp" },
  { alt: "Pepega", url: "https://cdn.betterttv.net/emote/5aca62163e290877a25481ad/3x.webp" },
  { alt: "WeirdChamp", url: "https://cdn.betterttv.net/emote/5d20a55de1cfde376e532972/3x.webp" },
  { alt: "OMEGALUL", url: "https://cdn.betterttv.net/emote/583089f4737a8e61abb0186b/3x.webp" },
  { alt: "PauseChamp", url: "https://cdn.betterttv.net/emote/5d38aaa592fc550c2d5996b8/3x.webp" },
  { alt: "FeelsBadMan", url: "https://cdn.betterttv.net/emote/5a6dee3b2620951f291ec6d0/3x.webp" },
  { alt: "PepoG", url: "https://cdn.betterttv.net/emote/5e7401738c0f5c3723a9812e/3x.webp" },
  { alt: "KEKW", url: "https://cdn.betterttv.net/emote/5e9c6c187e090362f8b0b9e8/3x.webp" },
  { alt: "LULW", url: "https://cdn.betterttv.net/emote/5dc79d1b27360247dd6516ec/3x.webp" },
  { alt: "COGGERS", url: "https://cdn.betterttv.net/emote/5ab6f0ece1d6391b63498774/3x.webp" },
  { alt: "5Head", url: "https://cdn.betterttv.net/emote/5d6096974932b21d9c332904/3x.webp" },
  { alt: "Sadge", url: "https://cdn.betterttv.net/emote/5e0fa9d40550d42106b8a489/3x.webp" },
  { alt: "SUSSY", url: "https://cdn.betterttv.net/emote/6197ab6f54f3344f8806589d/3x.webp" },
  { alt: "AYAYA", url: "https://cdn.betterttv.net/emote/58493695987aab42df852e0f/3x.webp" },
  { alt: "Susge", url: "https://cdn.betterttv.net/emote/5f9f27ca40eb9502e2238a65/3x.webp" },
  // Twitch native
  { alt: "Kappa", url: "https://static-cdn.jtvnw.net/emoticons/v2/25/default/dark/3.0" },
  { alt: "PogChamp", url: "https://static-cdn.jtvnw.net/emoticons/v2/305954156/default/dark/3.0" },
  { alt: "LUL", url: "https://static-cdn.jtvnw.net/emoticons/v2/425618/default/dark/3.0" },
  { alt: "HeyGuys", url: "https://static-cdn.jtvnw.net/emoticons/v2/30259/default/dark/3.0" },
  { alt: "4Head", url: "https://static-cdn.jtvnw.net/emoticons/v2/354/default/dark/3.0" },
  { alt: "BibleThump", url: "https://static-cdn.jtvnw.net/emoticons/v2/360/default/dark/3.0" },
  { alt: "Keepo", url: "https://static-cdn.jtvnw.net/emoticons/v2/1902/default/dark/3.0" },
  { alt: "Kreygasm", url: "https://static-cdn.jtvnw.net/emoticons/v2/1904/default/dark/3.0" },
  { alt: "DansGame", url: "https://static-cdn.jtvnw.net/emoticons/v2/33/default/dark/3.0" },
  { alt: "WutFace", url: "https://static-cdn.jtvnw.net/emoticons/v2/28087/default/dark/3.0" },
  { alt: "BabyRage", url: "https://static-cdn.jtvnw.net/emoticons/v2/196892/default/dark/3.0" },
  { alt: "ResidentSleeper", url: "https://static-cdn.jtvnw.net/emoticons/v2/41/default/dark/3.0" },
];

// Multi-line chaotic content to stress row reflow.
const MULTILINE = [
  "okay so\nfirst of all\nthat was insane",
  "wait\nwhat\nwhat just happened",
  "no but seriously\nlook at that chat\nit's chaos",
  "1\n2\n3\n4\n5\nGO GO GO",
];

const pick = <T,>(arr: T[]): T => arr[Math.floor(Math.random() * arr.length)];

let nextId = 0;
let emojiMode = false;

function emojiBurst(): Token[] {
  const burst = pick(EMOJI_STORM_BURSTS);
  const out: Token[] = [];
  for (let i = 0; i < burst; i++) out.push(pick(EMOJI_POOL));
  if (Math.random() < 0.3) out.splice(Math.floor(burst / 2), 0, pick(SHORT));
  return [out.join(" ")];
}

function chaoticParts(): Token[] {
  const r = Math.random();
  // 25% short, 20% emote burst, 15% mixed text+emote+emoji, 15% long, 10% multiline, 15% emoji storm
  if (r < 0.25) return [pick(SHORT)];
  if (r < 0.45) {
    // 1-6 random emotes
    const n = 1 + Math.floor(Math.random() * 6);
    const out: Token[] = [];
    for (let i = 0; i < n; i++) {
      const e = pick(EMOTES);
      out.push({ emote: e.url, alt: e.alt });
    }
    return out;
  }
  if (r < 0.6) {
    // text + emote(s) + emoji
    const e = pick(EMOTES);
    return [pick(SHORT) + " ", { emote: e.url, alt: e.alt }, " " + pick(EMOJI_POOL)];
  }
  if (r < 0.75) return [pick(LONG)];
  if (r < 0.85) return [pick(MULTILINE)];
  return emojiBurst();
}

function makeMessage(): TestMessage {
  const parts: Token[] = emojiMode ? emojiBurst() : chaoticParts();
  return {
    id: `m-${nextId++}`,
    user: pick(USERS),
    color: pick(COLORS),
    parts,
    ts: Date.now(),
  };
}

const ROW_STYLE: React.CSSProperties = { padding: "2px 0", fontSize: 13, lineHeight: 1.3, wordBreak: "break-word", whiteSpace: "pre-wrap" };
const EMOTE_STYLE: React.CSSProperties = { display: "inline-block", height: 24, verticalAlign: "middle", marginInline: 2 };

const Row = memo(function Row({ msg }: { msg: TestMessage }) {
  return (
    <div data-msg-id={msg.id} style={ROW_STYLE}>
      <span style={{ color: "#888", marginRight: 8, fontSize: 11 }}>
        {new Date(msg.ts).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" })}
      </span>
      <span style={{ color: msg.color, fontWeight: 700, marginRight: 6 }}>{msg.user}:</span>
      <span>
        {msg.parts.map((p, i) =>
          typeof p === "string" ? (
            <span key={i}>{p}</span>
          ) : (
            <img key={i} src={p.emote} alt={p.alt} title={p.alt} style={EMOTE_STYLE} loading="lazy" />
          )
        )}
      </span>
    </div>
  );
});

declare global {
  interface Window {
    __test?: {
      start: (rate: number) => void;
      stop: () => void;
      count: () => number;
      topVisibleMsgId: () => string | null;
      topVisibleRect: () => { id: string; top: number; bottom: number } | null;
      // Programmatic scroll like real wheel input (small deltas spread over time).
      wheelScroll: (totalDeltaY: number, durationMs: number) => Promise<void>;
      // Read state.
      isFollowing: () => boolean;
      scrollOffset: () => number;
      scrollSize: () => number;
      // Reset buffer.
      clear: () => void;
    };
  }
}

function ChatHarness() {
  const [messages, setMessages] = useState<TestMessage[]>([]);
  const [isFollowing, setIsFollowing] = useState(true);
  const [rate, setRate] = useState(50);
  const [running, setRunning] = useState(false);
  const [emojiStorm, setEmojiStorm] = useState(false);
  // Counts how many times we transitioned from following → not following
  // WITHOUT a wheel-up event in flight. Should always stay 0 — any increment
  // means a spurious unfollow snuck in (the bug the user reports).
  const spuriousUnfollowsRef = useRef(0);
  const [spuriousUnfollows, setSpuriousUnfollows] = useState(0);
  const spamTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const startSpam = useCallback((r: number, withEmojis?: boolean) => {
    if (spamTimerRef.current) clearInterval(spamTimerRef.current);
    emojiMode = withEmojis ?? emojiMode;
    const intervalMs = Math.max(1, 1000 / r);
    spamTimerRef.current = setInterval(() => {
      setMessages((prev) => [...prev, makeMessage()].slice(-CAP));
    }, intervalMs);
    setRunning(true);
  }, []);
  const stopSpam = useCallback(() => {
    if (spamTimerRef.current) {
      clearInterval(spamTimerRef.current);
      spamTimerRef.current = null;
    }
    setRunning(false);
  }, []);
  const isFollowingRef = useRef(true);
  const vlistRef = useRef<VListHandle>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const messagesLenRef = useRef(0);
  const lastScrollOffsetRef = useRef(0);
  messagesLenRef.current = messages.length;

  // Freeze approach: while reading (not following), virtua sees a FIXED
  // snapshot of the messages array. No items appended, none dropped → the
  // scroll position is exact by construction. Zero compensation required.
  // The live `messages` array keeps accumulating in the background; we just
  // don't render it. When the user returns to the bottom, we swap back.
  const [snapshot, setSnapshot] = useState<TestMessage[] | null>(null);
  const displayed = snapshot ?? messages;
  const displayedLenRef = useRef(0);
  displayedLenRef.current = displayed.length;

  const userScrollingRef = useRef(false);
  const userScrollClearTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const closeBottomGap = useCallback(() => {
    const h = vlistRef.current;
    if (!h) return;
    const gap = h.scrollSize - h.scrollOffset - h.viewportSize;
    if (gap > 0) h.scrollTo(h.scrollSize);
  }, []);

  const pinRafRef = useRef<number | null>(null);
  const pinToLast = useCallback(() => {
    if (!isFollowingRef.current) return;
    if (pinRafRef.current !== null) return;
    pinRafRef.current = requestAnimationFrame(() => {
      pinRafRef.current = null;
      const h = vlistRef.current;
      if (!h || !isFollowingRef.current || displayedLenRef.current === 0) return;
      h.scrollToIndex(displayedLenRef.current - 1, { align: "end" });
      closeBottomGap();
    });
  }, [closeBottomGap]);

  // While following, every new live message → scroll to bottom. While reading
  // (snapshot active), this effect never fires because `displayed` doesn't
  // change. messages.length is the right trigger; we want to react to live
  // chat growth even though we're rendering a snapshot.
  useEffect(() => {
    pinToLast();
  }, [messages.length, pinToLast]);

  const handleListScroll = useCallback(() => {
    const h = vlistRef.current;
    if (!h) return;
    lastScrollOffsetRef.current = h.scrollOffset;
    // Re-follow is handled in onListScrollEnd, not here — `handleListScroll`
    // fires too often (every scroll frame) and would race with the
    // userScrollingRef gate.
  }, []);

  // Mark user-driven input. The wheel handler also handles the unfollow
  // transition directly: scrolling up while at the bottom is a one-event
  // signal — waiting for `handleListScroll` to compare scrollOffset deltas
  // doesn't work because `pinToLast` keeps snapping the user back to the
  // bottom between wheel events, so the delta-based detection never trips.
  // Detect spurious unfollows: any transition follow → !follow without a
  // wheel-up in the last 500 ms is a bug (some code path other than the
  // wheel handler is flipping the state).
  const lastWheelUpAtRef = useRef(0);
  const prevFollowingRef = useRef(isFollowing);
  useEffect(() => {
    if (prevFollowingRef.current && !isFollowing) {
      const sinceWheel = Date.now() - lastWheelUpAtRef.current;
      if (sinceWheel > 500) {
        spuriousUnfollowsRef.current++;
        setSpuriousUnfollows(spuriousUnfollowsRef.current);
      }
    }
    prevFollowingRef.current = isFollowing;
  }, [isFollowing]);
  useEffect(() => {
    const root = containerRef.current;
    if (!root) return;
    const markUserScroll = () => {
      userScrollingRef.current = true;
      if (userScrollClearTimerRef.current) clearTimeout(userScrollClearTimerRef.current);
      userScrollClearTimerRef.current = setTimeout(() => {
        userScrollingRef.current = false;
        userScrollClearTimerRef.current = null;
      }, 150);
    };
    const onWheel = (e: Event) => {
      markUserScroll();
      const we = e as WheelEvent;
      if (we.deltaY < 0 && isFollowingRef.current) {
        lastWheelUpAtRef.current = Date.now();
        isFollowingRef.current = false;
        setIsFollowing(false);
        setSnapshot(messages.slice());
      }
    };
    root.addEventListener("wheel", onWheel, { passive: true, capture: true });
    root.addEventListener("touchmove", markUserScroll, { passive: true, capture: true });
    root.addEventListener("keydown", markUserScroll, { capture: true });
    return () => {
      root.removeEventListener("wheel", onWheel, { capture: true } as any);
      root.removeEventListener("touchmove", markUserScroll, { capture: true } as any);
      root.removeEventListener("keydown", markUserScroll, { capture: true } as any);
      // Do NOT clear the user-scrolling timer here — re-mounts from
      // unrelated dep changes would orphan the timer and leave the ref
      // stuck at true. The timer is owned by a long-lived ref and only
      // canceled when superseded by a fresh user input.
    };
  }, [messages, pinToLast]);

  const onListScrollEnd = useCallback(() => {
    const h = vlistRef.current;
    if (!h) return;
    if (isFollowingRef.current) {
      closeBottomGap();
      return;
    }
    // virtua fires onScrollEnd after the scroll has truly settled — momentum
    // included. Reliable signal for "user is done moving." If they ended at
    // the bottom, swap to live and snap immediately.
    const distanceFromBottom = h.scrollSize - h.scrollOffset - h.viewportSize;
    if (distanceFromBottom < SCROLL_NOISE_PX) {
      isFollowingRef.current = true;
      setIsFollowing(true);
      setSnapshot(null);
      pinToLast();
    }
  }, [closeBottomGap, pinToLast]);

  // Expose test API on window.
  useEffect(() => {
    const findScrollContainer = (): HTMLElement | null => {
      const root = containerRef.current;
      if (!root) return null;
      const candidates = root.querySelectorAll("*");
      for (const el of candidates) {
        const e = el as HTMLElement;
        if (e.scrollHeight > e.clientHeight && e.clientHeight > 100) return e;
      }
      return null;
    };

    const findTopVisible = (): { id: string; top: number; bottom: number } | null => {
      const sc = findScrollContainer();
      if (!sc) return null;
      const containerTop = sc.getBoundingClientRect().top;
      const rows = sc.querySelectorAll("[data-msg-id]");
      // Virtua recycles DOM order. Pick the visually-topmost row whose bottom
      // crosses into the viewport.
      let best: { id: string; top: number; bottom: number } | null = null;
      for (const row of rows) {
        const r = row.getBoundingClientRect();
        if (r.bottom <= containerTop) continue;
        if (best === null || r.top < best.top) {
          best = { id: (row as HTMLElement).dataset.msgId!, top: r.top, bottom: r.bottom };
        }
      }
      return best;
    };

    const rectOf = (id: string): { top: number; bottom: number } | null => {
      const el = containerRef.current?.querySelector(`[data-msg-id="${id}"]`) as HTMLElement | null;
      if (!el) return null;
      const r = el.getBoundingClientRect();
      return { top: r.top, bottom: r.bottom };
    };

    (window.__test as any) = {
      rectOf,
      start: (r: number) => startSpam(r),
      stop: () => stopSpam(),
      count: () => messagesLenRef.current,
      topVisibleMsgId: () => findTopVisible()?.id ?? null,
      topVisibleRect: () => findTopVisible(),
      wheelScroll: async (totalDeltaY: number, durationMs: number) => {
        const sc = findScrollContainer();
        if (!sc) return;
        const steps = Math.max(1, Math.round(durationMs / 16));
        const perStep = totalDeltaY / steps;
        for (let i = 0; i < steps; i++) {
          // Real wheel events: dispatch + also adjust scrollTop (virtua relies on
          // native scroll, not on wheel handlers, so we must move scrollTop).
          sc.scrollTop += perStep;
          sc.dispatchEvent(
            new WheelEvent("wheel", { deltaY: perStep, bubbles: true, cancelable: true })
          );
          await new Promise((r) => setTimeout(r, 16));
        }
      },
      isFollowing: () => isFollowingRef.current,
      scrollOffset: () => vlistRef.current?.scrollOffset ?? 0,
      scrollSize: () => vlistRef.current?.scrollSize ?? 0,
      clear: () => {
        setMessages([]);
        nextId = 0;
      },
    };
  }, [startSpam, stopSpam]);

  const scrollContainer = useCallback((): HTMLElement | null => {
    const root = containerRef.current;
    if (!root) return null;
    for (const el of root.querySelectorAll<HTMLElement>("*")) {
      if (el.scrollHeight > el.clientHeight && el.clientHeight > 100) return el;
    }
    return null;
  }, []);

  const scrollToBottom = useCallback(() => {
    if (displayedLenRef.current === 0 || isFollowingRef.current) return;
    isFollowingRef.current = true;
    setIsFollowing(true);
    setSnapshot(null);
    pinToLast();
  }, [pinToLast]);
  const wheelScrollUI = useCallback(async (delta: number, ms: number) => {
    const sc = scrollContainer();
    if (!sc) return;
    const steps = Math.max(1, Math.round(ms / 16));
    const perStep = delta / steps;
    for (let i = 0; i < steps; i++) {
      sc.scrollTop += perStep;
      sc.dispatchEvent(new WheelEvent("wheel", { deltaY: perStep, bubbles: true, cancelable: true }));
      await new Promise((r) => setTimeout(r, 16));
    }
  }, [scrollContainer]);

  return (
    <div style={{ display: "flex", height: "100vh", gap: 0 }}>
      <ControlPanel
        running={running}
        rate={rate}
        setRate={setRate}
        emojiStorm={emojiStorm}
        setEmojiStorm={(v) => { setEmojiStorm(v); emojiMode = v; }}
        onStart={() => startSpam(rate, emojiStorm)}
        onStop={stopSpam}
        onClear={() => { stopSpam(); setMessages([]); setSnapshot(null); nextId = 0; spuriousUnfollowsRef.current = 0; setSpuriousUnfollows(0); }}
        onWheel={wheelScrollUI}
        isFollowing={isFollowing}
        liveCount={messages.length}
        snapshotCount={snapshot?.length ?? null}
        spuriousUnfollows={spuriousUnfollows}
      />
      <div
        ref={containerRef}
        style={{
          display: "flex",
          flexDirection: "column",
          height: "100vh",
          flex: 1,
          background: "#18181b",
          borderLeft: "1px solid #2a2a2d",
        }}
      >
        <div
          style={{
            padding: "8px 12px",
            borderBottom: "1px solid #2a2a2d",
            fontSize: 12,
            color: isFollowing ? "#0ed463" : "#ff8a3b",
          }}
        >
          {isFollowing
            ? `● LIVE — ${messages.length} msgs`
            : `○ READING (frozen at ${snapshot?.length ?? 0}, live: ${messages.length})`}
        </div>
        <div style={{ flex: 1, position: "relative", display: "flex", flexDirection: "column", minHeight: 0 }}>
          <VList
            ref={vlistRef}
            onScroll={handleListScroll}
            onScrollEnd={onListScrollEnd}
            style={{ flex: 1, padding: 12 }}
          >
            {displayed.map((m) => (
              <Row key={m.id} msg={m} />
            ))}
          </VList>
          {!isFollowing && (
            <button
              onClick={scrollToBottom}
              style={{
                position: "absolute",
                bottom: 16,
                left: "50%",
                transform: "translateX(-50%)",
                background: "#9147ff",
                color: "#fff",
                border: "none",
                borderRadius: 999,
                padding: "8px 16px",
                fontSize: 12,
                fontWeight: 700,
                cursor: "pointer",
                boxShadow: "0 4px 12px rgba(0,0,0,0.4)",
              }}
            >
              ↓ Scroll to bottom
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

interface ControlPanelProps {
  running: boolean;
  rate: number;
  setRate: (n: number) => void;
  emojiStorm: boolean;
  setEmojiStorm: (v: boolean) => void;
  onStart: () => void;
  onStop: () => void;
  onClear: () => void;
  onWheel: (delta: number, ms: number) => Promise<void>;
  isFollowing: boolean;
  liveCount: number;
  snapshotCount: number | null;
  spuriousUnfollows: number;
}

const btnStyle: React.CSSProperties = {
  background: "#26262a",
  color: "#efeff1",
  border: "1px solid #3a3a3d",
  borderRadius: 4,
  padding: "6px 10px",
  fontSize: 12,
  cursor: "pointer",
  fontFamily: "inherit",
};
const labelStyle: React.CSSProperties = { fontSize: 10, color: "#888", textTransform: "uppercase", letterSpacing: 0.5, marginBottom: 4 };
const sectionStyle: React.CSSProperties = { display: "flex", flexDirection: "column", gap: 6, padding: "10px 12px", borderBottom: "1px solid #2a2a2d" };

function ControlPanel({ running, rate, setRate, emojiStorm, setEmojiStorm, onStart, onStop, onClear, onWheel, isFollowing, liveCount, snapshotCount, spuriousUnfollows }: ControlPanelProps) {
  return (
    <div style={{ width: 280, background: "#0e0e10", borderRight: "1px solid #2a2a2d", overflowY: "auto", fontSize: 12 }}>
      <div style={sectionStyle}>
        <div style={labelStyle}>Spam stream</div>
        <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
          <input
            type="number"
            value={rate}
            min={1}
            max={500}
            onChange={(e) => setRate(Number(e.target.value) || 1)}
            style={{ ...btnStyle, width: 70, padding: "6px 8px" }}
          />
          <span style={{ color: "#888" }}>msg/s</span>
        </div>
        <div style={{ display: "flex", gap: 6 }}>
          {running ? (
            <button style={{ ...btnStyle, background: "#7a1f1f", flex: 1 }} onClick={onStop}>■ Stop</button>
          ) : (
            <button style={{ ...btnStyle, background: "#1f5a3a", flex: 1 }} onClick={onStart}>▶ Start</button>
          )}
          <button style={btnStyle} onClick={onClear}>Clear</button>
        </div>
        <div style={{ display: "flex", gap: 4 }}>
          {[10, 30, 50, 100, 200].map((r) => (
            <button key={r} style={{ ...btnStyle, padding: "4px 8px", fontSize: 11 }} onClick={() => setRate(r)}>{r}</button>
          ))}
        </div>
        <label style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 11, color: "#aaa", cursor: "pointer", marginTop: 4 }}>
          <input
            type="checkbox"
            checked={emojiStorm}
            onChange={(e) => setEmojiStorm(e.target.checked)}
          />
          Emoji storm (Unicode only, variable widths)
        </label>
      </div>

      <div style={sectionStyle}>
        <div style={labelStyle}>Scroll up (deltaY = −px)</div>
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 4 }}>
          <button style={btnStyle} onClick={() => onWheel(-300, 100)}>−300 fast</button>
          <button style={btnStyle} onClick={() => onWheel(-300, 800)}>−300 slow</button>
          <button style={btnStyle} onClick={() => onWheel(-1000, 200)}>−1000 fast</button>
          <button style={btnStyle} onClick={() => onWheel(-1000, 1500)}>−1000 slow</button>
          <button style={btnStyle} onClick={() => onWheel(-3000, 300)}>−3000 fast</button>
          <button style={btnStyle} onClick={() => onWheel(-3000, 2000)}>−3000 slow</button>
        </div>
        <div style={labelStyle}>Scroll down</div>
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 4 }}>
          <button style={btnStyle} onClick={() => onWheel(300, 200)}>+300</button>
          <button style={btnStyle} onClick={() => onWheel(1000, 400)}>+1000</button>
        </div>
        <div style={labelStyle}>Trickle scroll (tiny deltas)</div>
        <button style={btnStyle} onClick={() => onWheel(-200, 2000)}>−200 over 2s (slow trackpad)</button>
        <button style={btnStyle} onClick={() => onWheel(-500, 5000)}>−500 over 5s (very slow)</button>
      </div>

      <div style={sectionStyle}>
        <div style={labelStyle}>State</div>
        <div style={{ display: "grid", gridTemplateColumns: "auto 1fr", gap: "2px 8px", fontFamily: "ui-monospace, monospace", fontSize: 11 }}>
          <span style={{ color: "#888" }}>mode:</span>
          <span style={{ color: isFollowing ? "#0ed463" : "#ff8a3b" }}>{isFollowing ? "LIVE" : "READING (frozen)"}</span>
          <span style={{ color: "#888" }}>live buffer:</span>
          <span>{liveCount}/{CAP}</span>
          <span style={{ color: "#888" }}>snapshot:</span>
          <span>{snapshotCount === null ? "—" : `${snapshotCount} (frozen)`}</span>
          <span style={{ color: "#888" }}>spurious:</span>
          <span style={{ color: spuriousUnfollows > 0 ? "#ff5e5e" : "#0ed463" }}>{spuriousUnfollows}</span>
        </div>
      </div>

      <div style={sectionStyle}>
        <div style={labelStyle}>Notes</div>
        <div style={{ color: "#888", lineHeight: 1.5, fontSize: 11 }}>
          Freeze approach: scroll up → list is frozen to a snapshot. New incoming live messages don't affect what you're reading at all (the snapshot doesn't change). Scroll back to the bottom → snap back to live stream.
        </div>
      </div>
    </div>
  );
}

createRoot(document.getElementById("root")!).render(<ChatHarness />);
