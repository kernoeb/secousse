# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is this

Secousse is a desktop Twitch client built with Tauri 2 (Rust backend + React 19/TypeScript frontend, Vite + Tailwind v4). It streams live video via HLS, connects to IRC chat with emotes from 4 sources (Twitch, 7TV, BTTV, FFZ), and supports OAuth login for followed channels and chat.

App identifier: `com.kernoeb.secousse`. Package manager: `bun`.

## Commands

```bash
bun run tauri dev     # Dev mode (Vite HMR on :1420 + Rust backend)
bun run tauri build   # Production build → src-tauri/target/release/bundle/
bun run build         # tsc + vite build (frontend only, no bundling)
npx tsc --noEmit      # Type-check frontend only
bun run bump          # Bump version in package.json + Cargo.toml + tauri.conf.json
```

## Source layout

- `src-tauri/src/main.rs` — entrypoint, just calls `secousse_lib::run()`.
- `src-tauri/src/lib.rs` — `AppState`, all `#[tauri::command]` handlers, OAuth callback HTTP server on `:17563`, programmatic window creation, background spade task.
- `src-tauri/src/twitch.rs` — `TwitchClient` (GQL + Helix), HLS playback token, search, follow/unfollow, badges, spade analytics.
- `src-tauri/src/chat.rs` — IRC-over-WebSocket connection, message parsing.
- `src-tauri/src/emotes.rs` — 7TV/BTTV/FFZ fetchers (global + per-channel).
- `src/App.tsx` — single orchestrator, owns all top-level state.
- `src/components/` — `Navbar`, `Sidebar`, `VideoPlayer`, `Chat`, `StreamInfo`, `BrowseGrid`.
- `src/hooks/` — `useAuth`, `useChat`, `useEmotes`, `useSearch`, `useTopStreams`.
- `src/lib/utils.ts` — localStorage persistence helpers, viewer-count formatter, `cn()`, `AUTO_QUALITY` sentinel.
- `src/lib/spamSim.ts` — dev-only chat spam simulator (see "Dev tooling" below).
- `src/TauriHlsLoader.ts` — custom HLS.js loader using `@tauri-apps/plugin-http`.
- `src/types.ts` — all shared TS types (Twitch GQL/Helix shapes, chat, emotes, UI state).

## Architecture

**Frontend ↔ Backend communication** uses two mechanisms:
- **Tauri commands** (RPC): frontend calls `invoke("command_name", {args})`, Rust handles via `#[tauri::command]` functions registered in `lib.rs`'s `invoke_handler![…]`.
- **Tauri events** (push): Rust emits `chat-message`, `chat-notice`, `chat-disconnected`, `login-success`; frontend listens with `listen()`.

**State lives in `App.tsx`** — it's the single orchestrator. All data fetching happens through hooks, and state flows down to components as props. Components don't fetch.

**UI state persistence** uses `localStorage` via wrapper functions in `src/lib/utils.ts` (channel, active tab, sidebar open, chat open, volume, muted, preferred quality height). The currently selected channel is also kept in `sessionStorage` so per-tab state survives reloads but doesn't leak between windows. Credentials use Tauri's encrypted store plugin (`device_id`, `access_token` in `settings.bin`).

## Key architectural decisions

**Two Twitch API sources** in `src-tauri/src/twitch.rs`:
- GQL API (unauthenticated, internal client ID) — stream info, top streams, search, playback tokens, badges, spade analytics.
- Helix API (authenticated, app client ID + OAuth token) — self info, followed channels, follow/unfollow, Twitch emotes.

**HLS streaming bypasses CORS** via `src/TauriHlsLoader.ts` — a custom HLS.js loader that routes segment/manifest fetches through `@tauri-apps/plugin-http` (native HTTP, no browser CORS restrictions). The plugin is configured with the `unsafe-headers` Cargo feature in `src-tauri/Cargo.toml`; without it, Twitch's HLS edge silently strips Origin/Referer/User-Agent headers and rejects the request.

**Chat uses raw IRC over WebSocket** (`src-tauri/src/chat.rs`). Connects to `wss://irc-ws.chat.twitch.tv:443`, parses PRIVMSG/NOTICE tags, emits structured events to frontend. Keepalive PING every 30s. Frontend auto-reconnects after 2s on `chat-disconnected`. `useChat` dedupes by IRC `id` tag (LRU of 500) and falls back to a monotonic counter for messages with no id (rare). The message buffer is capped at 300.

**OAuth flow** opens `id.twitch.tv/oauth2/authorize` in the system browser, then a local HTTP server on `:17563` serves a JS page that extracts the token from the URL fragment and POSTs it back. Rust then updates `TwitchClient`, persists the token, and emits `login-success` to the frontend.

**Window creation** is programmatic in `setup()` (not via `tauri.conf.json` window list) so we can apply a transparent macOS title bar and custom NSColor background. The window starts hidden and is shown either when the frontend invokes `show_main_window` (post-paint) or by a 2s safety-fallback timer if the JS never runs.

**Logging** goes through `tauri-plugin-log`: stdout + a rotating file in the platform log dir (`~/Library/Logs/<id>` on macOS). Capped at 5 MB with `KeepOne` rotation. Frontend logs via `@tauri-apps/plugin-log` (`info`, `debug`, `error`); `attachConsole()` in `App.tsx` forwards `console.*` calls to the Rust log too.

## Auto-refresh intervals

- Sidebar (followed channels or top streams): 60s (frontend `setInterval`).
- Current stream info (viewers, title): 60s (frontend `setInterval`).
- Spade watch analytics reporting: 60s (Rust background task in `lib.rs::run`'s `setup()`).
- Chat PING keepalive: 30s.

## Pause behavior

When the video is paused, HLS.js segment fetching is stopped (`hls.stopLoad()`) via `onPause`/`onPlay` video events to prevent buffer churn. The error handler guards `startLoad()` against `videoRef.current?.paused` to avoid silently resuming during intentional pause.

## Dev tooling

- `src/lib/spamSim.ts` exposes `window.__spam.start({rate, emotesPerMsg, durationSec})` / `__spam.stop()` in dev builds, useful for stress-testing the chat renderer.
- Setting `VITE_AUTOSPAM='{"rate":50}'` (any JSON of spam options) auto-starts the simulator 3s after a channel + emotes are ready.

## Release workflow

GitHub Actions (`.github/workflows/release.yml`) uses `tauri-action`. The pipeline runs a serial `create-release` job that emits a `releaseId` consumed by the per-OS build matrix — this avoids the parallel-job race that otherwise creates duplicate draft releases. Don't refactor the matrix to create the release inline.
