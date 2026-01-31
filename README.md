# Secousse

An open-source streaming client for Twitch, built with Tauri 2, React, TypeScript, and Rust.

*Secousse* is French for "twitch" (as in a sudden movement).

![Secousse Screenshot](screenshot.png)

## Features

- **Live Stream Playback** - HLS video streaming with quality selection
- **Chat** - Real-time chat with emote support (Twitch, 7TV, BTTV, FFZ)
- **Authentication** - OAuth login to access your followed channels and send chat messages
- **Follow/Unfollow** - Manage your followed channels directly from the app
- **Browse** - Discover top live streams
- **Search** - Find channels quickly

## Tech Stack

- **Frontend**: React 19, TypeScript, Tailwind CSS
- **Backend**: Rust with Tauri 2
- **Video**: HLS.js with custom Tauri loader for CORS bypass
- **Chat**: IRC WebSocket connection
- **APIs**: Twitch GQL (public data) + Helix API (authenticated operations)

## Development

### Prerequisites

- [Node.js](https://nodejs.org/) or [Bun](https://bun.sh/)
- [Rust](https://rustup.rs/)
- [Tauri CLI](https://tauri.app/start/prerequisites/)

### Setup

```bash
# Install dependencies
bun install

# Run in development mode
bun run tauri dev

# Build for production
bun run tauri build
```

## Project Structure

```
secousse/
├── src/                    # React frontend
│   ├── App.tsx            # Main application component
│   ├── TauriHlsLoader.ts  # Custom HLS loader for Tauri
│   └── types.ts           # TypeScript type definitions
├── src-tauri/             # Rust backend
│   └── src/
│       ├── lib.rs         # Tauri commands and app setup
│       ├── twitch.rs      # Twitch API client (GQL + Helix)
│       ├── chat.rs        # IRC WebSocket chat handler
│       └── emotes.rs      # 7TV/BTTV/FFZ emote fetching
└── package.json
```

## Acknowledgments

This project is inspired by [Xtra](https://github.com/crackededed/Xtra), an excellent open-source Twitch client for Android. Thank you to the Xtra team for their work!

## License

MIT
