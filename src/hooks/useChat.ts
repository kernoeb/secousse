import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { info, debug, error as logError } from "@tauri-apps/plugin-log";
import type { ChatMessage } from "../types";

interface UseChatReturn {
  messages: ChatMessage[];
  isConnected: boolean;
  sendMessage: (message: string) => Promise<void>;
}

export function useChat(channel: string | null, isLoggedIn: boolean): UseChatReturn {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [isConnected, setIsConnected] = useState(false);
  const currentChannelRef = useRef<string | null>(null);
  const connectingRef = useRef<string | null>(null);

  // Track seen message IDs to prevent duplicates
  const seenIdsRef = useRef<Set<string>>(new Set());

  // Monotonic counter for messages whose IRC `id` tag is missing (rare, but
  // would otherwise produce duplicate React keys and force a full reconcile).
  const nextKeyRef = useRef(0);

  // Connect to chat when channel changes
  useEffect(() => {
    if (!channel) {
      currentChannelRef.current = null;
      connectingRef.current = null;
      setIsConnected(false);
      setMessages([]);
      seenIdsRef.current.clear();
      return;
    }

    // Guard against duplicate connections
    if (connectingRef.current === channel) {
      return;
    }
    connectingRef.current = channel;
    setIsConnected(false);

    info(`[useChat] Connecting to chat: ${channel}`);
    currentChannelRef.current = channel;
    setMessages([]);
    seenIdsRef.current.clear();

    invoke("connect_to_chat", { channel })
      .then(() => {
        setIsConnected(true);
      })
      .catch(err => {
        logError(`[useChat] Failed to connect to chat: ${err}`);
        connectingRef.current = null;
        setIsConnected(false);
      });
  }, [channel]);

  // `listen()` is async: if the cleanup fires before it resolves, the unlisten
  // function would be lost and the listener leaks. We capture the unlisten in a
  // local var and use a `cancelled` flag so an unresolved `listen()` is torn
  // down as soon as it returns.
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    listen<ChatMessage>("chat-message", (event) => {
      const newMsg = event.payload;

      if (newMsg.channel !== currentChannelRef.current) {
        debug(`[useChat] Ignoring message from #${newMsg.channel}, current is #${currentChannelRef.current}`);
        return;
      }

      if (newMsg.id && seenIdsRef.current.has(newMsg.id)) {
        debug(`[useChat] Skipping duplicate message ID: ${newMsg.id}`);
        return;
      }

      if (newMsg.id) {
        seenIdsRef.current.add(newMsg.id);
        if (seenIdsRef.current.size > 500) {
          const firstId = seenIdsRef.current.values().next().value;
          if (firstId) seenIdsRef.current.delete(firstId);
        }
      }

      const _renderKey = newMsg.id || `local-${nextKeyRef.current++}`;
      setMessages((prev) => [...prev, { ...newMsg, _renderKey, timestamp: Date.now() }].slice(-300));
    }).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    listen<string>("chat-disconnected", async (event) => {
      const disconnectedChannel = event.payload;
      info(`[useChat] Chat disconnected from: ${disconnectedChannel}`);

      if (disconnectedChannel === channel) {
        setIsConnected(false);
        info("[useChat] Attempting to reconnect...");
        await new Promise(resolve => setTimeout(resolve, 2000));

        if (disconnectedChannel === channel) {
          try {
            await invoke("connect_to_chat", { channel });
            setIsConnected(true);
            info("[useChat] Successfully reconnected");
          } catch (err) {
            logError(`[useChat] Failed to reconnect: ${err}`);
          }
        }
      }
    }).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [channel]);

  const sendMessage = useCallback(async (message: string) => {
    if (!message.trim() || !isLoggedIn || !isConnected) return;

    try {
      await invoke("send_chat_message", { message: message.trim() });
    } catch (err) {
      logError(`[useChat] Send message error: ${err}`);
    }
  }, [isLoggedIn, isConnected]);

  return {
    messages,
    isConnected,
    sendMessage,
  };
}
