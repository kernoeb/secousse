//! Chat view component
//!
//! Displays real-time Twitch chat messages with inline emotes from
//! Twitch, 7TV, BTTV, and FFZ.

use async_compat::Compat;
use gpui::*;
use gpui_component::button::{Button, ButtonCustomVariant, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{Icon, IconName, Sizable};
use log::{error, info};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use crate::api::chat::{ChatEvent, ChatMessage};
use crate::api::emotes;
use crate::theme;

const MAX_MESSAGES: usize = 150;

/// Emote size in the chat (pixels, roughly 1x scale for inline text)
const EMOTE_SIZE: f32 = 24.0;

/// Chat view state
pub struct ChatView {
    /// Channel name
    channel: String,
    /// Chat messages
    messages: Arc<Mutex<VecDeque<ChatMessage>>>,
    /// Whether chat is connected
    is_connected: bool,
    /// Chat connection sender (for sending messages) - async_channel for smol compatibility
    sender: Option<async_channel::Sender<String>>,
    /// Access token for authenticated chat
    access_token: Option<String>,
    /// Username for authenticated chat
    username: Option<String>,
    /// Virtualized list state (only renders visible messages)
    list_state: ListState,
    /// Last known message count (to detect new additions / removals)
    last_message_count: usize,
    /// Text input state for the chat input
    chat_input: Entity<InputState>,
    /// Third-party emote map: emote name → image URL (7TV, BTTV, FFZ)
    third_party_emotes: Arc<Mutex<HashMap<String, String>>>,
}

/// Events emitted by chat view
#[derive(Clone)]
pub enum ChatViewEvent {
    ConnectionStatusChanged,
    CloseRequested,
}

impl EventEmitter<ChatViewEvent> for ChatView {}

impl ChatView {
    /// Create a new chat view for a channel.
    /// `channel_id` is the Twitch user/broadcaster ID, used to fetch third-party emotes.
    pub fn new(
        channel: String,
        channel_id: Option<String>,
        access_token: Option<String>,
        username: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let messages = Arc::new(Mutex::new(VecDeque::new()));
        let chat_input = cx.new(|cx| InputState::new(window, cx));

        // Subscribe to input events for sending messages
        cx.subscribe_in(&chat_input, window, |this, state, event: &InputEvent, window, cx| {
            match event {
                InputEvent::PressEnter { .. } => {
                    let text = state.read(cx).value().to_string();
                    if !text.is_empty() {
                        this.send_message_text(text.clone(), cx);
                        state.update(cx, |state, cx| {
                            state.set_value("", window, cx);
                        });
                    }
                }
                InputEvent::Change | InputEvent::Focus | InputEvent::Blur => {}
            }
        })
        .detach();

        // Bottom-aligned list: new items appear at the bottom, viewport stays pinned.
        // overdraw of 200px renders a bit beyond the viewport for smoother scrolling.
        let list_state = ListState::new(0, ListAlignment::Bottom, px(200.0));

        let third_party_emotes = Arc::new(Mutex::new(HashMap::new()));

        let mut view = Self {
            channel: channel.clone(),
            messages,
            is_connected: false,
            sender: None,
            access_token,
            username,
            list_state,
            last_message_count: 0,
            chat_input,
            third_party_emotes: third_party_emotes.clone(),
        };

        // Load cached emotes first for fast startup
        {
            let cached_global = emotes::load_cached_global_emotes();
            let cached_channel = channel_id
                .as_deref()
                .map(emotes::load_cached_channel_emotes)
                .unwrap_or_default();

            if !cached_global.is_empty() || !cached_channel.is_empty() {
                let mut map = third_party_emotes.lock().unwrap();
                for emote in cached_global.into_iter().chain(cached_channel) {
                    map.insert(emote.name, emote.url);
                }
            }
        }

        // Fetch third-party emotes (7TV, BTTV, FFZ) in the background
        if let Some(ch_id) = channel_id {
            cx.spawn(async move |this: gpui::WeakEntity<ChatView>, cx: &mut gpui::AsyncApp| {
                info!("[ChatView] Fetching emotes for channel {}", ch_id);

                // Fetch global + channel emotes concurrently
                let (global, channel_emotes) = Compat::new(async {
                    tokio::join!(
                        emotes::fetch_global_emotes(),
                        emotes::fetch_channel_emotes(&ch_id)
                    )
                })
                .await;

                let total = global.len() + channel_emotes.len();
                info!(
                    "[ChatView] Loaded {} emotes ({} global, {} channel)",
                    total,
                    global.len(),
                    channel_emotes.len()
                );

                // Insert into the shared map
                {
                    let mut map = third_party_emotes.lock().unwrap();
                    for emote in global.into_iter().chain(channel_emotes) {
                        map.insert(emote.name, emote.url);
                    }
                }

                // Notify to re-render messages with emotes
                let _ = cx.update(|cx: &mut App| {
                    let _ = this.update(cx, |_view: &mut ChatView, cx| {
                        cx.notify();
                    });
                });
            })
            .detach();
        }

        // Start connection
        view.connect(cx);

        view
    }

    /// Connect to chat
    fn connect(&mut self, cx: &mut Context<Self>) {
        let channel = self.channel.clone();
        let access_token = self.access_token.clone();
        let username = self.username.clone();
        let messages = self.messages.clone();

        info!("[ChatView] Connecting to #{}", channel);

        cx.spawn(async move |this: gpui::WeakEntity<ChatView>, cx: &mut gpui::AsyncApp| {
            // Reconnect loop - will keep trying to connect
            loop {
                // Connect using tokio runtime via Compat wrapper
                let channel_for_connect = channel.clone();
                let access_token_clone = access_token.clone();
                let username_clone = username.clone();
                let connection_result = Compat::new(crate::api::chat::connect_chat(
                    channel_for_connect,
                    access_token_clone,
                    username_clone,
                ))
                .await;

                match connection_result {
                    Ok(connection) => {
                        info!("[ChatView] Connected to #{}", channel);

                        // Store sender for sending messages
                        let sender = connection.sender.clone();
                        let _ = cx.update(|cx: &mut App| {
                            let _ = this.update(cx, |view: &mut ChatView, cx| {
                                view.sender = Some(sender);
                                view.is_connected = true;
                                cx.emit(ChatViewEvent::ConnectionStatusChanged);
                                cx.notify();
                            });
                        });

                        // Process incoming messages - async_channel works with smol directly
                        let channel_for_loop = channel.clone();
                        while let Ok(event) = connection.receiver.recv().await {
                            match event {
                                ChatEvent::Message(msg) => {
                                    let mut msgs = messages.lock().unwrap();
                                    msgs.push_back(msg);
                                    while msgs.len() > MAX_MESSAGES {
                                        msgs.pop_front();
                                    }
                                    drop(msgs);

                                    let _ = cx.update(|cx: &mut App| {
                                        let _ = this.update(cx, |_view: &mut ChatView, cx| {
                                            cx.notify();
                                        });
                                    });
                                }
                                ChatEvent::Notice(notice) => {
                                    info!("[ChatView] Notice: {}", notice);
                                }
                                ChatEvent::Connected => {
                                    info!("[ChatView] Connection confirmed");
                                }
                                ChatEvent::Disconnected(reason) => {
                                    info!("[ChatView] Disconnected: {}", reason);
                                    let _ = cx.update(|cx: &mut App| {
                                        let _ = this.update(cx, |view: &mut ChatView, cx| {
                                            view.is_connected = false;
                                            view.sender = None;
                                            cx.emit(ChatViewEvent::ConnectionStatusChanged);
                                            cx.notify();
                                        });
                                    });
                                    break;
                                }
                            }
                        }
                        info!("[ChatView] Message loop ended for #{}", channel_for_loop);
                    }
                    Err(e) => {
                        error!("[ChatView] Failed to connect: {}", e);
                        let _ = cx.update(|cx: &mut App| {
                            let _ = this.update(cx, |view: &mut ChatView, cx| {
                                view.is_connected = false;
                                cx.notify();
                            });
                        });
                    }
                }

                // Wait before attempting to reconnect
                info!(
                    "[ChatView] Will attempt to reconnect to #{} in 5 seconds...",
                    channel
                );
                smol::Timer::after(std::time::Duration::from_secs(5)).await;

                // Check if we should still reconnect (view might be dropped)
                let should_reconnect = cx
                    .update(|cx: &mut App| {
                        this.update(cx, |view: &mut ChatView, _cx| {
                            // Only reconnect if we're still tracking this channel
                            !view.channel.is_empty()
                        })
                        .ok()
                        .unwrap_or(false)
                    })
                    .ok()
                    .unwrap_or(false);

                if !should_reconnect {
                    info!("[ChatView] Stopping reconnect loop for #{}", channel);
                    break;
                }

                info!("[ChatView] Attempting to reconnect to #{}...", channel);
            }
        })
        .detach();
    }

    /// Send a message with the given text
    fn send_message_text(&mut self, text: String, cx: &mut Context<Self>) {
        if text.is_empty() {
            return;
        }

        let user = self
            .username
            .clone()
            .unwrap_or_else(|| "You".to_string());
        let id = format!(
            "local-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        let local_message = ChatMessage {
            id,
            user,
            message: text.clone(),
            color: None,
            badges: Vec::new(),
            channel: self.channel.clone(),
            emotes: Vec::new(),
        };

        {
            let mut msgs = self.messages.lock().unwrap();
            msgs.push_back(local_message);
            while msgs.len() > MAX_MESSAGES {
                msgs.pop_front();
            }
            self.last_message_count = msgs.len();
        }
        self.list_state.reset(self.last_message_count);
        cx.notify();

        if let Some(sender) = &self.sender {
            let sender = sender.clone();

            // async_channel works with smol directly, no Compat needed
            cx.spawn(async move |_this: gpui::WeakEntity<ChatView>, _cx: &mut gpui::AsyncApp| {
                if let Err(e) = sender.send(text).await {
                    error!("[ChatView] Failed to send message: {}", e);
                }
            })
            .detach();

            cx.notify();
        }
    }

    /// Sync the list state with the current message count.
    /// Called once per render — only touches the list when the count actually changed.
    fn sync_list(&mut self) {
        let count = self.messages.lock().unwrap().len();
        if count != self.last_message_count {
            // Tell the list the total item count changed.
            // `reset` re-measures everything but is cheap because the list
            // only measures visible + overdraw items.
            self.list_state.reset(count);
            self.last_message_count = count;
        }
    }
}

impl Render for ChatView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_connected = self.is_connected;
        let has_input = !self.chat_input.read(cx).value().is_empty();
        let can_send = self.access_token.is_some() && self.sender.is_some();

        // Sync list state with current message count (cheap no-op when unchanged)
        self.sync_list();

        let message_count = self.last_message_count;

        // Build the virtualized message list.
        // The closure is called ONLY for visible items (+ overdraw) — not all 150.
        let messages_ref = self.messages.clone();
        let emotes_ref = self.third_party_emotes.clone();
        let chat_list = list(self.list_state.clone(), move |ix, _window, _cx| {
            let msgs = messages_ref.lock().unwrap();
            if let Some(msg) = msgs.get(ix) {
                let emote_map = emotes_ref.lock().unwrap();
                render_message(msg, &emote_map).into_any_element()
            } else {
                div().into_any_element()
            }
        })
        .w_full()
        .h_full()
        .py(px(8.0));

        div()
            .id("chat-view")
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .bg(theme::BG_SECONDARY)
            // Chat header
            .child(
                div()
                    .h(px(50.0))
                    .flex_shrink_0()
                    .px(px(16.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(theme::BORDER_SUBTLE)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                {
                                    let close_variant = ButtonCustomVariant::new(cx)
                                        .color(theme::TRANSPARENT.into())
                                        .foreground(theme::TEXT_SECONDARY.into())
                                        .border(theme::TRANSPARENT.into())
                                        .hover(theme::BG_ELEVATED.into())
                                        .active(theme::BG_ELEVATED.into());

                                    Button::new("chat-close-btn")
                                        .custom(close_variant)
                                        .xsmall()
                                        .rounded(px(2.0))
                                        .icon(IconName::PanelRight)
                                        .on_click(cx.listener(|_this, _event, _window, cx| {
                                            cx.emit(ChatViewEvent::CloseRequested);
                                        }))
                                },
                            )
                            .child(
                                div()
                                    .text_color(theme::TEXT_PRIMARY)
                                    .text_size(px(14.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("STREAM CHAT"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .child(div().size(px(8.0)).rounded_full().bg(if is_connected {
                                theme::SUCCESS_GREEN
                            } else {
                                theme::OFFLINE_GRAY
                            }))
                            .child(
                                div()
                                    .text_color(theme::TEXT_SECONDARY)
                                    .text_size(px(11.0))
                                    .child(if is_connected {
                                        "Connected"
                                    } else {
                                        "Disconnected"
                                    }),
                            ),
                    ),
            )
            // Chat messages area
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .child(if message_count == 0 {
                        div()
                            .w_full()
                            .h_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(theme::TEXT_SECONDARY)
                            .text_size(px(13.0))
                            .child(if is_connected {
                                "Waiting for messages..."
                            } else {
                                "Connecting to chat..."
                            })
                            .into_any_element()
                    } else {
                        chat_list.into_any_element()
                    }),
            )
            // Chat input
            .child(
                div()
                    .h(px(70.0))
                    .flex_shrink_0()
                    .px(px(12.0))
                    .py(px(10.0))
                    .border_t_1()
                    .border_color(theme::BORDER_SUBTLE)
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child({
                        let placeholder = if can_send {
                            "Send a message"
                        } else {
                            "Log in to chat"
                        };

                        self.chat_input.update(cx, |state, cx| {
                            state.set_placeholder(placeholder, window, cx);
                        });

                        Input::new(&self.chat_input)
                            .large()
                            .disabled(!can_send)
                            .suffix(if has_input && can_send {
                                Button::new("send-btn")
                                    .primary()
                                    .xsmall()
                                    .rounded(px(2.0))
                                    .icon(Icon::empty().path("icons/send.svg").small())
                                    .label("Chat")
                                    .on_click(cx.listener(|this, _event, window, cx| {
                                        let text = this.chat_input.read(cx).value().to_string();
                                        this.send_message_text(text, cx);
                                        this.chat_input.update(cx, |state, cx| {
                                            state.set_value("", window, cx);
                                        });
                                    }))
                                    .into_any_element()
                            } else {
                                div().into_any_element()
                            })
                    }),
            )
    }
}

/// A fragment of a chat message: either text or an emote image.
enum MessageFragment {
    Text(String),
    Emote { url: String },
}

/// Render a single chat message with inline emote images.
///
/// Strategy:
/// 1. Build a set of Twitch-native emote ranges from `msg.emotes` (position-based).
/// 2. Walk through the message: for each word that isn't a native emote, check the
///    third-party emote map (7TV/BTTV/FFZ).
/// 3. Produce a flex-wrap div with text spans and `img()` elements.
fn render_message(msg: &ChatMessage, third_party_emotes: &HashMap<String, String>) -> impl IntoElement {
    let color = msg
        .color
        .as_deref()
        .and_then(parse_hex_color)
        .unwrap_or(theme::TWITCH_PURPLE);

    // --- Build fragments from message body ---
    let fragments = build_message_fragments(&msg.message, &msg.emotes, third_party_emotes);

    // Check if the message has any emotes at all
    let has_emotes = fragments
        .iter()
        .any(|f| matches!(f, MessageFragment::Emote { .. }));

    // If no emotes, use the fast StyledText path (no flex-wrap overhead)
    if !has_emotes {
        let username_part = format!("{}: ", msg.user);
        let full_text = format!("{}{}", username_part, msg.message);
        let username_len = username_part.len();

        return div()
            .id(SharedString::from(format!("msg-{}", msg.id)))
            .w_full()
            .py(px(3.0))
            .px(px(16.0))
            .text_size(px(13.0))
            .text_color(theme::TEXT_PRIMARY)
            .child(
                StyledText::new(full_text).with_highlights(vec![(
                    0..username_len,
                    HighlightStyle {
                        color: Some(color.into()),
                        font_weight: Some(FontWeight::SEMIBOLD),
                        ..Default::default()
                    },
                )]),
            )
            .into_any_element();
    }

    // --- Emotes present: use flex-wrap layout ---
    let mut row = div()
        .id(SharedString::from(format!("msg-{}", msg.id)))
        .w_full()
        .py(px(3.0))
        .px(px(16.0))
        .text_size(px(13.0))
        .text_color(theme::TEXT_PRIMARY)
        .flex()
        .flex_wrap()
        .items_center();

    // Username prefix
    row = row.child(
        div()
            .text_color(color)
            .font_weight(FontWeight::SEMIBOLD)
            .flex_shrink_0()
            .child(format!("{}: ", msg.user)),
    );

    // Message fragments
    for fragment in fragments {
        match fragment {
            MessageFragment::Text(text) => {
                row = row.child(div().child(text));
            }
            MessageFragment::Emote { url } => {
                row = row.child(
                    img(SharedString::from(url))
                        .w(px(EMOTE_SIZE))
                        .h(px(EMOTE_SIZE))
                        .flex_shrink_0(),
                );
            }
        }
    }

    row.into_any_element()
}

/// Build message fragments by resolving Twitch native emotes (position-based)
/// and third-party emotes (name-based word matching).
fn build_message_fragments(
    message: &str,
    twitch_emotes: &[(String, usize, usize)],
    third_party: &HashMap<String, String>,
) -> Vec<MessageFragment> {
    // If there are Twitch native emotes with position data, use position-based splitting
    if !twitch_emotes.is_empty() {
        return build_fragments_with_twitch_emotes(message, twitch_emotes, third_party);
    }

    // No Twitch native emotes — just do word-based matching for third-party
    build_fragments_word_match(message, third_party)
}

/// Position-based splitting for messages with Twitch native emotes.
/// Twitch emote positions are byte offsets into the message string.
fn build_fragments_with_twitch_emotes(
    message: &str,
    twitch_emotes: &[(String, usize, usize)],
    third_party: &HashMap<String, String>,
) -> Vec<MessageFragment> {
    let mut fragments = Vec::new();
    let msg_bytes = message.as_bytes();
    let mut cursor = 0usize;

    for (emote_id, start, end) in twitch_emotes {
        let start = char_index_to_byte_index(message, *start).min(msg_bytes.len());
        // Twitch end positions are inclusive (char index)
        let end = char_index_to_byte_index(message, end.saturating_add(1)).min(msg_bytes.len());

        if start > cursor {
            // Text between previous emote and this one — check for third-party emotes
            let text_between = &message[cursor..start];
            fragments.extend(build_fragments_word_match(text_between, third_party));
        }

        // Twitch native emote
        let url = format!(
            "https://static-cdn.jtvnw.net/emoticons/v2/{}/default/dark/2.0",
            emote_id
        );
        fragments.push(MessageFragment::Emote { url });
        cursor = end;
    }

    // Remaining text after last emote
    if cursor < message.len() {
        let remaining = &message[cursor..];
        fragments.extend(build_fragments_word_match(remaining, third_party));
    }

    fragments
}

fn char_index_to_byte_index(text: &str, char_index: usize) -> usize {
    if char_index == 0 {
        return 0;
    }

    let mut count = 0usize;
    for (byte_index, _) in text.char_indices() {
        if count == char_index {
            return byte_index;
        }
        count += 1;
    }

    text.len()
}

/// Word-based matching for third-party emotes (7TV, BTTV, FFZ).
/// Splits on whitespace, checks each word against the emote map.
fn build_fragments_word_match(
    text: &str,
    third_party: &HashMap<String, String>,
) -> Vec<MessageFragment> {
    if third_party.is_empty() {
        // Fast path: no third-party emotes loaded (yet)
        if !text.is_empty() {
            return vec![MessageFragment::Text(text.to_string())];
        }
        return vec![];
    }

    let mut fragments = Vec::new();
    let mut pending_text = String::new();

    for word in text.split_inclusive(' ') {
        let trimmed = word.trim();
        if let Some(url) = third_party.get(trimmed) {
            // Flush accumulated text
            if !pending_text.is_empty() {
                fragments.push(MessageFragment::Text(pending_text.clone()));
                pending_text.clear();
            }
            fragments.push(MessageFragment::Emote {
                url: url.clone(),
            });
            // Preserve trailing space after emote
            if word.ends_with(' ') {
                pending_text.push(' ');
            }
        } else {
            pending_text.push_str(word);
        }
    }

    if !pending_text.is_empty() {
        fragments.push(MessageFragment::Text(pending_text));
    }

    fragments
}

/// Parse hex color string to Rgba
fn parse_hex_color(hex: &str) -> Option<Rgba> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }

    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;

    Some(Rgba {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    })
}
