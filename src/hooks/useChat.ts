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

  // Track seen message IDs to prevent duplicates
  const seenIdsRef = useRef<Set<string>>(new Set());

  // Connect to chat when channel changes
  useEffect(() => {
    if (!channel) {
      currentChannelRef.current = null;
      setMessages([]);
      seenIdsRef.current.clear();
      return;
    }

    info(`[useChat] Connecting to chat: ${channel}`);
    currentChannelRef.current = channel;
    setMessages([]);
    seenIdsRef.current.clear();
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

      // Skip if we've already seen this message ID
      if (newMsg.id && seenIdsRef.current.has(newMsg.id)) {
        debug(`[useChat] Skipping duplicate message ID: ${newMsg.id}`);
        return;
      }

      // Add to seen IDs (keep last 500 to prevent memory growth)
      if (newMsg.id) {
        seenIdsRef.current.add(newMsg.id);
        if (seenIdsRef.current.size > 500) {
          const firstId = seenIdsRef.current.values().next().value;
          if (firstId) seenIdsRef.current.delete(firstId);
        }
      }

      setMessages((prev) => [...prev, { ...newMsg, timestamp: Date.now() }].slice(-200));
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
