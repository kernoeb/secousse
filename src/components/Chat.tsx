import { useState, useMemo, memo, useRef, useEffect, useCallback } from "react";
import { PanelRight, PanelLeft, User, Settings, Send } from "lucide-react";
import { VList, type VListHandle } from "virtua";
import { cn, twitchEmoteUrl } from "../lib/utils";
import type { ChatMessage, TwitchBadge } from "../types";
import { EmoteImg } from "./EmoteImg";

const VLIST_STYLE: React.CSSProperties = { padding: 12 };

// Tolerance for re-engaging follow-bottom: once the user scrolls within
// this many px of the bottom, snap back to following live messages.
const SCROLL_NOISE_PX = 16;

interface ChatProps {
  isOpen: boolean;
  setIsOpen: (open: boolean) => void;
  messages: ChatMessage[];
  emotes: Map<string, string>;
  globalBadges: TwitchBadge[];
  channelBadges: TwitchBadge[];
  isLoggedIn: boolean;
  isConnected: boolean;
  onSendMessage: (message: string) => void;
}

export function Chat({
  isOpen,
  setIsOpen,
  messages,
  emotes,
  globalBadges,
  channelBadges,
  isLoggedIn,
  isConnected,
  onSendMessage,
}: ChatProps) {
  const [chatInput, setChatInput] = useState("");
  const [isFollowing, setIsFollowing] = useState(true);
  const isFollowingRef = useRef(true);
  const vlistRef = useRef<VListHandle>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  // Freeze approach: while the user is reading (not following), virtua sees
  // a FIXED snapshot of the messages array. No items appended, none dropped
  // → scrollOffset stays exact by construction, zero compensation needed.
  // The live `messages` prop keeps accumulating in the background; we just
  // don't render it. When the user lands back at the bottom we swap to live.
  const [snapshot, setSnapshot] = useState<ChatMessage[] | null>(null);
  const displayed = snapshot ?? messages;
  const displayedLenRef = useRef(0);
  displayedLenRef.current = displayed.length;
  // Read inside the wheel handler so the listener effect can have `[]` deps.
  // With `[messages]`, the listener would detach/re-attach on every new chat
  // message (50–100×/s on busy streams) for no behavioral gain.
  const messagesRef = useRef(messages);
  messagesRef.current = messages;

  const badgeIndex = useMemo(() => {
    const map = new Map<string, string>();
    for (const b of globalBadges) map.set(`${b.setID}/${b.version}`, b.imageURL);
    for (const b of channelBadges) map.set(`${b.setID}/${b.version}`, b.imageURL);
    return map;
  }, [globalBadges, channelBadges]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      if (chatInput.trim()) {
        onSendMessage(chatInput);
        setChatInput("");
      }
    }
  };

  const handleSend = () => {
    if (chatInput.trim()) {
      onSendMessage(chatInput);
      setChatInput("");
    }
  };

  // scrollToIndex can land short of the true bottom in two cases:
  // - per-item size-cache rounding (1-2px clip on the last text line)
  // - virtua aligning to last-item.bottom rather than scroll-area.bottom,
  //   eating the VList's padding-bottom.
  // Caller must have already gated on isFollowingRef.
  const closeBottomGap = useCallback(() => {
    const h = vlistRef.current;
    if (!h) return;
    const gap = h.scrollSize - h.scrollOffset - h.viewportSize;
    if (gap > 0) h.scrollTo(h.scrollSize);
  }, []);

  // rAF coalesces N pin requests per frame (hot when many emote <img> onLoad
  // fire near-simultaneously alongside the messages-length effect).
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

  const onListScrollEnd = useCallback(() => {
    const h = vlistRef.current;
    if (!h) return;
    if (isFollowingRef.current) {
      closeBottomGap();
      return;
    }
    // virtua fires onScrollEnd after scroll truly settles — momentum scroll
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

  useEffect(() => {
    pinToLast();
  }, [messages.length, pinToLast]);

  useEffect(() => {
    const root = containerRef.current;
    if (!root) return;
    const onWheel = (e: WheelEvent) => {
      if (e.deltaY < 0 && isFollowingRef.current) {
        isFollowingRef.current = false;
        setIsFollowing(false);
        setSnapshot(messagesRef.current.slice());
      }
    };
    root.addEventListener("wheel", onWheel, { passive: true, capture: true });
    return () => {
      root.removeEventListener("wheel", onWheel, true);
    };
  }, []);

  useEffect(() => () => {
    if (pinRafRef.current !== null) cancelAnimationFrame(pinRafRef.current);
  }, []);

  const scrollToBottom = useCallback(() => {
    if (displayedLenRef.current === 0 || isFollowingRef.current) return;
    isFollowingRef.current = true;
    setIsFollowing(true);
    setSnapshot(null);
    pinToLast();
  }, [pinToLast]);

  return (
    <>
      <aside
        ref={containerRef}
        className={cn(
          "bg-surface border-l border-border flex flex-col z-30 transition-all duration-300 relative",
          isOpen ? "w-[340px]" : "w-0 overflow-hidden"
        )}
      >
        <div className="h-12 flex items-center justify-between px-4 border-b border-border">
          <button
            onClick={() => setIsOpen(false)}
            className="p-1.5 hover:bg-hover rounded-md transition-colors opacity-80 hover:opacity-100"
            title="Close chat"
          >
            <PanelRight className="w-4 h-4" />
          </button>
          <span className="font-bold text-[11px] uppercase tracking-[0.1em] text-muted">Stream Chat</span>
          <button className="p-1.5 hover:bg-hover rounded-md transition-colors opacity-80 hover:opacity-100">
            <User className="w-4 h-4" />
          </button>
        </div>

        <VList
          ref={vlistRef}
          onScrollEnd={onListScrollEnd}
          className="flex-1 scrollbar-thin"
          style={VLIST_STYLE}
        >
          {displayed.map((m) => (
            <ChatMessageView
              key={m._renderKey ?? m.id}
              msg={m}
              emotes={emotes}
              badgeIndex={badgeIndex}
              onImageLoad={pinToLast}
            />
          ))}
        </VList>

        {!isFollowing && (
          <button
            onClick={scrollToBottom}
            className="absolute bottom-36 left-1/2 -translate-x-1/2 bg-twitch hover:bg-twitch-dark text-white text-xs px-3 py-1.5 rounded-full shadow-lg transition-all z-10"
          >
            Scroll to bottom
          </button>
        )}

        <div className="p-3 border-t border-border">
          <div className="relative mb-3">
            <textarea
              placeholder={!isLoggedIn ? "Log in to chat" : !isConnected ? "Connecting to chat..." : "Send a message"}
              value={chatInput}
              onChange={(e) => setChatInput(e.target.value)}
              onKeyDown={handleKeyDown}
              disabled={!isLoggedIn || !isConnected}
              className={cn(
                "w-full bg-base border border-border rounded-md p-2 text-sm focus:outline-none focus:border-twitch resize-none min-h-[44px] max-h-[160px] transition-all placeholder:text-muted/40",
                (!isLoggedIn || !isConnected) && "opacity-50 cursor-not-allowed"
              )}
            />
          </div>
          <div className="flex items-center justify-between">
            <button className="p-2 hover:bg-hover rounded-md transition-colors group">
              <Settings className="w-4 h-4 text-muted group-hover:text-[#e8e8ee]" />
            </button>
            <button
              onClick={handleSend}
              disabled={!isLoggedIn || !isConnected || !chatInput.trim()}
              className={cn(
                "bg-twitch hover:bg-twitch-dark px-4 py-1.5 rounded-md font-bold text-[13px] transition-all shadow-lg shadow-twitch/20 active:scale-95 text-white flex items-center gap-2",
                (!isLoggedIn || !isConnected || !chatInput.trim()) && "opacity-50 cursor-not-allowed"
              )}
            >
              <Send className="w-4 h-4" /> Chat
            </button>
          </div>
        </div>
      </aside>

      {!isOpen && (
        <button
          onClick={() => setIsOpen(true)}
          className="absolute right-4 top-16 bg-twitch hover:bg-twitch-dark p-2 rounded-md z-40 transition-all"
          title="Open chat"
        >
          <PanelLeft className="w-5 h-5 text-white" />
        </button>
      )}
    </>
  );
}

interface ChatMessageViewProps {
  msg: ChatMessage;
  emotes: Map<string, string>;
  badgeIndex: Map<string, string>;
  onImageLoad: () => void;
}

const ChatMessageView = memo(function ChatMessageView({ msg, emotes, badgeIndex, onImageLoad }: ChatMessageViewProps) {
  const badgeUrls = useMemo(() => {
    if (!msg.badges) return [];
    const out: string[] = [];
    for (const [name, version] of msg.badges) {
      const url = badgeIndex.get(`${name}/${version}`);
      if (url) out.push(url);
    }
    return out;
  }, [msg.badges, badgeIndex]);

  const parts = useMemo(() => {
    const codepoints = Array.from(msg.message);
    const ranges = msg.emotes
      .filter(r => r.start <= r.end && r.end < codepoints.length)
      .sort((a, b) => a.start - b.start);

    const emoteImg = (key: string, src: string, alt: string) => (
      <EmoteImg key={key} url={src} alt={alt} onReady={onImageLoad} />
    );

    const renderText = (text: string, keyPrefix: string) =>
      text.split(/(\s+)/).map((word, i) => {
        if (!word) return null;
        const url = emotes.get(word);
        return url ? emoteImg(`${keyPrefix}-${i}`, url, word) : word;
      });

    const out: React.ReactNode[] = [];
    let cursor = 0;
    ranges.forEach((r, idx) => {
      if (r.start > cursor) {
        out.push(...renderText(codepoints.slice(cursor, r.start).join(""), `t${idx}`));
      }
      const name = codepoints.slice(r.start, r.end + 1).join("");
      out.push(emoteImg(`e${idx}`, twitchEmoteUrl(r.id), name));
      cursor = r.end + 1;
    });
    if (cursor < codepoints.length) {
      out.push(...renderText(codepoints.slice(cursor).join(""), "tail"));
    }
    return out;
  }, [msg.message, msg.emotes, emotes, onImageLoad]);

  return (
    <div className="text-[13px] leading-tight break-words py-0.5">
      <span className="text-muted mr-2 text-[11px]">
        {new Date(msg.timestamp).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}
      </span>
      <span className="inline-flex gap-0.5 mr-1 align-middle">
        {badgeUrls.map((url, i) => (
          <img key={i} src={url} loading="lazy" decoding="async" className="w-4 h-4 rounded-sm" />
        ))}
      </span>
      <span
        className="font-bold hover:bg-hover cursor-pointer rounded px-1 -ml-1 mr-1"
        style={{ color: msg.color || "#ff8280" }}
      >
        {msg.user}:
      </span>
      <span>{parts}</span>
    </div>
  );
});
