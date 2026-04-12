# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is this

Secousse is a desktop Twitch client built with Tauri 2 (Rust backend + React/TypeScript frontend). It streams live video via HLS, connects to IRC chat with emotes from 4 sources (Twitch, 7TV, BTTV, FFZ), and supports OAuth login for followed channels and chat.

## Commands

```bash
bun run tauri dev     # Dev mode (Vite HMR on :1420 + Rust backend)
bun run tauri build   # Production build → src-tauri/target/release/bundle/
npx tsc --noEmit      # Type-check frontend only
```

## Architecture

**Frontend ↔ Backend communication** uses two mechanisms:
- **Tauri commands** (RPC): frontend calls `invoke("command_name", {args})`, Rust handles via `#[tauri::command]` functions in `src-tauri/src/lib.rs`
- **Tauri events** (push): Rust emits events (`chat-message`, `chat-notice`, `chat-disconnected`, `login-success`), frontend listens with `listen()`

**State lives in App.tsx** — it's the single orchestrator. All data fetching happens through hooks (`useAuth`, `useChat`, `useEmotes`, `useSearch`, `useTopStreams`), and state flows down to components as props.

**UI state persistence** uses `localStorage` via wrapper functions in `src/lib/utils.ts` (channel, active tab, sidebar open, chat open). Credentials use Tauri's encrypted store plugin (`device_id`, `access_token`).

## Key architectural decisions

**Two Twitch API sources** in `src-tauri/src/twitch.rs`:
- GQL API (unauthenticated, internal client ID) — used for stream info, top streams, search, playback tokens
- Helix API (authenticated, app client ID + OAuth token) — used for self info, followed channels, follow/unfollow, emotes

**HLS streaming bypasses CORS** via `src/TauriHlsLoader.ts` — a custom HLS.js loader that routes segment/manifest fetches through `@tauri-apps/plugin-http` (native HTTP, no browser CORS restrictions).

**Chat uses raw IRC over WebSocket** (`src-tauri/src/chat.rs`). Connects to `wss://irc-ws.chat.twitch.tv:443`, parses PRIVMSG/NOTICE tags, emits structured events to frontend. Keepalive PING every 30s. Frontend auto-reconnects after 2s on disconnect.

**OAuth flow** starts a local HTTP server on `:17563` to capture the redirect token, then emits `login-success` to the frontend.

## Auto-refresh intervals

- Sidebar (followed channels or top streams): 60s
- Current stream info (viewers, title): 60s
- Spade watch analytics reporting: 60s (Rust-side background task)
- Chat PING keepalive: 30s

## Pause behavior

When the video is paused, HLS.js segment fetching is stopped (`hls.stopLoad()`) via `onPause`/`onPlay` video events to prevent buffer churn. The error handler guards `startLoad()` against `videoRef.current?.paused` to avoid silently resuming during intentional pause.
