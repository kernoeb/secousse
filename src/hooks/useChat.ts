import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { info, debug, error as logError } from "@tauri-apps/plugin-log";
import type { ChatClear, ChatDeletion, ChatMessage, ChatNotice, ChatReplyParent } from "../types";

// A dropped socket is usually back within seconds, so try once quickly, then
// settle into a slower loop for as long as the channel stays open.
const FIRST_RECONNECT_MS = 2000;
const RETRY_DELAY_MS = 10_000;

interface UseChatReturn {
  messages: ChatMessage[];
  isConnected: boolean;
  sendMessage: (message: string, reply?: ChatReplyParent) => Promise<void>;
}

function deletionKind(clear: ChatClear): ChatDeletion {
  if (clear.target_msg_id) return "message";
  if (!clear.target_user_id && !clear.target_login) return "clear";
  return clear.ban_duration != null ? "timeout" : "ban";
}

export function useChat(channel: string | null, isLoggedIn: boolean, isAuthResolved: boolean): UseChatReturn {
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

  // Id of the live connection attempt, so the reconnect listener releases and
  // rebuilds the same one. Assigned by the connect effect below.
  const subscriberRef = useRef<string | null>(null);

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

    // Connecting before the token check lands would build an anonymous socket
    // and rebuild it a moment later, closing and reopening the IRC connection.
    if (!isAuthResolved) return;

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

    // One id per run, never per hook instance: Rust keeps subscribers in a set,
    // so two runs sharing an id count as one and the first cleanup would close
    // the connection the second run is still using.
    const subscriber = `${getCurrentWindow().label}:${crypto.randomUUID()}`;
    subscriberRef.current = subscriber;
    let cancelled = false;
    let retryTimer: ReturnType<typeof setTimeout> | undefined;

    const release = () => invoke("disconnect_from_chat", { channel, subscriber })
      .catch(err => logError(`[useChat] Failed to disconnect from chat: ${err}`));

    const attempt = async () => {
      try {
        await invoke("connect_to_chat", { channel, subscriber });
        // The effect was torn down while Rust was registering us, so its own
        // release ran too early: release again now that the id exists.
        if (cancelled) {
          release();
          return;
        }
        setIsConnected(true);
      } catch (err) {
        logError(`[useChat] Failed to connect to chat: ${err}`);
        if (cancelled) return;
        setIsConnected(false);
        retryTimer = setTimeout(attempt, RETRY_DELAY_MS);
      }
    };
    attempt();

    // Rust keeps the connection alive while another window still subscribes.
    // Clearing the guard lets a re-run for the same channel reconnect.
    return () => {
      cancelled = true;
      connectingRef.current = null;
      clearTimeout(retryTimer);
      // Unconditional: releasing an id Rust never registered is a no-op, and
      // the reconnect listener may have registered ours without us knowing.
      release();
    };
  }, [channel, isLoggedIn, isAuthResolved]);

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

  // Deleted messages stay in the list, greyed out. Only the matching rows get
  // a new object, so the memoised rows of the others do not re-render.
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    listen<ChatClear>("chat-clear", (event) => {
      const clear = event.payload;
      if (clear.channel.toLowerCase() !== currentChannelRef.current) return;

      const kind = deletionKind(clear);
      const login = clear.target_login?.toLowerCase();
      // Our own echoes carry no user id, so a timeout of ourselves matches on the login.
      const matches = (m: ChatMessage) => {
        if (m.system || m.deleted) return false;
        if (kind === "clear") return true;
        if (kind === "message") return m.id === clear.target_msg_id;
        return (!!clear.target_user_id && m.user_id === clear.target_user_id) || (!!login && m.login === login);
      };
      info(`[useChat] Clear (${kind}) on #${clear.channel}`);
      setMessages((prev) => {
        let changed = false;
        const next = prev.map((m) => {
          if (!matches(m)) return m;
          changed = true;
          return { ...m, deleted: kind };
        });
        return changed ? next : prev;
      });
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
        for (let attempt = 0; ; attempt++) {
          const wait = attempt === 0 ? FIRST_RECONNECT_MS : RETRY_DELAY_MS;
          await new Promise(resolve => setTimeout(resolve, wait));
          if (cancelled || disconnectedChannel !== target) return;
          try {
            await invoke("connect_to_chat", { channel, subscriber: subscriberRef.current });
            if (cancelled) return;
            setIsConnected(true);
            info("[useChat] Successfully reconnected");
            return;
          } catch (err) {
            logError(`[useChat] Failed to reconnect: ${err}`);
          }
        }
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

  const sendMessage = useCallback(async (message: string, reply?: ChatReplyParent) => {
    let text = message.trim();
    if (!text || !channel || !isLoggedIn || !isConnected) return;

    // Twitch's own client starts a reply with the parent's mention, and other
    // clients rely on it to show who the reply is for.
    const mention = reply ? `@${reply.user_login} ` : "";
    if (mention && !text.startsWith("/") && !text.toLowerCase().startsWith(mention.toLowerCase())) {
      text = mention + text;
    }

    try {
      await invoke("send_chat_message", { channel, message: text, reply: reply ?? null });
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
