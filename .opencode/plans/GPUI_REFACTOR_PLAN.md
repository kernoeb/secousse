# Secousse GPUI Refactor Plan

## Executive Summary

This document outlines the complete refactoring of **Secousse** from a Tauri/React/Tailwind stack to a pure Rust application using **GPUI** - the GPU-accelerated UI framework from Zed. This is a **complete rewrite** that will port all features while eliminating the JavaScript/web layer entirely.

---

## Table of Contents

1. [Current Architecture Analysis](#1-current-architecture-analysis)
2. [GPUI Framework Overview](#2-gpui-framework-overview)
3. [Feature Mapping](#3-feature-mapping)
4. [New Architecture Design](#4-new-architecture-design)
5. [Implementation Phases](#5-implementation-phases)
6. [Technical Deep Dives](#6-technical-deep-dives)
7. [Dependencies](#7-dependencies)
8. [Platform Considerations](#8-platform-considerations)
9. [Risk Assessment](#9-risk-assessment)
10. [Timeline Estimate](#10-timeline-estimate)

---

## 1. Current Architecture Analysis

### 1.1 Technology Stack (Current)

| Layer | Technology | Purpose |
|-------|------------|---------|
| Desktop Runtime | Tauri 2.0 | Native wrapper, IPC bridge |
| Frontend | React 19 | UI rendering |
| Build Tool | Vite 7 | Frontend bundling |
| Styling | Tailwind CSS 4 | Utility-first CSS |
| Backend | Rust | API client, WebSocket, business logic |

### 1.2 Current Features (Complete List)

#### Video Playback
- [x] HLS live stream playback via hls.js
- [x] Custom TauriHlsLoader for CORS bypass (proxies through Rust)
- [x] Quality selection (auto + manual levels: 1080p60, 720p60, etc.)
- [x] Play/Pause, Volume control with slider
- [x] Fullscreen toggle (CSS-based)
- [x] Live viewer count display
- [x] Offline channel detection with profile picture

#### Chat System
- [x] Real-time IRC WebSocket connection to `irc-ws.chat.twitch.tv`
- [x] Anonymous (justinfan) and authenticated modes
- [x] Send messages when logged in
- [x] Emote rendering inline in messages
- [x] Badge display (subscriber, moderator, VIP, broadcaster, bits)
- [x] Username colors
- [x] Auto-scroll with manual scroll detection
- [x] "Scroll to bottom" button when scrolled up
- [x] Auto-reconnect on disconnect

#### Authentication
- [x] OAuth 2.0 implicit flow via system browser
- [x] Local HTTP callback server (port 17563)
- [x] Token persistence via Tauri Store plugin
- [x] Token validation on startup
- [x] Login/Logout functionality

#### Following System
- [x] View followed live channels in sidebar
- [x] Follow/Unfollow channels
- [x] Auto-refresh every 60 seconds

#### Browse/Discovery
- [x] Top 30 live streams grid
- [x] Stream preview thumbnails (with size templating)
- [x] Viewer count, game category display
- [x] Auto-refresh every 60 seconds

#### Search
- [x] Debounced channel search (300ms)
- [x] Live/offline status indicators
- [x] Quick channel navigation
- [x] Click-outside to dismiss

#### Emote Support
- [x] Twitch global emotes
- [x] Twitch channel/subscriber emotes
- [x] 7TV global and channel emotes
- [x] BTTV global and channel emotes
- [x] FFZ channel emotes

#### Badge Support
- [x] Global Twitch badges
- [x] Channel-specific badges (subscriber tiers, bits)

#### Analytics
- [x] Spade events (minute-watched) every 60 seconds
- [x] Contributes to viewership statistics

#### UI Components
- [x] Navbar (logo, tabs, search, login, profile)
- [x] Sidebar (collapsible, channel list with viewer counts)
- [x] Video player (custom controls overlay)
- [x] Chat panel (resizable concept, message list, input)
- [x] Stream info bar (avatar, title, game, follow button)
- [x] Browse grid (responsive card layout)

### 1.3 Current Data Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                    FRONTEND (React/TypeScript)                  │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐            │
│  │ useAuth │  │ useChat │  │useEmotes│  │useSearch│   ...      │
│  └────┬────┘  └────┬────┘  └────┬────┘  └────┬────┘            │
│       │            │            │            │                  │
│       └────────────┴────────────┴────────────┘                  │
│                         │                                       │
│              invoke() / listen()                                │
└─────────────────────────┼───────────────────────────────────────┘
                          │ IPC
┌─────────────────────────┼───────────────────────────────────────┐
│                    BACKEND (Rust/Tauri)                         │
│  ┌────────────────────────────────────────────────────────┐    │
│  │                    AppState (Mutex)                     │    │
│  │  - twitch_client: TwitchClient                          │    │
│  │  - chat_handle: JoinHandle                              │    │
│  │  - chat_sender: mpsc::Sender                            │    │
│  │  - watch_state: WatchState                              │    │
│  │  - cached_username: Option<String>                      │    │
│  └────────────────────────────────────────────────────────┘    │
│                                                                 │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐                      │
│  │twitch.rs │  │ chat.rs  │  │emotes.rs │                      │
│  │GQL/Helix │  │WebSocket │  │7TV/BTTV  │                      │
│  └──────────┘  └──────────┘  └──────────┘                      │
└─────────────────────────────────────────────────────────────────┘
```

---

## 2. GPUI Framework Overview

### 2.1 What is GPUI?

GPUI is a **hybrid immediate/retained mode, GPU-accelerated UI framework** for Rust, created by Zed Industries for the Zed editor. Key characteristics:

- **GPU Rendering**: Uses Metal (macOS), Vulkan (Linux), Direct3D (Windows)
- **Reactive Model**: Entity-based state management with observers
- **Tailwind-style API**: Familiar styling with `.flex()`, `.bg()`, `.p_4()`, etc.
- **High Performance**: Designed for 120fps editor rendering
- **Pure Rust**: No JavaScript, no web views, no CSS

### 2.2 Core Concepts

#### Entities (`Entity<T>`)
- Application state containers owned by the `App` context
- Similar to React's `useState` but with explicit ownership
- Can be observed and emit events

```rust
// Creating an entity
let counter: Entity<Counter> = cx.new(|_| Counter { count: 0 });

// Updating an entity
counter.update(cx, |counter, cx| {
    counter.count += 1;
    cx.notify(); // Trigger re-render for observers
});

// Reading an entity
let count = counter.read(cx).count;
```

#### Views (Entities that implement `Render`)
- Entities that can produce UI
- Re-render when `notify()` is called

```rust
struct MyView {
    label: SharedString,
}

impl Render for MyView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .p_4()
            .bg(rgb(0x1a1a1a))
            .child(self.label.clone())
    }
}
```

#### Elements (Building Blocks)
- `div()` - The swiss-army knife element
- `img()` - Image rendering
- `svg()` - SVG rendering
- `uniform_list()` - Virtualized lists
- Custom elements via `Element` trait

#### Contexts
- `App` - Global application state
- `Context<T>` - Entity-specific context with notify/emit
- `Window` - Window state and operations
- `AsyncApp` - For async operations across await points

#### Actions (Keyboard Shortcuts)
```rust
#[gpui::action]
struct MoveUp;

actions!(menu, [MoveUp, MoveDown, Select]);

// Binding in render
div()
    .key_context("menu")
    .on_action(|_: &MoveUp, window, cx| { /* handle */ })
```

### 2.3 Styling API (Tailwind-like)

```rust
div()
    // Layout
    .flex()
    .flex_col()
    .flex_row()
    .gap_2()
    .p_4()
    .m_2()
    .w_full()
    .h_64()
    .size_8()
    
    // Colors
    .bg(rgb(0x1a1a1a))
    .text_color(rgb(0xffffff))
    .border_color(rgb(0x3f3f46))
    
    // Borders
    .border_1()
    .rounded_md()
    .rounded_full()
    
    // Effects
    .shadow_lg()
    .opacity(0.5)
    
    // Interactions
    .cursor_pointer()
    .hover(|style| style.bg(rgb(0x2f2f35)))
    .on_click(|event, window, cx| { /* handle */ })
    .on_mouse_down(MouseButton::Left, |event, window, cx| { /* handle */ })
```

### 2.4 Async Operations

```rust
// Spawn async task
cx.spawn(|this, mut cx| async move {
    let result = fetch_data().await;
    this.update(&mut cx, |this, cx| {
        this.data = result;
        cx.notify();
    }).ok();
}).detach();

// With background executor
cx.background_executor().spawn(async {
    heavy_computation()
}).detach();
```

### 2.5 Event System

```rust
// Define event
struct DataLoaded {
    items: Vec<Item>,
}

// Implement emitter
impl EventEmitter<DataLoaded> for MyModel {}

// Emit event
cx.emit(DataLoaded { items });

// Subscribe to events
cx.subscribe(&model, |this, _model, event: &DataLoaded, cx| {
    this.items = event.items.clone();
    cx.notify();
}).detach();
```

---

## 3. Feature Mapping

### 3.1 React Hooks → GPUI Entities

| React Hook | GPUI Equivalent | Notes |
|------------|-----------------|-------|
| `useAuth` | `Entity<AuthState>` | Global app state |
| `useChat` | `Entity<ChatState>` | Per-channel, with WebSocket task |
| `useEmotes` | `Entity<EmoteCache>` | Shared cache, lazy loading |
| `useSearch` | `Entity<SearchState>` | Debounced with timer |
| `useTopStreams` | `Entity<BrowseState>` | Cached with refresh |

### 3.2 React Components → GPUI Views

| React Component | GPUI Equivalent | Complexity |
|-----------------|-----------------|------------|
| `App.tsx` | `SecousseApp` (root view) | High |
| `Navbar.tsx` | `NavbarView` | Medium |
| `Sidebar.tsx` | `SidebarView` | Medium |
| `VideoPlayer.tsx` | `VideoPlayerView` + gpui-video-player | High |
| `Chat.tsx` | `ChatView` with `uniform_list` | High |
| `StreamInfo.tsx` | `StreamInfoView` | Low |
| `BrowseGrid.tsx` | `BrowseGridView` | Medium |

### 3.3 State Management Comparison

**React (Current)**:
```typescript
const [channel, setChannel] = useState<string | null>(null);
const [isLoggedIn, setIsLoggedIn] = useState(false);

useEffect(() => {
    // Side effects
}, [channel]);
```

**GPUI (New)**:
```rust
struct AppState {
    channel: Option<String>,
    auth: Entity<AuthState>,
}

impl AppState {
    fn set_channel(&mut self, channel: Option<String>, cx: &mut Context<Self>) {
        self.channel = channel;
        cx.notify();
        // Trigger side effects via observers
    }
}

// Observers react to changes
cx.observe(&app_state, |this, app_state, cx| {
    if let Some(channel) = &app_state.read(cx).channel {
        this.load_channel_data(channel, cx);
    }
}).detach();
```

---

## 4. New Architecture Design

### 4.1 Project Structure

```
secousse/
├── Cargo.toml
├── build.rs                    # Build-time asset embedding
├── assets/
│   ├── icons/                  # SVG icons
│   ├── fonts/                  # Custom fonts (optional)
│   └── keymap.json             # Default keybindings
├── src/
│   ├── main.rs                 # Entry point
│   ├── app.rs                  # SecousseApp root view
│   ├── state/
│   │   ├── mod.rs
│   │   ├── app_state.rs        # Global application state
│   │   ├── auth_state.rs       # Authentication state
│   │   ├── chat_state.rs       # Chat connection state
│   │   ├── emote_cache.rs      # Emote/badge caching
│   │   └── settings.rs         # Persistent settings
│   ├── views/
│   │   ├── mod.rs
│   │   ├── navbar.rs           # Top navigation bar
│   │   ├── sidebar.rs          # Left sidebar
│   │   ├── video_player.rs     # Video player view
│   │   ├── chat.rs             # Chat panel
│   │   ├── chat_message.rs     # Individual chat message
│   │   ├── stream_info.rs      # Stream info bar
│   │   ├── browse_grid.rs      # Browse/discovery grid
│   │   └── search.rs           # Search dropdown
│   ├── components/
│   │   ├── mod.rs
│   │   ├── button.rs           # Reusable button
│   │   ├── icon.rs             # Icon wrapper
│   │   ├── input.rs            # Text input field
│   │   ├── avatar.rs           # User avatar
│   │   ├── badge.rs            # Chat badge
│   │   ├── emote.rs            # Emote image
│   │   └── tooltip.rs          # Hover tooltip
│   ├── api/
│   │   ├── mod.rs
│   │   ├── twitch.rs           # Twitch API client (from current)
│   │   ├── chat.rs             # IRC WebSocket (from current)
│   │   ├── emotes.rs           # 7TV/BTTV/FFZ (from current)
│   │   └── hls.rs              # HLS stream URL fetching
│   ├── actions.rs              # Keyboard actions
│   ├── theme.rs                # Color/styling constants
│   └── util.rs                 # Utility functions
└── tests/
    └── ...
```

### 4.2 Data Flow (New)

```
┌─────────────────────────────────────────────────────────────────┐
│                    GPUI Application                             │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │                   SecousseApp (Root View)                 │  │
│  │  ┌─────────────────────────────────────────────────────┐ │  │
│  │  │              Entity<AppState>                        │ │  │
│  │  │  - current_channel: Option<String>                   │ │  │
│  │  │  - active_tab: ActiveTab                             │ │  │
│  │  │  - auth: Entity<AuthState>                           │ │  │
│  │  │  - chat: Entity<ChatState>                           │ │  │
│  │  │  - emotes: Entity<EmoteCache>                        │ │  │
│  │  │  - settings: Entity<Settings>                        │ │  │
│  │  └─────────────────────────────────────────────────────┘ │  │
│  │                          │                                │  │
│  │         ┌────────────────┼────────────────┐               │  │
│  │         ▼                ▼                ▼               │  │
│  │  ┌──────────┐    ┌──────────────┐  ┌───────────┐         │  │
│  │  │ Navbar   │    │ VideoPlayer  │  │   Chat    │         │  │
│  │  │ (View)   │    │   (View)     │  │  (View)   │         │  │
│  │  └──────────┘    │ +gpui-video  │  │+uniform   │         │  │
│  │                  │  -player     │  │  _list    │         │  │
│  │  ┌──────────┐    └──────────────┘  └───────────┘         │  │
│  │  │ Sidebar  │                                             │  │
│  │  │ (View)   │    ┌──────────────┐  ┌───────────┐         │  │
│  │  └──────────┘    │ StreamInfo   │  │ BrowseGrid│         │  │
│  │                  │   (View)     │  │  (View)   │         │  │
│  │                  └──────────────┘  └───────────┘         │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                 │
│  Background Tasks (Async):                                      │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐       │
│  │ IRC Chat │  │  Spade   │  │ Auto-    │  │  Token   │       │
│  │WebSocket │  │ Events   │  │ Refresh  │  │Validation│       │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘       │
└─────────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┼───────────────┐
              ▼               ▼               ▼
        ┌──────────┐   ┌──────────┐   ┌──────────┐
        │ Twitch   │   │ 7TV/BTTV │   │ Local    │
        │ API      │   │ /FFZ     │   │ Storage  │
        └──────────┘   └──────────┘   └──────────┘
```

### 4.3 State Architecture

```rust
// Main application state
pub struct AppState {
    // Current view state
    pub current_channel: Option<String>,
    pub active_tab: ActiveTab,
    pub is_sidebar_open: bool,
    pub is_chat_open: bool,
    pub is_fullscreen: bool,
    
    // Sub-entities (owned by App context)
    pub auth: Entity<AuthState>,
    pub chat: Entity<ChatState>,
    pub emotes: Entity<EmoteCache>,
    pub browse: Entity<BrowseState>,
    pub settings: Entity<Settings>,
    
    // Cached data
    pub current_user_info: Option<UserInfo>,
    pub followed_channels: Vec<FollowedChannel>,
}

pub struct AuthState {
    pub is_logged_in: bool,
    pub access_token: Option<String>,
    pub self_info: Option<SelfInfo>,
    pub device_id: String,
}

pub struct ChatState {
    pub messages: Vec<ChatMessage>,
    pub is_connected: bool,
    pub channel: Option<String>,
    // WebSocket handle managed internally
}

pub struct EmoteCache {
    pub global_emotes: HashMap<String, Emote>,
    pub channel_emotes: HashMap<String, HashMap<String, Emote>>,
    pub global_badges: HashMap<String, Badge>,
    pub channel_badges: HashMap<String, HashMap<String, Badge>>,
    pub twitch_global_emotes: HashMap<String, Emote>,
    pub twitch_channel_emotes: HashMap<String, HashMap<String, Emote>>,
}
```

---

## 5. Implementation Phases

### Phase 1: Project Setup & Core Infrastructure (Week 1)

**Goals**: Set up project structure, basic window, theme system

**Tasks**:
1. Create new Cargo project with GPUI dependency
2. Set up build.rs for asset embedding
3. Implement basic window with dark theme
4. Create theme.rs with Twitch color palette
5. Implement Settings entity with persistence (serde + local file)
6. Port TwitchClient from current `twitch.rs`
7. Port emotes API from current `emotes.rs`

**Deliverable**: Empty window with correct colors, settings persistence working

### Phase 2: Authentication System (Week 1-2)

**Goals**: Full OAuth flow working

**Tasks**:
1. Implement AuthState entity
2. Port OAuth local server logic
3. Implement token validation on startup
4. Create login/logout actions
5. Persist token to local storage
6. Create basic Navbar view with login button

**Deliverable**: Can login via browser, token persists across restarts

### Phase 3: Navigation & Layout (Week 2)

**Goals**: Main app layout with navigation

**Tasks**:
1. Implement SecousseApp root view
2. Create Navbar view (logo, tabs, profile)
3. Create Sidebar view (collapsible)
4. Implement tab switching (Following/Browse)
5. Create placeholder views for main content areas
6. Implement keyboard shortcuts for navigation

**Deliverable**: Full layout visible, tabs switch, sidebar collapses

### Phase 4: Browse & Discovery (Week 2-3)

**Goals**: Browse top streams functionality

**Tasks**:
1. Implement BrowseState entity with refresh logic
2. Create BrowseGrid view with stream cards
3. Implement async image loading for thumbnails
4. Add viewer count formatting
5. Implement click-to-select channel
6. Add auto-refresh timer (60s)

**Deliverable**: Can browse top 30 streams, click to select

### Phase 5: Search System (Week 3)

**Goals**: Channel search functionality

**Tasks**:
1. Create SearchState entity with debouncing
2. Implement text input component
3. Create search results dropdown
4. Handle live/offline status display
5. Implement click-outside to dismiss
6. Connect to channel selection

**Deliverable**: Can search channels, results show live status

### Phase 6: Video Playback (Week 3-4)

**Goals**: Live stream video playback

**Tasks**:
1. Integrate gpui-video-player dependency
2. Implement HLS URL fetching (GQL PlaybackAccessToken)
3. Create VideoPlayerView with video element
4. Implement play/pause controls
5. Implement volume control with slider
6. Implement quality selection menu
7. Implement fullscreen toggle
8. Add loading state overlay
9. Handle offline channel display
10. Add live indicator with viewer count

**Deliverable**: Can watch live streams with full controls

### Phase 7: Chat System (Week 4-5)

**Goals**: Real-time chat with emotes/badges

**Tasks**:
1. Port chat.rs WebSocket logic
2. Implement ChatState entity with message buffer
3. Create ChatView with uniform_list for messages
4. Implement ChatMessage view with badge/emote rendering
5. Create text input for sending messages
6. Implement auto-scroll with scroll detection
7. Add "scroll to bottom" button
8. Handle anonymous vs authenticated modes
9. Implement auto-reconnect logic

**Deliverable**: Full chat functionality with emotes and badges

### Phase 8: Emote & Badge Rendering (Week 5)

**Goals**: Proper emote/badge display

**Tasks**:
1. Implement EmoteCache entity with lazy loading
2. Create Emote component (async image loading)
3. Create Badge component
4. Implement emote parsing in messages
5. Handle Twitch, 7TV, BTTV, FFZ emotes
6. Implement badge positioning and sizing
7. Cache loaded images

**Deliverable**: All emotes and badges render correctly

### Phase 9: Following System (Week 5-6)

**Goals**: Followed channels sidebar

**Tasks**:
1. Implement followed channels fetching (Helix API)
2. Create followed channel list in sidebar
3. Add follow/unfollow actions
4. Implement StreamInfo view with follow button
5. Add auto-refresh for followed channels
6. Show live status in sidebar

**Deliverable**: Can view/manage followed channels

### Phase 10: Analytics & Polish (Week 6)

**Goals**: Spade events, final polish

**Tasks**:
1. Implement Spade event sending (minute-watched)
2. Add watch state tracking
3. Implement remaining keyboard shortcuts
4. Add tooltips throughout
5. Performance optimization pass
6. Error handling improvements
7. Logging system setup

**Deliverable**: Feature-complete application

### Phase 11: Testing & Platform Support (Week 7)

**Goals**: Cross-platform testing, bug fixes

**Tasks**:
1. Test on macOS thoroughly
2. Test on Linux (track GPUI issues)
3. Test on Windows (experimental)
4. Fix platform-specific issues
5. Document platform limitations
6. Create release builds

**Deliverable**: Working builds for all target platforms

---

## 6. Technical Deep Dives

### 6.1 Video Playback with gpui-video-player

The gpui-video-player library provides GStreamer-based video playback integrated with GPUI.

**Key Integration Points**:

```rust
use gpui_video_player::{Video, VideoOptions, video};

pub struct VideoPlayerView {
    video: Option<Video>,
    channel: Option<String>,
    is_loading: bool,
    is_paused: bool,
    volume: f64,
    current_quality: i32,
}

impl VideoPlayerView {
    async fn load_stream(&mut self, channel: &str, cx: &mut AsyncWindowContext) {
        // 1. Fetch playback access token via GQL
        let token = self.twitch_client.get_playback_access_token(channel).await?;
        
        // 2. Construct Usher URL
        let url = format!(
            "https://usher.ttvnw.net/api/channel/hls/{}.m3u8?token={}&sig={}&....",
            channel, token.value, token.signature
        );
        
        // 3. Create Video with URL
        let video = Video::new_with_options(
            &Url::parse(&url)?,
            VideoOptions {
                frame_buffer_capacity: Some(30),
                looping: Some(false),
                speed: Some(1.0),
            }
        )?;
        
        self.video = Some(video);
        cx.notify();
    }
}

impl Render for VideoPlayerView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgb(0x000000))
            .when_some(self.video.as_ref(), |this, vid| {
                this.child(
                    video(vid.clone())
                        .id("stream-video")
                        .buffer_capacity(30)
                )
            })
            .child(self.render_controls())
    }
}
```

**HLS Quality Selection**:

gpui-video-player uses GStreamer's playbin which handles HLS natively. Quality selection requires querying the HLS manifest for available levels and using GStreamer's stream selection.

### 6.2 Chat with uniform_list

For efficient rendering of potentially thousands of chat messages:

```rust
impl Render for ChatView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let message_count = self.messages.len();
        
        div()
            .flex()
            .flex_col()
            .h_full()
            .child(
                uniform_list(
                    "chat-messages",
                    message_count,
                    cx.processor(|this, range, window, cx| {
                        range.map(|ix| {
                            let msg = &this.messages[ix];
                            ChatMessageView::new(msg.clone(), &this.emotes, &this.badges)
                                .into_any_element()
                        }).collect()
                    })
                )
                .flex_1()
                .track_scroll(self.scroll_handle.clone())
            )
            .child(self.render_input(cx))
    }
}
```

### 6.3 Emote Parsing & Rendering

```rust
pub struct ParsedMessagePart {
    pub kind: PartKind,
}

pub enum PartKind {
    Text(String),
    Emote { name: String, url: String },
}

pub fn parse_message(
    text: &str,
    emotes: &HashMap<String, Emote>,
    twitch_emotes: &[(String, usize, usize)], // (id, start, end)
) -> Vec<ParsedMessagePart> {
    let mut parts = Vec::new();
    let mut current_pos = 0;
    
    // First, handle Twitch emotes by position
    for (emote_id, start, end) in twitch_emotes {
        if *start > current_pos {
            // Add text before emote
            parts.push(ParsedMessagePart {
                kind: PartKind::Text(text[current_pos..*start].to_string()),
            });
        }
        
        parts.push(ParsedMessagePart {
            kind: PartKind::Emote {
                name: text[*start..=*end].to_string(),
                url: format!("https://static-cdn.jtvnw.net/emoticons/v2/{}/default/dark/3.0", emote_id),
            },
        });
        
        current_pos = end + 1;
    }
    
    // Then parse remaining text for 7TV/BTTV/FFZ emotes
    if current_pos < text.len() {
        let remaining = &text[current_pos..];
        for word in remaining.split_whitespace() {
            if let Some(emote) = emotes.get(word) {
                parts.push(ParsedMessagePart {
                    kind: PartKind::Emote {
                        name: word.to_string(),
                        url: emote.url.clone(),
                    },
                });
            } else {
                parts.push(ParsedMessagePart {
                    kind: PartKind::Text(format!("{} ", word)),
                });
            }
        }
    }
    
    parts
}
```

### 6.4 HTTP Client Setup

```rust
use gpui::HttpClient;
use reqwest_client::ReqwestClient;

fn setup_http_client(cx: &mut App) {
    let http_client = ReqwestClient::user_agent("Secousse/1.0").unwrap();
    cx.set_http_client(Arc::new(http_client));
}
```

### 6.5 Local Storage (Settings Persistence)

```rust
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
pub struct Settings {
    pub device_id: Option<String>,
    pub access_token: Option<String>,
    pub last_channel: Option<String>,
    pub volume: f64,
    pub sidebar_open: bool,
    pub chat_open: bool,
}

impl Settings {
    pub fn load() -> Self {
        let path = Self::path();
        if path.exists() {
            std::fs::read_to_string(&path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default()
        } else {
            Self::default()
        }
    }
    
    pub fn save(&self) {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            std::fs::write(path, json).ok();
        }
    }
    
    fn path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("secousse")
            .join("settings.json")
    }
}
```

---

## 7. Dependencies

### 7.1 Cargo.toml (New)

```toml
[package]
name = "secousse"
version = "0.1.0"
edition = "2021"

[dependencies]
# GPUI Framework
gpui = { git = "https://github.com/zed-industries/zed", rev = "main" }
# OR when published to crates.io:
# gpui = "0.1"

# Video Player
gpui-video-player = "0.1"

# Async Runtime (GPUI uses smol internally, but we need tokio for WebSockets)
tokio = { version = "1", features = ["full"] }
futures = "0.3"

# HTTP Client
reqwest = { version = "0.12", features = ["json", "cookies", "rustls-tls"] }
reqwest-client = { git = "https://github.com/zed-industries/zed" }

# WebSocket (for IRC chat)
tokio-tungstenite = { version = "0.26", features = ["rustls-tls-webpki-roots"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# URL Handling
url = "2.5"
urlencoding = "2.1"

# Utilities
chrono = { version = "0.4", features = ["serde"] }
base64 = "0.22"
rand = "0.8"
uuid = { version = "1", features = ["v4"] }
regex = "1"
anyhow = "1.0"
thiserror = "1.0"

# Logging
log = "0.4"
env_logger = "0.11"

# Local storage
dirs = "5.0"

# Image handling
image = "0.25"
smallvec = "1.13"

[target.'cfg(target_os = "macos")'.dependencies]
core-foundation = "0.10"
core-video = "0.2"

[build-dependencies]
# For embedding assets
include_dir = "0.7"
```

### 7.2 System Dependencies

**macOS**:
- Xcode Command Line Tools
- GStreamer (for video): `brew install gstreamer gst-plugins-base gst-plugins-good gst-plugins-bad gst-plugins-ugly`

**Linux**:
- GStreamer: `apt install libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly`
- Vulkan: `apt install libvulkan-dev`

**Windows**:
- GStreamer runtime from https://gstreamer.freedesktop.org/download/
- Visual Studio Build Tools

---

## 8. Platform Considerations

### 8.1 macOS (Primary Support)

- **Rendering**: Metal (excellent performance)
- **Status**: Fully supported by GPUI
- **Video**: Hardware-accelerated via CVPixelBuffer
- **Notes**: Best experience, should be primary development platform

### 8.2 Linux (Secondary Support)

- **Rendering**: Vulkan or OpenGL fallback
- **Status**: Actively improving in GPUI
- **Known Issues**:
  - Some window management quirks
  - Font rendering differences
  - Need to test on multiple distros (Ubuntu, Fedora, Arch)
- **Video**: Software rendering or VA-API

### 8.3 Windows (Experimental)

- **Rendering**: Direct3D 12 or Vulkan
- **Status**: Experimental in GPUI
- **Known Issues**:
  - May have stability issues
  - Window management differences
  - DPI scaling challenges
- **Notes**: Track Zed's Windows progress closely

### 8.4 Platform-Specific Code

```rust
#[cfg(target_os = "macos")]
fn platform_specific_init() {
    // macOS-specific initialization
}

#[cfg(target_os = "linux")]
fn platform_specific_init() {
    // Linux-specific initialization
}

#[cfg(target_os = "windows")]
fn platform_specific_init() {
    // Windows-specific initialization
}
```

---

## 9. Risk Assessment

### 9.1 High Risk

| Risk | Mitigation |
|------|------------|
| GPUI API changes | Pin to specific git rev, track Zed releases |
| Video playback complexity | Start with gpui-video-player, have fallback plan |
| Windows support gaps | Develop primarily on macOS, test Windows regularly |

### 9.2 Medium Risk

| Risk | Mitigation |
|------|------------|
| Chat performance with many messages | Use uniform_list, limit message buffer |
| Emote loading performance | Aggressive caching, lazy loading |
| OAuth browser flow on Linux | Test on multiple DEs, use xdg-open |

### 9.3 Low Risk

| Risk | Mitigation |
|------|------------|
| API compatibility | APIs are already working in current app |
| State management complexity | GPUI's entity model is well-documented |
| Styling parity | Tailwind-like API is intuitive |

---

## 10. Timeline Estimate

| Phase | Duration | Cumulative |
|-------|----------|------------|
| Phase 1: Setup & Core | 1 week | Week 1 |
| Phase 2: Authentication | 0.5 week | Week 1.5 |
| Phase 3: Navigation & Layout | 1 week | Week 2.5 |
| Phase 4: Browse & Discovery | 0.5 week | Week 3 |
| Phase 5: Search | 0.5 week | Week 3.5 |
| Phase 6: Video Playback | 1.5 weeks | Week 5 |
| Phase 7: Chat System | 1.5 weeks | Week 6.5 |
| Phase 8: Emotes & Badges | 0.5 week | Week 7 |
| Phase 9: Following System | 0.5 week | Week 7.5 |
| Phase 10: Analytics & Polish | 0.5 week | Week 8 |
| Phase 11: Testing & Platforms | 1 week | Week 9 |

**Total Estimated Time: 8-9 weeks**

---

## Appendix A: Code Migration Reference

### A.1 Files to Port Directly

These Rust files can be largely reused:

- `src-tauri/src/twitch.rs` → `src/api/twitch.rs` (minor adaptations)
- `src-tauri/src/chat.rs` → `src/api/chat.rs` (event emission changes)
- `src-tauri/src/emotes.rs` → `src/api/emotes.rs` (direct port)

### A.2 Files to Rewrite

These require complete rewrite in GPUI:

- All React components → GPUI views
- All React hooks → GPUI entities
- Tauri commands → Direct function calls

### A.3 Removed Dependencies

No longer needed:
- Tauri and all tauri-plugin-*
- React, Vite, Tailwind CSS
- hls.js (replaced by GStreamer)
- lucide-react (replaced by custom SVG icons)

---

## Appendix B: Keyboard Shortcuts Plan

```rust
actions!(secousse, [
    // Navigation
    GoToFollowing,
    GoToBrowse,
    ToggleSidebar,
    ToggleChat,
    
    // Video
    TogglePlayPause,
    ToggleMute,
    VolumeUp,
    VolumeDown,
    ToggleFullscreen,
    
    // Chat
    FocusChatInput,
    ScrollToBottom,
    
    // Search
    FocusSearch,
    ClearSearch,
    
    // App
    Quit,
    Refresh,
]);

// Default bindings
cx.bind_keys([
    KeyBinding::new("cmd-1", GoToFollowing, None),
    KeyBinding::new("cmd-2", GoToBrowse, None),
    KeyBinding::new("cmd-b", ToggleSidebar, None),
    KeyBinding::new("cmd-shift-c", ToggleChat, None),
    KeyBinding::new("space", TogglePlayPause, Some("video-player")),
    KeyBinding::new("m", ToggleMute, Some("video-player")),
    KeyBinding::new("f", ToggleFullscreen, Some("video-player")),
    KeyBinding::new("cmd-k", FocusSearch, None),
    KeyBinding::new("escape", ClearSearch, Some("search")),
    KeyBinding::new("cmd-q", Quit, None),
    KeyBinding::new("cmd-r", Refresh, None),
]);
```

---

## Appendix C: Theme Constants

```rust
pub mod theme {
    use gpui::rgb;
    
    // Background colors
    pub const BG_PRIMARY: u32 = 0x0e0e10;      // Main background
    pub const BG_SECONDARY: u32 = 0x18181b;    // Sidebar, cards
    pub const BG_TERTIARY: u32 = 0x1f1f23;     // Hover states
    pub const BG_ELEVATED: u32 = 0x26262c;     // Menus, dropdowns
    
    // Text colors
    pub const TEXT_PRIMARY: u32 = 0xefeff1;    // Main text
    pub const TEXT_SECONDARY: u32 = 0xadadb8;  // Muted text
    pub const TEXT_LINK: u32 = 0xbf94ff;       // Links
    
    // Brand colors
    pub const TWITCH_PURPLE: u32 = 0x9146ff;   // Primary brand
    pub const TWITCH_PURPLE_HOVER: u32 = 0x772ce8;
    
    // Status colors
    pub const LIVE_RED: u32 = 0xeb0400;        // Live indicator
    pub const SUCCESS_GREEN: u32 = 0x00c853;
    pub const WARNING_YELLOW: u32 = 0xffca28;
    pub const ERROR_RED: u32 = 0xff4444;
    
    // Border colors
    pub const BORDER_DEFAULT: u32 = 0x3f3f46;
    pub const BORDER_FOCUS: u32 = 0x9146ff;
}
```

---

## Next Steps

1. **Review this plan** - Confirm all features are captured
2. **Set up development environment** - Install GPUI dependencies
3. **Create project skeleton** - Basic Cargo.toml and directory structure
4. **Begin Phase 1** - Core infrastructure implementation

Ready to proceed when you give the go-ahead!
