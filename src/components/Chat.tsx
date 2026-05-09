import { useState, useMemo, memo, useRef, useEffect, useCallback } from "react";
import { PanelRight, PanelLeft, User, Settings, Send } from "lucide-react";
import { VList, type VListHandle } from "virtua";
import { cn, twitchEmoteUrl } from "../lib/utils";
import type { ChatMessage, TwitchBadge } from "../types";

const VLIST_STYLE: React.CSSProperties = { padding: 12 };

// Sub-pixel gap below which the pin closes via direct scrollTo. Larger gaps
// indicate a real user scroll-up and must not be overridden.
const PIN_GAP_SLOP_PX = 8;
// Tolerance for distinguishing programmatic vs user scroll. Set wide enough
// to absorb virtua's offset jitter when it re-measures items during layout
// shifts (typical few-pixel adjustments after image-load reflow), so the user
// only "unfollows" on a real scroll-up, not on a measurement artifact.
const SCROLL_NOISE_PX = 16;
// Distance from bottom past which a user up-scroll is considered intentional.
const UNFOLLOW_THRESHOLD_PX = 100;

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
  // Tracks user *intent* to follow the bottom, not pixel proximity. A layout
  // shift (emote loads, message grows) doesn't disengage — only an active
  // up-scroll does. The ref keeps `pinToLast` stable across renders so it
  // doesn't invalidate ChatMessageView's `parts` useMemo on every message.
  const [isFollowing, setIsFollowing] = useState(true);
  const isFollowingRef = useRef(true);
  const vlistRef = useRef<VListHandle>(null);
  const messagesLenRef = useRef(0);
  const lastScrollOffsetRef = useRef(0);
  messagesLenRef.current = messages.length;

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

  // Two-pass scroll: virtua measures items via ResizeObserver one frame after
  // they mount, so the first scrollToIndex uses an estimated/cached size for
  // a fresh row. The second pass on the next frame lands flush with the real
  // bottom once the row's actual height is known. Same logic catches late
  // emote-image loads that grow the row after the first pin.
  const pinRafRef = useRef<number | null>(null);
  const pinToLast = useCallback(() => {
    if (!isFollowingRef.current) return;
    if (pinRafRef.current !== null) return;
    pinRafRef.current = requestAnimationFrame(() => {
      pinRafRef.current = null;
      const h = vlistRef.current;
      if (!h || !isFollowingRef.current || messagesLenRef.current === 0) return;
      h.scrollToIndex(messagesLenRef.current - 1, { align: "end" });
      // Per-item size-cache rounding can leave scrollToIndex 1-2px short of
      // the absolute bottom and clip the last text line. Close that gap via
      // direct scrollTo, capped so a real user-up-scroll isn't overridden.
      const gap = h.scrollSize - h.scrollOffset - h.viewportSize;
      if (gap > 0 && gap < PIN_GAP_SLOP_PX) h.scrollTo(h.scrollSize);
    });
  }, []);

  useEffect(() => {
    pinToLast();
  }, [messages.length, pinToLast]);

  useEffect(() => () => {
    if (pinRafRef.current !== null) cancelAnimationFrame(pinRafRef.current);
  }, []);

  const handleListScroll = useCallback(() => {
    const h = vlistRef.current;
    if (!h) return;
    const offset = h.scrollOffset;
    const distanceFromBottom = h.scrollSize - offset - h.viewportSize;
    const userScrolledUp = offset < lastScrollOffsetRef.current - SCROLL_NOISE_PX;
    lastScrollOffsetRef.current = offset;

    if (isFollowingRef.current && userScrolledUp && distanceFromBottom > UNFOLLOW_THRESHOLD_PX) {
      isFollowingRef.current = false;
      setIsFollowing(false);
    } else if (!isFollowingRef.current && distanceFromBottom < SCROLL_NOISE_PX) {
      isFollowingRef.current = true;
      setIsFollowing(true);
    }
  }, []);

  const scrollToBottom = useCallback(() => {
    if (messagesLenRef.current === 0 || isFollowingRef.current) return;
    isFollowingRef.current = true;
    setIsFollowing(true);
    pinToLast();
  }, [pinToLast]);

  return (
    <>
      <aside
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
          onScroll={handleListScroll}
          className="flex-1 custom-scrollbar"
          style={VLIST_STYLE}
        >
          {messages.map((m) => (
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
      <img key={key} src={src} alt={alt} loading="lazy" decoding="async" onLoad={onImageLoad} className="inline-block h-6 mx-0.5 align-middle" />
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
