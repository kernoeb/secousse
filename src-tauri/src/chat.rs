use futures_util::{SinkExt, StreamExt};
use log::{info, error};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Window};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tokio::sync::mpsc;

// Twitch never echoes our own PRIVMSG back, it answers with a USERSTATE. We
// queue what we send so the read task can pair each USERSTATE with its text.
const MAX_PENDING_ECHO: usize = 20;

static ECHO_SEQ: AtomicU64 = AtomicU64::new(0);


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EmoteRange {
    pub id: String,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub id: String,
    pub user: String,
    pub message: String,
    pub color: Option<String>,
    pub badges: Vec<(String, String)>,
    pub emotes: Vec<EmoteRange>,
    pub channel: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatNotice {
    pub msg_id: String,
    pub message: String,
    pub channel: String,
}

pub struct ChatConnection {
    pub sender: mpsc::Sender<String>,
    pub read_task: tokio::task::JoinHandle<()>,
    pub write_task: tokio::task::JoinHandle<()>,
}

pub async fn connect_chat(
    channel: String, 
    window: Window, 
    access_token: Option<String>,
    username: Option<String>,
) -> anyhow::Result<ChatConnection> {
    let url = "wss://irc-ws.chat.twitch.tv:443";
    let (ws_stream, _) = connect_async(url).await?;
    let (mut write, mut read) = ws_stream.split();

    // Create channel for sending messages
    let (tx, mut rx) = mpsc::channel::<String>(100);

    // Send initial IRC commands
    write.send(Message::Text("CAP REQ :twitch.tv/tags twitch.tv/commands".into())).await?;
    
    // Use authenticated or anonymous connection
    if let (Some(token), Some(user)) = (&access_token, &username) {
        write.send(Message::Text(format!("PASS oauth:{}", token).into())).await?;
        write.send(Message::Text(format!("NICK {}", user.to_lowercase()).into())).await?;
        info!("[Chat] Connecting as authenticated user: {}", user);
    } else {
        write.send(Message::Text("PASS SCHMOOPIE".into())).await?;
        write.send(Message::Text("NICK justinfan12345".into())).await?;
        info!("[Chat] Connecting as anonymous user");
    }
    
    write.send(Message::Text(format!("JOIN #{}", channel).into())).await?;

    let channel_clone = channel.clone();
    let channel_for_read = channel.clone();

    let pending_echo: Arc<Mutex<VecDeque<String>>> = Arc::new(Mutex::new(VecDeque::new()));
    let pending_for_read = pending_echo.clone();
    let self_name = username.clone();

    // Spawn task to handle incoming messages
    let window_clone = window.clone();
    let read_task = tokio::spawn(async move {
        while let Some(msg) = read.next().await {
            match msg {
                Ok(msg) if msg.is_text() => {
                    let text = msg.to_text().unwrap_or("");
                    for line in text.lines() {
                        if line.starts_with("PING") {
                            // PING handled in write task
                        } else if line.contains("PRIVMSG") {
                            if let Some(mut parsed) = parse_irc_message(line) {
                                // Include channel info so frontend can filter
                                parsed.channel = channel_for_read.clone();
                                let _ = window_clone.emit("chat-message", parsed);
                            }
                        } else if line.contains(" USERSTATE ") {
                            // Our own message came back as a USERSTATE, so build
                            // the echo the frontend never receives as a PRIVMSG.
                            let text = pending_for_read.lock().unwrap().pop_front();
                            if let Some(text) = text {
                                let mut echo = parse_own_message(line, &text, self_name.as_deref());
                                echo.channel = channel_for_read.clone();
                                let _ = window_clone.emit("chat-message", echo);
                            }
                        } else if line.contains(" NOTICE ") {
                            // Twitch refused our last message (slow mode, subs
                            // only, ban, unverified account...): no USERSTATE
                            // will come, so drop its echo and show the reason.
                            info!("[Chat] Notice: {}", line);
                            // Only a `msg_*` notice is a refusal. Room-mode
                            // announcements must not eat a pending echo. Read
                            // before parsing the text: a notice we cannot render
                            // must still drop its echo, or the next accepted
                            // message would be echoed with the refused text.
                            if irc_tag(irc_tags(line), "msg-id").unwrap_or("").starts_with("msg_") {
                                pending_for_read.lock().unwrap().pop_front();
                            }
                            if let Some(mut notice) = parse_notice(line) {
                                notice.channel = channel_for_read.clone();
                                let _ = window_clone.emit("chat-notice", notice);
                            }
                        } else if line.contains("USERNOTICE") {
                            // Handle user notices (subs, raids, etc.)
                            info!("[Chat] UserNotice: {}", line);
                        }
                    }
                }
                Err(e) => {
                    error!("[Chat] Read error: {}", e);
                    break;
                }
                _ => {}
            }
        }
        info!("[Chat] Read loop ended for #{}", channel_clone);
        // Single emit point — frontend treats duplicates as fresh disconnects.
        let _ = window_clone.emit("chat-disconnected", channel_clone.clone());
    });

    // Spawn task to handle outgoing messages and pings
    let channel_for_write = channel.clone();
    let write_task = tokio::spawn(async move {
        let mut ping_interval = tokio::time::interval(std::time::Duration::from_secs(30));
        
        loop {
            tokio::select! {
                _ = ping_interval.tick() => {
                    if write.send(Message::Text("PING :tmi.twitch.tv".into())).await.is_err() {
                        break;
                    }
                }
                msg = rx.recv() => {
                    match msg {
                        Some(text) => {
                            let irc_msg = format!("PRIVMSG #{} :{}", channel_for_write, text);
                            info!("[Chat] Sending message: {}", irc_msg);
                            match write.send(Message::Text(irc_msg.into())).await {
                                Ok(_) => {
                                    info!("[Chat] Message sent successfully");
                                    if echoes_back(&text) {
                                        let mut pending = pending_echo.lock().unwrap();
                                        // Pairing is positional, so a full queue
                                        // means we lost track. Drop everything
                                        // rather than echo the wrong text.
                                        if pending.len() >= MAX_PENDING_ECHO { pending.clear(); }
                                        pending.push_back(text);
                                    }
                                }
                                Err(e) => {
                                    error!("[Chat] Failed to send message: {}", e);
                                    break;
                                }
                            }
                        }
                        None => break,
                    }
                }
            }
        }
        info!("[Chat] Write loop ended for #{}", channel_for_write);
    });

    Ok(ChatConnection { sender: tx, read_task, write_task })
}

// The tag list is the first word of the line, and its first tag carries a
// leading '@'. Without stripping both, `irc_tag` can never match tag number one.
fn irc_tags(line: &str) -> &str {
    let head = line.split(' ').next().unwrap_or("");
    head.strip_prefix('@').unwrap_or(head)
}

// A slash command answers with a NOTICE and no USERSTATE, and only `/me` also
// posts a chat line. Queueing the others would pair their text with the next
// real message's USERSTATE and echo the wrong thing.
fn echoes_back(text: &str) -> bool {
    !text.starts_with('/') || text == "/me" || text.starts_with("/me ")
}

fn irc_tag<'a>(tags: &'a str, key: &str) -> Option<&'a str> {
    tags.split(';').find_map(|s| {
        let rest = s.strip_prefix(key)?;
        rest.strip_prefix('=')
    })
}

fn parse_notice(line: &str) -> Option<ChatNotice> {
    let (head, message) = line.split_once(" NOTICE ")?;
    let message = message.split_once(" :")?.1.trim();
    if message.is_empty() { return None; }

    Some(ChatNotice {
        msg_id: irc_tag(irc_tags(head), "msg-id").unwrap_or("").to_string(),
        message: message.to_string(),
        channel: String::new(),
    })
}

fn parse_badges(tags: &str) -> Vec<(String, String)> {
    let mut badges = Vec::new();
    for b in irc_tag(tags, "badges").unwrap_or("").split(',') {
        let pair: Vec<&str> = b.split('/').collect();
        if pair.len() == 2 {
            badges.push((pair[0].to_string(), pair[1].to_string()));
        }
    }
    badges
}

// USERSTATE carries our name, colour and badges but no message id and no emote
// ranges, so the frontend falls back to matching emotes by name.
fn parse_own_message(userstate: &str, text: &str, fallback_name: Option<&str>) -> ChatMessage {
    let tags = irc_tags(userstate);

    let user = irc_tag(tags, "display-name")
        .filter(|s| !s.is_empty())
        .or(fallback_name)
        .unwrap_or("You")
        .to_string();

    ChatMessage {
        id: format!("self-{}", ECHO_SEQ.fetch_add(1, Ordering::Relaxed)),
        user,
        message: text.to_string(),
        color: irc_tag(tags, "color").filter(|s| !s.is_empty()).map(|s| s.to_string()),
        badges: parse_badges(tags),
        emotes: Vec::new(),
        channel: String::new(),
    }
}

fn parse_irc_message(text: &str) -> Option<ChatMessage> {
    let parts: Vec<&str> = text.splitn(2, " PRIVMSG #").collect();
    if parts.len() < 2 { return None; }

    let tags_part = irc_tags(parts[0]);
    let content_parts: Vec<&str> = parts[1].splitn(2, " :").collect();
    if content_parts.len() < 2 { return None; }

    let message = content_parts[1].trim();

    let id = irc_tag(tags_part, "id").unwrap_or("").to_string();
    let user = irc_tag(tags_part, "display-name").unwrap_or("Unknown");
    let color = irc_tag(tags_part, "color")
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let badges = parse_badges(tags_part);

    let emotes_str = irc_tag(tags_part, "emotes").unwrap_or("");

    let mut emotes = Vec::new();
    if !emotes_str.is_empty() {
        // IRC tag format: id1:start-end,start-end/id2:start-end (positions are 0-indexed inclusive code points)
        for emote_group in emotes_str.split('/') {
            let group_parts: Vec<&str> = emote_group.splitn(2, ':').collect();
            if group_parts.len() != 2 { continue; }
            let id = group_parts[0];
            for range in group_parts[1].split(',') {
                let bounds: Vec<&str> = range.split('-').collect();
                if bounds.len() != 2 { continue; }
                if let (Ok(start), Ok(end)) = (bounds[0].parse::<usize>(), bounds[1].parse::<usize>()) {
                    emotes.push(EmoteRange { id: id.to_string(), start, end });
                }
            }
        }
        emotes.sort_by_key(|e| e.start);
    }

    Some(ChatMessage {
        id,
        user: user.to_string(),
        message: message.to_string(),
        color,
        badges,
        emotes,
        channel: String::new(), // Will be set by caller
    })
}
