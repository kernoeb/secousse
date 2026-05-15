import { useEffect, useState, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { info, error as logError, attachConsole } from "@tauri-apps/plugin-log";
import { MessageSquare, Pin, PinOff } from "lucide-react";

import { useAuth, useChat, useEmotes, useUserInfo } from "./hooks";
import { VideoPlayer, Chat } from "./components";
import { cn } from "./lib/utils";

interface PopoutAppProps {
  channel: string;
}

export default function PopoutApp({ channel }: PopoutAppProps) {
  const [isFullscreen, setIsFullscreen] = useState(false);
  const [isChatOpen, setIsChatOpen] = useState(false);
  const [alwaysOnTop, setAlwaysOnTop] = useState(false);

  const { isLoggedIn } = useAuth();
  const { allEmotes, globalBadges, channelBadges, loadChannelEmotes } = useEmotes();
  const { userInfo } = useUserInfo(channel);
  const chat = useChat(isChatOpen ? channel : null, isLoggedIn);

  const initRef = useRef(false);

  useEffect(() => {
    if (initRef.current) return;
    initRef.current = true;
    attachConsole();
    info(`[PopoutApp] Initializing for channel: ${channel}`);
    setTimeout(() => invoke("show_main_window").catch(() => {}), 0);
  }, [channel]);

  useEffect(() => {
    if (!userInfo?.id) return;
    loadChannelEmotes(userInfo.id);
  }, [userInfo?.id, loadChannelEmotes]);

  const toggleAlwaysOnTop = useCallback(async () => {
    const next = !alwaysOnTop;
    try {
      await getCurrentWebviewWindow().setAlwaysOnTop(next);
      setAlwaysOnTop(next);
    } catch (err) {
      logError(`[PopoutApp] setAlwaysOnTop failed: ${err}`);
    }
  }, [alwaysOnTop]);

  return (
    <div className="flex flex-col h-screen w-full bg-base text-[#e8e8ee]">
      <header className="flex items-center justify-between px-3 py-2 bg-surface border-b border-border shrink-0">
        <div className="flex items-center gap-2 min-w-0">
          {userInfo?.profileImageURL && (
            <img
              src={userInfo.profileImageURL}
              alt={userInfo.displayName}
              className="w-6 h-6 rounded-full"
            />
          )}
          <span className="font-semibold text-sm truncate">
            {userInfo?.displayName || channel}
          </span>
          {userInfo?.stream && (
            <span className="bg-red-600 text-white text-[10px] font-bold px-1.5 py-0.5 rounded">
              LIVE
            </span>
          )}
        </div>
        <div className="flex items-center gap-1">
          <button
            onClick={() => setIsChatOpen(!isChatOpen)}
            className={cn("p-1.5 rounded hover:bg-hover transition-colors", isChatOpen && "text-twitch")}
            title="Toggle chat"
          >
            <MessageSquare className="w-4 h-4" />
          </button>
          <button
            onClick={toggleAlwaysOnTop}
            className={cn("p-1.5 rounded hover:bg-hover transition-colors", alwaysOnTop && "text-twitch")}
            title="Always on top"
          >
            {alwaysOnTop ? <Pin className="w-4 h-4" /> : <PinOff className="w-4 h-4" />}
          </button>
        </div>
      </header>

      <div className="flex flex-1 overflow-hidden">
        <main className="flex-1 bg-base flex flex-col relative overflow-hidden">
          <VideoPlayer
            channel={channel}
            userInfo={userInfo}
            isFullscreen={isFullscreen}
            setIsFullscreen={setIsFullscreen}
          />
        </main>

        {isChatOpen && (
          <Chat
            isOpen={isChatOpen}
            setIsOpen={setIsChatOpen}
            messages={chat.messages}
            emotes={allEmotes}
            globalBadges={globalBadges}
            channelBadges={channelBadges}
            isLoggedIn={isLoggedIn}
            isConnected={chat.isConnected}
            onSendMessage={chat.sendMessage}
          />
        )}
      </div>
    </div>
  );
}
