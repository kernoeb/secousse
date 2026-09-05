import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { info, debug, error as logError } from "@tauri-apps/plugin-log";
import type { ChatMessage, ChatNotice } from "../types";

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
  const reconnectingRef = useRef<string | null>(null);

  // Track seen message IDs to prevent duplicates
  const seenIdsRef = useRef<Set<string>>(new Set());

  // Monotonic counter for messages whose IRC `id` tag is missing (rare, but
  // would otherwise produce duplicate React keys and force a full reconcile).
  const nextKeyRef = useRef(0);

  // Identifies this hook instance to the Rust chat registry. The window label
  // prefix lets a closed pop-out release everything it held in one sweep.
  const subscriberRef = useRef<string | null>(null);
  if (!subscriberRef.current) {
    subscriberRef.current = `${getCurrentWindow().label}:${crypto.randomUUID()}`;
  }

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

    // Guard against duplicate connections. Keyed on the login state too: an
    // anonymous connection cannot send, so logging in has to rebuild it.
    const connectKey = `${channel}:${isLoggedIn}`;
    if (connectingRef.current === connectKey) {
      return;
    }
    connectingRef.current = connectKey;
    setIsConnected(false);

    info(`[useChat] Connecting to chat: ${channel}`);
    currentChannelRef.current = channel.toLowerCase();
    setMessages([]);
    seenIdsRef.current.clear();

    const subscriber = subscriberRef.current!;
    const connected = invoke("connect_to_chat", { channel, subscriber })
      .then(() => {
        setIsConnected(true);
      })
      .catch(err => {
        logError(`[useChat] Failed to connect to chat: ${err}`);
        connectingRef.current = null;
        setIsConnected(false);
      });

    // Rust keeps the connection alive while another window still subscribes.
    // Clearing the guard lets a re-run for the same channel reconnect.
    return () => {
      connectingRef.current = null;
      // Chained on the connect: releasing a subscriber Rust has not registered
      // yet is a no-op, and would leave the connection with an id no window
      // can ever release.
      connected.then(() => invoke("disconnect_from_chat", { channel, subscriber }))
        .catch(err => logError(`[useChat] Failed to disconnect from chat: ${err}`));
    };
  }, [channel, isLoggedIn]);

  // `listen()` is async: if the cleanup fires before it resolves, the unlisten
  // function would be lost and the listener leaks. We capture the unlisten in a
  // local var and use a `cancelled` flag so an unresolved `listen()` is torn
  // down as soon as it returns.
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    listen<ChatMessage>("chat-message", (event) => {
      const newMsg = event.payload;

      if (newMsg.channel.toLowerCase() !== currentChannelRef.current) {
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

    listen<ChatNotice>("chat-notice", (event) => {
      const notice = event.payload;
      if (notice.channel.toLowerCase() !== currentChannelRef.current) return;

      info(`[useChat] Notice (${notice.msg_id || "unknown"}): ${notice.message}`);
      const _renderKey = `notice-${nextKeyRef.current++}`;
      setMessages((prev) => [...prev, {
        id: _renderKey,
        user: "",
        message: notice.message,
        badges: [],
        emotes: [],
        channel: notice.channel,
        timestamp: Date.now(),
        system: true,
        _renderKey,
      }].slice(-300));
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
    // Rust emits the normalized channel, so compare on the same footing.
    const target = channel?.toLowerCase() ?? null;

    listen<string>("chat-disconnected", async (event) => {
      const disconnectedChannel = event.payload.toLowerCase();
      info(`[useChat] Chat disconnected from: ${disconnectedChannel}`);

      if (cancelled || disconnectedChannel !== target) return;
      // Coalesce duplicates: the disconnect event is global, so every listener
      // in this window would otherwise race to rebuild the same connection.
      if (reconnectingRef.current === disconnectedChannel) return;
      reconnectingRef.current = disconnectedChannel;

      setIsConnected(false);
      info("[useChat] Attempting to reconnect...");
      try {
        await new Promise(resolve => setTimeout(resolve, 2000));
        if (cancelled || disconnectedChannel !== target) return;
        await invoke("connect_to_chat", { channel, subscriber: subscriberRef.current });
        if (cancelled) return;
        setIsConnected(true);
        info("[useChat] Successfully reconnected");
      } catch (err) {
        logError(`[useChat] Failed to reconnect: ${err}`);
      } finally {
        if (reconnectingRef.current === disconnectedChannel) {
          reconnectingRef.current = null;
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
    if (!message.trim() || !channel || !isLoggedIn || !isConnected) return;

    try {
      await invoke("send_chat_message", { channel, message: message.trim() });
    } catch (err) {
      logError(`[useChat] Send message error: ${err}`);
    }
  }, [channel, isLoggedIn, isConnected]);

  return {
    messages,
    isConnected,
    sendMessage,
  };
}
