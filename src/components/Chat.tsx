import { useState, useMemo, memo, useRef, useEffect, useCallback } from "react";
import { useI18n, type Translate } from "../i18n";
import { PanelRight, PanelLeft, User, Send, Reply, X } from "lucide-react";
import { VList, type VListHandle } from "virtua";
import { cn, twitchEmoteUrl } from "../lib/utils";
import type { ChatDeletion, ChatMessage, ChatReplyParent, TwitchBadge } from "../types";
import { EmoteImg } from "./EmoteImg";

const VLIST_STYLE: React.CSSProperties = { padding: 12 };
const ASIDE_STYLE: React.CSSProperties = { contain: "layout style paint" };

// Tolerance for re-engaging follow-bottom: once the user scrolls within
// this many px of the bottom, snap back to following live messages.
const SCROLL_NOISE_PX = 16;

const DELETION_KEYS = {
  message: "chat.deletedMessage",
  timeout: "chat.deletedTimeout",
  ban: "chat.deletedBan",
  clear: "chat.deletedClear",
} as const satisfies Record<ChatDeletion, string>;

// Notices, local echoes ("self-N") and messages without an IRC id have nothing
// Twitch could attach a reply to.
function canReplyTo(msg: ChatMessage): boolean {
  return !msg.system && !msg.deleted && !!msg.id && !msg.id.startsWith("self-");
}

interface ChatProps {
  isOpen: boolean;
  setIsOpen: (open: boolean) => void;
  /** Channel the messages come from. A pending reply is dropped when it changes. */
  channel: string | null;
  messages: ChatMessage[];
  emotes: Map<string, string>;
  globalBadges: TwitchBadge[];
  channelBadges: TwitchBadge[];
  isLoggedIn: boolean;
  isConnected: boolean;
  onSendMessage: (message: string, reply?: ChatReplyParent) => void;
  /** Login of the signed-in user, to highlight messages that mention them. */
  selfLogin?: string | null;
  isFullscreen?: boolean;
  openButtonVisible?: boolean;
}

export function Chat({
  isOpen,
  setIsOpen,
  channel,
  messages,
  emotes,
  globalBadges,
  channelBadges,
  isLoggedIn,
  isConnected,
  onSendMessage,
  selfLogin,
  isFullscreen = false,
  openButtonVisible = true,
}: ChatProps) {
  const { t } = useI18n();
  const [chatInput, setChatInput] = useState("");
  const [replyTo, setReplyTo] = useState<ChatReplyParent | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
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
  // A frozen snapshot must still show moderation, so swap in the rows marked deleted since.
  const displayed = useMemo(() => {
    if (!snapshot) return messages;
    const deleted = new Map<string, ChatMessage>();
    for (const m of messages) if (m.deleted) deleted.set(m._renderKey ?? m.id, m);
    if (deleted.size === 0) return snapshot;
    return snapshot.map((m) => deleted.get(m._renderKey ?? m.id) ?? m);
  }, [snapshot, messages]);
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

  const ownLogin = selfLogin?.toLowerCase() || null;
  const mentionRe = useMemo(() => {
    if (!ownLogin) return null;
    const login = ownLogin.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    return new RegExp(`(?:^|[^\\w])@?${login}(?!\\w)`, "i");
  }, [ownLogin]);

  const startReply = useCallback((msg: ChatMessage) => {
    setReplyTo({
      msg_id: msg.id,
      user_login: msg.login || msg.user.toLowerCase(),
      display_name: msg.user,
      body: msg.message,
    });
    textareaRef.current?.focus();
  }, []);

  // A reply kept across a channel switch would point at a message in another channel.
  useEffect(() => {
    setReplyTo(null);
  }, [channel]);

  const handleSend = () => {
    if (chatInput.trim()) {
      onSendMessage(chatInput, replyTo ?? undefined);
      setChatInput("");
      setReplyTo(null);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    } else if (e.key === "Escape" && replyTo) {
      e.preventDefault();
      setReplyTo(null);
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
        style={ASIDE_STYLE}
        className={cn(
          "bg-surface border-l border-border flex flex-col z-30 transition-all duration-300 relative",
          isOpen ? "w-[340px]" : "w-0 overflow-hidden"
        )}
      >
        <div className="h-12 flex items-center justify-between px-4 border-b border-border">
          <button
            onClick={() => setIsOpen(false)}
            className="p-1.5 hover:bg-hover rounded-md transition-colors opacity-80 hover:opacity-100"
            title={t("chat.close")}
          >
            <PanelRight className="w-4 h-4" />
          </button>
          <span className="font-bold text-[11px] uppercase tracking-[0.1em] text-muted">{t("chat.title")}</span>
          <div className="p-1.5 opacity-80">
            <User className="w-4 h-4" />
          </div>
        </div>

        <VList
          ref={vlistRef}
          onScrollEnd={onListScrollEnd}
          className={isFollowing ? "flex-1 scrollbar-hidden" : "flex-1 scrollbar-thin"}
          style={VLIST_STYLE}
        >
          {displayed.map((m) => (
            <ChatMessageView
              key={m._renderKey ?? m.id}
              msg={m}
              emotes={emotes}
              badgeIndex={badgeIndex}
              onImageLoad={pinToLast}
              selfLogin={ownLogin}
              mentionRe={mentionRe}
              onReply={isLoggedIn ? startReply : undefined}
              t={t}
            />
          ))}
        </VList>

        {!isFollowing && (
          <button
            onClick={scrollToBottom}
            className="absolute bottom-36 left-1/2 -translate-x-1/2 bg-twitch hover:bg-twitch-dark text-white text-xs px-3 py-1.5 rounded-full shadow-lg transition-all z-10"
          >
            {t("chat.scrollToBottom")}
          </button>
        )}

        <div className="p-3 border-t border-border">
          {replyTo && (
            <div className="flex items-center gap-1.5 mb-2 pl-2 pr-1 py-1 rounded-md bg-surface-alt text-[12px] text-muted">
              <Reply className="w-3.5 h-3.5 shrink-0" />
              <span className="flex-1 truncate">{t("chat.replyingTo", { name: replyTo.display_name })}</span>
              <button
                onClick={() => setReplyTo(null)}
                className="p-0.5 rounded hover:bg-hover hover:text-white transition-colors"
                title={t("chat.cancelReply")}
              >
                <X className="w-3.5 h-3.5" />
              </button>
            </div>
          )}
          <div className="relative mb-3">
            <textarea
              ref={textareaRef}
              placeholder={!isLoggedIn ? t("chat.loginToChat") : !isConnected ? t("chat.connecting") : t("chat.placeholder")}
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
          <div className="flex items-center justify-end">
            <button
              onClick={handleSend}
              disabled={!isLoggedIn || !isConnected || !chatInput.trim()}
              className={cn(
                "bg-twitch hover:bg-twitch-dark px-4 py-1.5 rounded-md font-bold text-[13px] transition-all shadow-lg shadow-twitch/20 active:scale-95 text-white flex items-center gap-2",
                (!isLoggedIn || !isConnected || !chatInput.trim()) && "opacity-50 cursor-not-allowed"
              )}
            >
              <Send className="w-4 h-4" /> {t("chat.send")}
            </button>
          </div>
        </div>
      </aside>

      {!isOpen && (
        <button
          onClick={() => setIsOpen(true)}
          className={cn(
            "absolute right-4 bg-twitch hover:bg-twitch-dark p-2 rounded-md z-40 transition-all",
            isFullscreen ? "top-4" : "top-16",
            openButtonVisible ? "opacity-100" : "opacity-0 pointer-events-none",
          )}
          title={t("chat.open")}
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
  selfLogin: string | null;
  mentionRe: RegExp | null;
  /** Absent when logged out: replying needs an account. */
  onReply?: (msg: ChatMessage) => void;
  t: Translate;
}

const ChatMessageView = memo(function ChatMessageView(props: ChatMessageViewProps) {
  if (props.msg.system) return <ChatNoticeView msg={props.msg} />;
  return <ChatUserMessageView {...props} />;
});

function ChatNoticeView({ msg }: { msg: ChatMessage }) {
  return (
    <div className="text-[13px] leading-tight break-words py-0.5 px-2 my-0.5 border-l-2 border-twitch/60 bg-white/[0.03] rounded-sm text-muted italic">
      {msg.message}
    </div>
  );
}

function ChatUserMessageView({ msg, emotes, badgeIndex, onImageLoad, selfLogin, mentionRe, onReply, t }: ChatMessageViewProps) {
  const isMention = useMemo(() => {
    if (!selfLogin || msg.login === selfLogin) return false;
    return msg.reply?.user_login === selfLogin || !!mentionRe?.test(msg.message);
  }, [msg.login, msg.reply, msg.message, selfLogin, mentionRe]);

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
    <div
      className={cn(
        "group relative text-[13px] leading-tight break-words py-0.5",
        isMention && "-mx-2 pl-1.5 pr-2 border-l-2 border-twitch bg-twitch/15 rounded-r-sm",
        msg.deleted && "opacity-50",
      )}
      title={msg.deleted ? t(DELETION_KEYS[msg.deleted]) : undefined}
    >
      {msg.reply && (
        <div className="flex items-center gap-1 mb-0.5 text-[11px] text-muted min-w-0">
          <Reply className="w-3 h-3 shrink-0" />
          <span className="truncate">
            {t("chat.replyContext", { name: msg.reply.display_name })} {msg.reply.body}
          </span>
        </div>
      )}
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
      <span className={msg.deleted ? "text-muted [&_img]:grayscale" : undefined}>{parts}</span>
      {onReply && canReplyTo(msg) && (
        <button
          onClick={() => onReply(msg)}
          className="absolute right-0 top-0 p-0.5 rounded bg-elevated text-muted opacity-0 group-hover:opacity-100 focus-visible:opacity-100 hover:text-white transition-opacity"
          title={t("chat.reply")}
        >
          <Reply className="w-3.5 h-3.5" />
        </button>
      )}
    </div>
  );
}
