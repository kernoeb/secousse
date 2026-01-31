import { useState, useMemo } from "react";
import { PanelRight, PanelLeft, User, Settings, Send } from "lucide-react";
import { cn } from "../lib/utils";
import type { ChatMessage, TwitchBadge } from "../types";

interface ChatProps {
  isOpen: boolean;
  setIsOpen: (open: boolean) => void;
  messages: ChatMessage[];
  emotes: Map<string, string>;
  globalBadges: TwitchBadge[];
  channelBadges: TwitchBadge[];
  isLoggedIn: boolean;
  isAtBottom: boolean;
  chatContainerRef: React.RefObject<HTMLDivElement | null>;
  chatEndRef: React.RefObject<HTMLDivElement | null>;
  onScroll: () => void;
  onScrollToBottom: () => void;
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
  isAtBottom,
  chatContainerRef,
  chatEndRef,
  onScroll,
  onScrollToBottom,
  onSendMessage,
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
          "bg-[#18181b] border-l border-black flex flex-col shadow-2xl z-30 transition-all duration-300 relative",
          isOpen ? "w-[340px]" : "w-0 overflow-hidden"
        )}
      >
        <div className="h-12 flex items-center justify-between px-4 border-b border-black bg-[#18181b]">
          <button
            onClick={() => setIsOpen(false)}
            className="p-1.5 hover:bg-[#2f2f35] rounded-md transition-colors opacity-80 hover:opacity-100"
            title="Close chat"
          >
            <PanelRight className="w-4 h-4" />
          </button>
          <span className="font-bold text-[11px] uppercase tracking-[0.1em] opacity-80">Stream Chat</span>
          <button className="p-1.5 hover:bg-[#2f2f35] rounded-md transition-colors opacity-80 hover:opacity-100">
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
            />
          ))}
          <div ref={chatEndRef} />
        </div>

        {/* Scroll to bottom button */}
        {!isAtBottom && (
          <button
            onClick={onScrollToBottom}
            className="absolute bottom-36 left-1/2 -translate-x-1/2 bg-[#9146ff] hover:bg-[#772ce8] text-white text-xs px-3 py-1.5 rounded-full shadow-lg transition-all z-10"
          >
            Scroll to bottom
          </button>
        )}

        <div className="p-3 bg-[#18181b] border-t border-black">
          <div className="relative mb-3">
            <textarea
              placeholder={isLoggedIn ? "Send a message" : "Log in to chat"}
              value={chatInput}
              onChange={(e) => setChatInput(e.target.value)}
              onKeyDown={handleKeyDown}
              disabled={!isLoggedIn}
              className={cn(
                "w-full bg-[#0e0e10] border border-[#3f3f46] rounded-md p-2 text-sm focus:outline-none focus:border-[#9146ff] resize-none min-h-[44px] max-h-[160px] transition-all placeholder:text-white/20",
                !isLoggedIn && "opacity-50 cursor-not-allowed"
              )}
            />
          </div>
          <div className="flex items-center justify-between">
            <button className="p-2 hover:bg-[#2f2f35] rounded-md transition-colors group">
              <Settings className="w-4 h-4 text-[#adadb8] group-hover:text-[#efeff1]" />
            </button>
            <button
              onClick={handleSend}
              disabled={!isLoggedIn || !chatInput.trim()}
              className={cn(
                "bg-[#9146ff] hover:bg-[#772ce8] px-4 py-1.5 rounded-md font-bold text-[13px] transition-all shadow-lg shadow-[#9146ff]/20 active:scale-95 text-white flex items-center gap-2",
                (!isLoggedIn || !chatInput.trim()) && "opacity-50 cursor-not-allowed"
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
          className="absolute right-4 top-16 bg-[#9146ff] hover:bg-[#772ce8] p-2 rounded-md z-40 transition-all"
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
}

function ChatMessageView({ msg, emotes, globalBadges, channelBadges }: ChatMessageViewProps) {
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
    return msg.message.split(" ").map((word, i) => {
      const emoteUrl = emotes.get(word);
      if (emoteUrl) {
        return <img key={i} src={emoteUrl} alt={word} className="inline-block h-6 mx-0.5 align-middle" />;
      }
      return <span key={i}>{word} </span>;
    });
  }, [msg.message, emotes]);

  return (
    <div className="text-[13px] leading-tight break-words py-0.5">
      <span className="text-[#adadb8] mr-2 text-[11px]">
        {new Date(msg.timestamp).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}
      </span>
      <span className="inline-flex gap-0.5 mr-1 align-middle">
        {badgeUrls.map((url, i) => (
          <img key={i} src={url as string} className="w-4 h-4 rounded-sm" />
        ))}
      </span>
      <span
        className="font-bold hover:bg-[#2f2f35] cursor-pointer rounded px-1 -ml-1 mr-1"
        style={{ color: msg.color || "#ff8280" }}
      >
        {msg.user}:
      </span>
      <span>{parts}</span>
    </div>
  );
}
