import { useState, useMemo, memo } from "react";
import { PanelRight, PanelLeft, User, Settings, Send } from "lucide-react";
import { cn, twitchEmoteUrl } from "../lib/utils";
import type { ChatMessage, TwitchBadge } from "../types";

interface ChatProps {
  isOpen: boolean;
  setIsOpen: (open: boolean) => void;
  messages: ChatMessage[];
  emotes: Map<string, string>;
  globalBadges: TwitchBadge[];
  channelBadges: TwitchBadge[];
  isLoggedIn: boolean;
  isConnected: boolean;
  isAtBottom: boolean;
  chatContainerRef: React.RefObject<HTMLDivElement | null>;
  chatEndRef: React.RefObject<HTMLDivElement | null>;
  onScroll: () => void;
  onScrollToBottom: () => void;
  onSendMessage: (message: string) => void;
  onMessageImageLoad: () => void;
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
  isAtBottom,
  chatContainerRef,
  chatEndRef,
  onScroll,
  onScrollToBottom,
  onSendMessage,
  onMessageImageLoad,
}: ChatProps) {
  const [chatInput, setChatInput] = useState("");

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

        <div
          ref={chatContainerRef}
          onScroll={onScroll}
          className="flex-1 p-3 overflow-y-auto custom-scrollbar"
        >
          {messages.map((m, i) => (
            <ChatMessageView
              key={i}
              msg={m}
              emotes={emotes}
              globalBadges={globalBadges}
              channelBadges={channelBadges}
              onImageLoad={onMessageImageLoad}
            />
          ))}
          <div ref={chatEndRef} />
        </div>

        {/* Scroll to bottom button */}
        {!isAtBottom && (
          <button
            onClick={onScrollToBottom}
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

      {/* Chat toggle button when closed */}
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
  globalBadges: TwitchBadge[];
  channelBadges: TwitchBadge[];
  onImageLoad: () => void;
}

const ChatMessageView = memo(function ChatMessageView({ msg, emotes, globalBadges, channelBadges, onImageLoad }: ChatMessageViewProps) {
  const badgeUrls = useMemo(() => {
    if (!msg.badges) return [];
    return msg.badges
      .map(([name, version]) => {
        const channelBadge = channelBadges?.find(b => b.setID === name && b.version === version);
        if (channelBadge) return channelBadge.imageURL;

        const globalBadge = globalBadges?.find(b => b.setID === name && b.version === version);
        if (globalBadge) return globalBadge.imageURL;

        return null;
      })
      .filter(Boolean);
  }, [msg.badges, globalBadges, channelBadges]);

  const parts = useMemo(() => {
    const codepoints = Array.from(msg.message);
    const ranges = msg.emotes
      .filter(r => r.start <= r.end && r.end < codepoints.length)
      .sort((a, b) => a.start - b.start);

    // onLoad re-pins scroll after late layout shifts (see useChat#pinToBottomIfFollowing).
    const emoteImg = (key: string, src: string, alt: string) => (
      <img key={key} src={src} alt={alt} loading="lazy" decoding="async" onLoad={onImageLoad} className="inline-block h-6 mx-0.5 align-middle" />
    );

    // split on whitespace runs and keep them as separators so output preserves spacing
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
          <img key={i} src={url as string} loading="lazy" decoding="async" className="w-4 h-4 rounded-sm" />
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
