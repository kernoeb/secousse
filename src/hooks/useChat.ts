import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { info, debug, error as logError } from "@tauri-apps/plugin-log";
import type { ChatMessage } from "../types";

interface UseChatReturn {
  messages: ChatMessage[];
  isAtBottom: boolean;
  chatContainerRef: React.RefObject<HTMLDivElement | null>;
  chatEndRef: React.RefObject<HTMLDivElement | null>;
  sendMessage: (message: string) => Promise<void>;
  handleScroll: () => void;
  scrollToBottom: () => void;
}

export function useChat(channel: string | null, isLoggedIn: boolean): UseChatReturn {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [isAtBottom, setIsAtBottom] = useState(true);
  const chatContainerRef = useRef<HTMLDivElement>(null);
  const chatEndRef = useRef<HTMLDivElement>(null);
  const currentChannelRef = useRef<string | null>(null);

  // Connect to chat when channel changes
  useEffect(() => {
    if (!channel) {
      currentChannelRef.current = null;
      setMessages([]);
      return;
    }

    info(`[useChat] Connecting to chat: ${channel}`);
    currentChannelRef.current = channel;
    setMessages([]);
    setIsAtBottom(true);

    invoke("connect_to_chat", { channel }).catch(err => {
      logError(`[useChat] Failed to connect to chat: ${err}`);
    });
  }, [channel]);

  // Listen for chat messages
  useEffect(() => {
    const unlisten = listen<ChatMessage>("chat-message", (event) => {
      const newMsg = event.payload;

      // Only accept messages from the current channel
      if (newMsg.channel !== currentChannelRef.current) {
        debug(`[useChat] Ignoring message from #${newMsg.channel}, current is #${currentChannelRef.current}`);
        return;
      }

      setMessages((prev) => {
        // Check for duplicate (same user, same message, within 100ms)
        const lastMsg = prev[prev.length - 1];
        if (lastMsg &&
            lastMsg.user === newMsg.user &&
            lastMsg.message === newMsg.message &&
            Date.now() - lastMsg.timestamp < 100) {
          return prev;
        }
        return [...prev, { ...newMsg, timestamp: Date.now() }].slice(-200);
      });
    });
    return () => { unlisten.then(f => f()); };
  }, []);

  // Handle chat disconnection and auto-reconnect
  useEffect(() => {
    const unlisten = listen<string>("chat-disconnected", async (event) => {
      const disconnectedChannel = event.payload;
      info(`[useChat] Chat disconnected from: ${disconnectedChannel}`);

      if (disconnectedChannel === channel) {
        info("[useChat] Attempting to reconnect...");
        await new Promise(resolve => setTimeout(resolve, 2000));

        if (disconnectedChannel === channel) {
          try {
            await invoke("connect_to_chat", { channel });
            info("[useChat] Successfully reconnected");
          } catch (err) {
            logError(`[useChat] Failed to reconnect: ${err}`);
          }
        }
      }
    });
    return () => { unlisten.then(f => f()); };
  }, [channel]);

  // Track if user manually scrolled
  const userScrolledRef = useRef(false);

  // Auto-scroll when at bottom and user hasn't manually scrolled up
  useEffect(() => {
    if (isAtBottom && !userScrolledRef.current) {
      chatEndRef.current?.scrollIntoView({ behavior: "auto" });
    }
  }, [messages, isAtBottom]);

  const handleScroll = useCallback(() => {
    const container = chatContainerRef.current;
    if (!container) return;

    const threshold = 100;
    const distanceFromBottom = container.scrollHeight - container.scrollTop - container.clientHeight;
    const isNearBottom = distanceFromBottom < threshold;
    
    // If user scrolled away from bottom, mark as manually scrolled
    if (!isNearBottom) {
      userScrolledRef.current = true;
    } else {
      userScrolledRef.current = false;
    }
    
    setIsAtBottom(isNearBottom);
  }, []);

  const scrollToBottom = useCallback(() => {
    userScrolledRef.current = false;
    chatEndRef.current?.scrollIntoView({ behavior: "smooth" });
    setIsAtBottom(true);
  }, []);

  const sendMessage = useCallback(async (message: string) => {
    if (!message.trim() || !isLoggedIn) return;

    try {
      await invoke("send_chat_message", { message: message.trim() });
    } catch (err) {
      logError(`[useChat] Send message error: ${err}`);
    }
  }, [isLoggedIn]);

  return {
    messages,
    isAtBottom,
    chatContainerRef,
    chatEndRef,
    sendMessage,
    handleScroll,
    scrollToBottom,
  };
}
