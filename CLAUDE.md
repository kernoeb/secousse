# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is this

Secousse is a desktop Twitch client built with Tauri 2 (Rust backend + React 19/TypeScript frontend, Vite + Tailwind v4). It streams live video via HLS, connects to IRC chat with emotes from 4 sources (Twitch, 7TV, BTTV, FFZ), supports OAuth login for followed channels and chat, and offers a multi-stream grid (up to 4 tiles) plus detachable pop-out windows.

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
- `src/main.tsx` — entry router: reads `?popout=<channel>` from `window.location` and renders `<PopoutApp>` if present, otherwise `<App>`.
- `src/App.tsx` — single orchestrator for the main window, owns the grid state (`channels: string[]` + `focusedIndex`) and all top-level UI state.
- `src/PopoutApp.tsx` — minimal shell for detached stream windows. Renders one `<VideoPlayer>` + a toggleable `<Chat>` and an always-on-top pin in the header. No sidebar/browse/search.
- `src/components/` — `Navbar`, `Sidebar`, `VideoPlayer`, `StreamGrid` (CSS-grid layout for 1–4 tiles), `Chat`, `StreamInfo`, `BrowseGrid`, `ChannelActionButtons` (shared add-to-grid / pop-out / remove icon cluster used by sidebar rows, browse cards, and grid tile overlays).
- `src/hooks/` — `useAuth`, `useChat`, `useEmotes`, `useSearch`, `useTopStreams`, `useUserInfo` (per-channel `get_user_info` poller; each `StreamGridTile` and `PopoutApp` instantiates its own).
- `src/lib/utils.ts` — localStorage persistence helpers, viewer-count formatter, `cn()`, `AUTO_QUALITY` sentinel, `getPopoutChannel()`, `GRID_MAX_TILES`.
- `src/lib/spamSim.ts` — dev-only chat spam simulator (see "Dev tooling" below).
- `src/TauriHlsLoader.ts` — custom progressive HLS.js loader using `@tauri-apps/plugin-http`, with Twitch low-latency prefetch promotion (see "HLS streaming" below).
- `src/types.ts` — all shared TS types (Twitch GQL/Helix shapes, chat, emotes, UI state).

## Architecture

**Frontend ↔ Backend communication** uses two mechanisms:
- **Tauri commands** (RPC): frontend calls `invoke("command_name", {args})`, Rust handles via `#[tauri::command]` functions registered in `lib.rs`'s `invoke_handler![…]`.
- **Tauri events** (push): Rust emits `chat-message`, `chat-notice`, `chat-disconnected`, `login-success`; frontend listens with `listen()`.

**State lives in `App.tsx`** — it's the single orchestrator for the main window. All data fetching happens through hooks, and state flows down to components as props. The exception is `useUserInfo`: each `StreamGridTile` (and `PopoutApp`) calls it directly so tiles own their own per-channel polling without prop-drilling from the orchestrator. This trades one extra `get_user_info` RPC/min for the focused channel against avoiding HLS-rebuild bugs that previously occurred when sharing the fetch.

**Multi-stream grid.** `App.tsx` stores `channels: string[]` + `focusedIndex: number`. `<StreamGrid>` lays them out in a CSS grid (1, 2×1, 3×1, or 2×2 depending on count, cap `GRID_MAX_TILES = 4`). Audio is focused-only: each `<VideoPlayer>` receives `forceMuted={isMulti && !isFocused}` and the focused tile owns chat + stream info. Adding via "Shift+click" or the `<ChannelActionButtons>` `+` icon; when the grid is full, the add action falls back to opening a pop-out. Layout is persisted (`gridChannels` + `focusedIndex` in localStorage); single-stream usage is just `channels.length === 1`.

**Pop-out windows.** `open_popout` (Rust) builds a `WebviewWindowBuilder` with label `popout-<channel>`, idempotent — re-invoking on the same channel `unminimize`s + `set_focus`es the existing window. The URL is `index.html?popout=<channel>`, which `src/main.tsx` routes to `<PopoutApp>`. Capabilities use the `popout-*` glob so the popout windows inherit the main window's permissions. The OAuth token and emote cache are shared (Tauri store + frontend in-memory respectively are global to the WebView process).

**UI state persistence** uses `localStorage` via wrapper functions in `src/lib/utils.ts` (channel, active tab, sidebar open, chat open, volume, muted, preferred quality height, `gridChannels`, `focusedIndex`). The currently selected channel is also kept in `sessionStorage` so per-tab state survives reloads but doesn't leak between windows. Credentials use Tauri's store plugin (`device_id`, `access_token` in `settings.bin`). That file is **plaintext JSON**, not encrypted — `cat` shows the token. Treat it as a secret on disk, and encrypt it before storing anything more sensitive than the current scoped OAuth token.

## Key architectural decisions

**Two Twitch API sources** in `src-tauri/src/twitch.rs`:
- GQL API (internal client ID, always unauthenticated) — stream info, top streams, search, playback tokens, badges, spade analytics, and the follow/unfollow mutations. The endpoint is `https://gql.twitch.tv/gql` with **no trailing slash**: `/gql/` returns a `text/plain` 404, which surfaces from reqwest as the misleading `error decoding response body` because `res.json()` reports serde failures with that same message.
- Helix API (app client ID + OAuth token) — self info, followed channels, follow status, Twitch emotes.

**GQL cannot be authenticated from this app.** Checked against the live API, every combination is rejected: our app client ID gives `400 The "Client-ID" header is invalid`, and our OAuth token gives `401 The "Authorization" token is invalid` with either client ID, on any operation — even a read-only `{currentUser{login}}`. GQL accepts only Twitch's own first-party client IDs together with tokens minted for them, which is why `gql_headers()` sends no `Authorization`. Two consequences worth knowing before you touch either area:

- **`follow_user` and `unfollow_user` cannot work.** Both post a GQL mutation with `Authorization: OAuth <token>`, which is the combination that returns 401. `check_follow_status` goes through Helix and does work, so the UI can read follow state but not change it.
- **Video is capped at 1080p.** An anonymous playback token carries `maximum_resolution: FULL_HD` and says why: `maximum_resolution_reasons: {QUAD_HD: [AUTHZ_NOT_LOGGED_IN], ULTRA_HD: [AUTHZ_NOT_LOGGED_IN]}`. A logged-in twitch.tv session gets `ULTRA_HD`. 1440p needs a second thing too: `supported_codecs` including `h265` on the usher URL, because the 1440p rendition is HEVC (`hev1.1.2.L150.90`) and `get_usher_url` sends no `supported_codecs` at all. Adding that alone changes nothing while the token stays anonymous. The WebView itself is not the limit — `MediaSource.isTypeSupported` returns true for `hev1`/`hvc1` and false for AV1.

**HLS streaming bypasses CORS** via `src/TauriHlsLoader.ts` — a custom HLS.js loader that routes segment/manifest fetches through `@tauri-apps/plugin-http` (native HTTP, no browser CORS restrictions). The plugin is configured with the `unsafe-headers` Cargo feature in `src-tauri/Cargo.toml`; without it, Twitch's HLS edge silently strips Origin/Referer/User-Agent headers and rejects the request.

**HLS playback tuning** is split between `TauriHlsLoader.ts` (the loader) and the `new Hls({...})` config in `VideoPlayer.tsx` (the flags). Steady state on a live channel is ~4.4 s behind the encoder with no buffer errors; if you see repeated `bufferStalledError` or `bufferSeekOverHole`, something below has been undone. Three things matter:
1. **Early segment fetch (progressive feeding stays OFF).** Segment bodies are read chunk-by-chunk via `ReadableStream.getReader()` so `stats.loaded` tracks a chunked-transfer prefetch as it trickles in, but the bytes are handed to hls.js as one complete segment. Do **not** set `hls.config.progressive = true`: hls.js' `enableStreamingMode()` refuses progressive for custom loaders, and measurement confirmed the refusal is right. Twitch now serves **fMP4** segments (`moof`/`mdat`, no MPEG-TS sync bytes), and feeding those to the transmuxer in pieces yields fragments whose parsed duration is almost never the 2.000 s the playlist declares. The mis-timed appends shatter the media buffer into as many as 9 ranges and the player then chases holes it just created. Three matched pairs of 130 s runs: progressive on gave 8-20 buffer errors per run and 5.9 s median latency; progressive off gave 0-1 errors, 100% of fragments at the declared 2.000 s, and 4.4 s median latency. Progressive feeding made playback both less stable *and* slower.
2. **`promotePrefetches()` playlist rewrite.** Twitch's `#EXT-X-TWITCH-PREFETCH:URL` tag (their proprietary low-latency mechanism, not standard LL-HLS) announces future segment URLs while the encoder is still producing them; the CDN serves them via HTTP chunked transfer encoding. hls.js ignores the tag, so the loader rewrites each prefetch line into a standard `#EXTINF:dur,live\nURL` pair before handing the playlist to hls.js. The first byte of a prefetch arrives ~30 ms after the request; the rest streams over ~2 s of remaining encode time. Twitch signs each prefetch URL separately, so the same media reappears as a real `#EXTINF` under a different path at the same media sequence number. hls.js compares URLs per sequence number and raises a non-fatal `levelParsingError` on every refresh; that error path returns before arming the live reload timer, so playlist refreshes degrade to error-retry backoff. The player sets `ignorePlaylistParsingErrors: true` to silence it — do not remove it. The flag only suppresses the event: `mapFragmentIntersection` still returns early at the prefetch tail, so the live merge stays incomplete.
3. **TTFB-correct stats.** `stats.loading.first` is set right after `await fetch()` returns (headers received, body not yet read), not after `arrayBuffer()`. The previous behavior had `first === end`, which fed the full transfer duration to hls.js' TTFB estimator and inflated bandwidth estimates so ABR pinned to the top level regardless of link capacity.

Do **not** enable `lowLatencyMode: true` in the Hls config — it expects standard LL-HLS markers (`#EXT-X-PART-INF`) that Twitch doesn't emit, and activates aggressive catch-up seeks that cause `bufferStalledError` on the slightest segment variance.

**AbortController lifecycle in `TauriHlsLoader`.** Each `doFetch()` owns a fresh `AbortController` stored as `currentAbort`. The reference is cleared (via `releaseAbort(abort)`) the moment a body is fully consumed — at that point plugin-http has already released the Rust-side response resource. Without the clear, a later `loader.abort()` (from a channel switch or destroy) would call `dropBody()` on a freed `rid`, producing "resource id N is invalid" unhandled rejections (one per segment). `releaseAbort` uses an identity check (`if (this.currentAbort === abort)`) to guard a narrow race where a fetch completes after an external `load()` has already replaced the controller.

**Chat uses raw IRC over WebSocket** (`src-tauri/src/chat.rs`). Connects to `wss://irc-ws.chat.twitch.tv:443`, parses PRIVMSG/NOTICE tags, emits structured events to frontend. Keepalive PING every 30s. Frontend auto-reconnects after 2s on `chat-disconnected`. `useChat` dedupes by IRC `id` tag (LRU of 500) and falls back to a monotonic counter for messages with no id (rare). The message buffer is capped at 300.

**Chat is single-slot in Rust.** `AppState` holds one chat handle, so `connect_to_chat(channel)` aborts the previous connection. Implication for multi-window: opening a pop-out with its chat toggled on will kill the main window's chat. The current UX accepts this trade-off — refactor to a per-channel `HashMap` only if it becomes a real friction point.

**Emote cache is per-channel and LRU.** `useEmotes` keeps a `Map<channelId, ChannelEmoteEntry>` in a ref (cap 5, oldest evicted), with in-flight request dedup. The exported `allEmotes`/`channelBadges` are derived from the *focused* channel id so the chat renderer always sees the right set without re-fetching when the user shifts focus inside the grid. `loadChannelEmotes` is idempotent on cache hit (LRU bump only) — callers don't need their own load-once sentinel.

**OAuth flow** opens `id.twitch.tv/oauth2/authorize` in the system browser, then a local HTTP server on `:17563` serves a JS page that extracts the token from the URL fragment and POSTs it back. Rust then updates `TwitchClient`, persists the token, and emits `login-success` to the frontend.

**Window creation** is programmatic in `setup()` (not via `tauri.conf.json` window list) so we can apply a transparent macOS title bar and custom NSColor background. The window starts hidden and is shown either when the frontend invokes `show_main_window` (post-paint) or by a 2s safety-fallback timer if the JS never runs. Pop-out windows go through the same `WebviewWindowBuilder` path in the `open_popout` command (transparent title bar on macOS, label `popout-<channel>`, capability glob `popout-*`).

**AppKit must be touched from the main thread.** `apply_dark_titlebar` wraps its `setAppearance:` / `setBackgroundColor_` calls in `window.run_on_main_thread(...)`. `open_popout` is an `async` command, so Tauri runs it on a tokio worker; macOS 15 traps (`EXC_BREAKPOINT`, "Must only be used from the main thread") instead of tolerating the off-thread call, and the whole app dies with SIGTRAP. The `setup()` call site is already on the main thread, so before the fix only the pop-out path crashed. Same rule for any new Cocoa call added from a command handler.

**Logging** goes through `tauri-plugin-log`: stdout + a rotating file in the platform log dir (`~/Library/Logs/<id>` on macOS). Capped at 5 MB with `KeepOne` rotation. Frontend logs via `@tauri-apps/plugin-log` (`info`, `debug`, `error`); `attachConsole()` in `App.tsx` forwards `console.*` calls to the Rust log too.

## Auto-refresh intervals

- Sidebar (followed channels or top streams): 60s (frontend `setInterval`).
- Per-channel `get_user_info` poll: 60s, owned by `useUserInfo` (one instance per tile + one per `PopoutApp`; the focused channel is polled twice — once at the `App.tsx` level and once by its tile — by design).
- Spade watch analytics ping (`update_watch_state`): 60s (frontend interval gated on `isLoggedIn && userInfo.stream`).
- Chat PING keepalive: 30s.

## Pause behavior

When the video is paused, HLS.js segment fetching is stopped (`hls.stopLoad()`) via `onPause`/`onPlay` video events to prevent buffer churn. The error handler guards `startLoad()` against `videoRef.current?.paused` to avoid silently resuming during intentional pause.

## Dev tooling

- `src/lib/spamSim.ts` exposes `window.__spam.start({rate, emotesPerMsg, durationSec})` / `__spam.stop()` in dev builds, useful for stress-testing the chat renderer.
- Setting `VITE_AUTOSPAM='{"rate":50}'` (any JSON of spam options) auto-starts the simulator 3s after a channel + emotes are ready.

## Release workflow

GitHub Actions (`.github/workflows/release.yml`) uses `tauri-action`. The pipeline runs a serial `create-release` job that emits a `releaseId` consumed by the per-OS build matrix — this avoids the parallel-job race that otherwise creates duplicate draft releases. Don't refactor the matrix to create the release inline.
