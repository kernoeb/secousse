# Secousse GPUI

A GPU-accelerated Twitch streaming client built with [GPUI](https://gpui.rs/) - the UI framework from Zed.

## Technical Stack

- UI: GPUI + gpui-component
- Video: GStreamer (app sink pipeline)
- Networking: reqwest with rustls TLS only
- Chat: Twitch IRC over WebSocket (tokio-tungstenite)
- Async: smol in GPUI with tokio bridge (async-compat)
- Storage: JSON settings under OS app data directory

## Prerequisites

### macOS

1. **Xcode** - Install from the App Store or [developer.apple.com](https://developer.apple.com/xcode/)

2. **Metal Toolchain** - Required for GPUI shader compilation:
   - Open Xcode
   - Go to **Xcode > Settings > Components**
   - Download **Metal Toolchain**
   
   Or via command line (may require Xcode to be healthy):
   ```bash
   xcodebuild -downloadComponent MetalToolchain
   ```

3. **Rust** - Install via [rustup](https://rustup.rs/):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

### Linux (Future)

```bash
# Ubuntu/Debian
sudo apt install libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
    gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly \
    libvulkan-dev libxkbcommon-dev

# Fedora
sudo dnf install gstreamer1-devel gstreamer1-plugins-base-devel \
    gstreamer1-plugins-good gstreamer1-plugins-bad-free gstreamer1-plugins-ugly-free \
    vulkan-loader-devel libxkbcommon-devel
```

### Windows (Experimental)

- Visual Studio Build Tools
- GStreamer runtime from [gstreamer.freedesktop.org](https://gstreamer.freedesktop.org/download/)

## Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run
cargo run
```

## Environment Variables

If you encounter build issues with Metal/bindgen, try setting:

```bash
export SDKROOT=$(xcrun --show-sdk-path)
export PATH="/Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/bin:$PATH"
export BINDGEN_EXTRA_CLANG_ARGS="-isysroot $SDKROOT"
```

## Project Structure

```
secousse-gpui/
├── Cargo.toml          # Dependencies
├── src/
│   ├── main.rs         # Entry point
│   ├── app.rs          # Main application view
│   ├── theme.rs        # Twitch color palette
│   ├── actions.rs      # Keyboard shortcuts
│   ├── http.rs          # GPUI HTTP client bridge
│   ├── assets.rs        # Embedded assets
│   ├── api/
│   │   ├── mod.rs
│   │   ├── twitch.rs   # Twitch GQL/Helix API
│   │   ├── emotes.rs   # 7TV/BTTV/FFZ emotes
│   │   └── chat.rs     # IRC WebSocket
│   ├── state/
│   │   ├── mod.rs
│   │   ├── app_state.rs    # Main app state
│   │   ├── auth_state.rs   # Authentication
│   │   └── settings.rs     # Persistent settings
│   ├── views/
│   │   ├── mod.rs
│   │   ├── navbar.rs       # Top navigation
│   │   └── sidebar.rs      # Followed channels
│   └── video/
│       ├── mod.rs          # Video module
│       ├── element.rs      # GPUI video element
│       └── gst_video.rs    # GStreamer pipeline
└── assets/
    └── icons/              # SVG icons
```

## Features (Planned)

- [x] Project structure
- [x] Twitch API client
- [x] Emote providers (7TV, BTTV, FFZ)
- [x] IRC chat client
- [x] Settings persistence
- [x] Basic UI layout
- [x] Video playback (GStreamer)
- [x] OAuth authentication flow
- [x] Channel search
- [x] Browse top streams
- [x] Full chat with emotes/badges
- [x] Keyboard shortcuts

## License

MIT
