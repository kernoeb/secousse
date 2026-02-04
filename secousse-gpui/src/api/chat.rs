//! Twitch IRC Chat WebSocket client
//!
//! Handles real-time chat communication via Twitch's IRC WebSocket.

use futures_util::{SinkExt, StreamExt};
use log::{error, info};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

/// A parsed chat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Unique message ID
    pub id: String,
    /// Username of the sender
    pub user: String,
    /// Message content
    pub message: String,
    /// User's chat color (hex)
    pub color: Option<String>,
    /// List of badges (set_id, version)
    pub badges: Vec<(String, String)>,
    /// Channel the message was sent in
    pub channel: String,
    /// Twitch emotes in the message (emote_id, start_pos, end_pos)
    pub emotes: Vec<(String, usize, usize)>,
}

/// Chat connection handle
pub struct ChatConnection {
    /// Sender for outgoing messages
    pub sender: mpsc::Sender<String>,
    /// Receiver for incoming messages
    pub receiver: mpsc::Receiver<ChatEvent>,
}

/// Events emitted by the chat connection
#[derive(Debug, Clone)]
pub enum ChatEvent {
    /// A new chat message was received
    Message(ChatMessage),
    /// A notice was received (slow mode, sub only, etc.)
    Notice(String),
    /// Connection was established
    Connected,
    /// Connection was lost
    Disconnected(String),
}

/// Connect to Twitch IRC chat for a channel
pub async fn connect_chat(
    channel: String,
    access_token: Option<String>,
    username: Option<String>,
) -> anyhow::Result<ChatConnection> {
    let url = "wss://irc-ws.chat.twitch.tv:443";
    let (ws_stream, _) = connect_async(url).await?;
    let (mut write, mut read) = ws_stream.split();

    // Create channels for message passing
    let (tx_out, mut rx_out) = mpsc::channel::<String>(100);
    let (tx_in, rx_in) = mpsc::channel::<ChatEvent>(100);

    // Send initial IRC commands
    write
        .send(Message::Text(
            "CAP REQ :twitch.tv/tags twitch.tv/commands".into(),
        ))
        .await?;

    // Use authenticated or anonymous connection
    if let (Some(token), Some(user)) = (&access_token, &username) {
        write
            .send(Message::Text(format!("PASS oauth:{}", token).into()))
            .await?;
        write
            .send(Message::Text(format!("NICK {}", user.to_lowercase()).into()))
            .await?;
        info!("[Chat] Connecting as authenticated user: {}", user);
    } else {
        write.send(Message::Text("PASS SCHMOOPIE".into())).await?;
        write
            .send(Message::Text("NICK justinfan12345".into()))
            .await?;
        info!("[Chat] Connecting as anonymous user");
    }

    write
        .send(Message::Text(format!("JOIN #{}", channel).into()))
        .await?;

    let tx_in_clone = tx_in.clone();
    let _ = tx_in_clone.send(ChatEvent::Connected).await;

    let channel_for_read = channel.clone();
    let channel_for_write = channel.clone();

    // Spawn task to handle incoming messages
    let tx_in_read = tx_in.clone();
    tokio::spawn(async move {
        while let Some(msg) = read.next().await {
            match msg {
                Ok(msg) if msg.is_text() => {
                    let text = msg.to_text().unwrap_or("");
                    for line in text.lines() {
                        if line.starts_with("PING") {
                            // PING handled in write task
                        } else if line.contains("PRIVMSG") {
                            if let Some(mut parsed) = parse_irc_message(line) {
                                parsed.channel = channel_for_read.clone();
                                let _ = tx_in_read.send(ChatEvent::Message(parsed)).await;
                            }
                        } else if line.contains("NOTICE") {
                            info!("[Chat] Notice: {}", line);
                            let _ = tx_in_read.send(ChatEvent::Notice(line.to_string())).await;
                        } else if line.contains("USERNOTICE") {
                            info!("[Chat] UserNotice: {}", line);
                        }
                    }
                }
                Err(e) => {
                    error!("[Chat] Read error: {}", e);
                    let _ = tx_in_read
                        .send(ChatEvent::Disconnected(channel_for_read.clone()))
                        .await;
                    break;
                }
                _ => {}
            }
        }
        info!("[Chat] Read loop ended for #{}", channel_for_read);
        let _ = tx_in_read
            .send(ChatEvent::Disconnected(channel_for_read))
            .await;
    });

    // Spawn task to handle outgoing messages and pings
    tokio::spawn(async move {
        let mut ping_interval = tokio::time::interval(std::time::Duration::from_secs(30));

        loop {
            tokio::select! {
                _ = ping_interval.tick() => {
                    if write.send(Message::Text("PING :tmi.twitch.tv".into())).await.is_err() {
                        break;
                    }
                }
                msg = rx_out.recv() => {
                    match msg {
                        Some(text) => {
                            let irc_msg = format!("PRIVMSG #{} :{}", channel_for_write, text);
                            info!("[Chat] Sending message: {}", irc_msg);
                            match write.send(Message::Text(irc_msg.into())).await {
                                Ok(_) => info!("[Chat] Message sent successfully"),
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

    Ok(ChatConnection {
        sender: tx_out,
        receiver: rx_in,
    })
}

/// Parse an IRC PRIVMSG line into a ChatMessage
fn parse_irc_message(text: &str) -> Option<ChatMessage> {
    let parts: Vec<&str> = text.splitn(2, " PRIVMSG #").collect();
    if parts.len() < 2 {
        return None;
    }

    let tags_part = parts[0];
    let content_parts: Vec<&str> = parts[1].splitn(2, " :").collect();
    if content_parts.len() < 2 {
        return None;
    }

    let message = content_parts[1].trim();

    // Extract message ID for deduplication
    let id = tags_part
        .split(';')
        .find(|s| s.starts_with("id="))
        .and_then(|s| s.split('=').nth(1))
        .unwrap_or("")
        .to_string();

    let user = tags_part
        .split(';')
        .find(|s| s.starts_with("display-name="))
        .and_then(|s| s.split('=').nth(1))
        .unwrap_or("Unknown");

    let color = tags_part
        .split(';')
        .find(|s| s.starts_with("color="))
        .and_then(|s| s.split('=').nth(1))
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let badges_str = tags_part
        .split(';')
        .find(|s| s.starts_with("badges="))
        .and_then(|s| s.split('=').nth(1))
        .unwrap_or("");

    let mut badges = Vec::new();
    for b in badges_str.split(',') {
        let pair: Vec<&str> = b.split('/').collect();
        if pair.len() == 2 {
            badges.push((pair[0].to_string(), pair[1].to_string()));
        }
    }

    // Parse Twitch emotes from tags
    let emotes = parse_emotes_tag(tags_part);

    Some(ChatMessage {
        id,
        user: user.to_string(),
        message: message.to_string(),
        color,
        badges,
        channel: String::new(), // Will be set by caller
        emotes,
    })
}

/// Parse emotes tag to get emote positions
/// Format: emotes=emote_id:start-end,start-end/emote_id:start-end
fn parse_emotes_tag(tags: &str) -> Vec<(String, usize, usize)> {
    let mut result = Vec::new();

    let emotes_str = tags
        .split(';')
        .find(|s| s.starts_with("emotes="))
        .and_then(|s| s.split('=').nth(1))
        .unwrap_or("");

    if emotes_str.is_empty() {
        return result;
    }

    for emote_part in emotes_str.split('/') {
        let parts: Vec<&str> = emote_part.splitn(2, ':').collect();
        if parts.len() != 2 {
            continue;
        }

        let emote_id = parts[0];
        for range in parts[1].split(',') {
            let positions: Vec<&str> = range.split('-').collect();
            if positions.len() == 2 {
                if let (Ok(start), Ok(end)) = (
                    positions[0].parse::<usize>(),
                    positions[1].parse::<usize>(),
                ) {
                    result.push((emote_id.to_string(), start, end));
                }
            }
        }
    }

    // Sort by start position
    result.sort_by_key(|(_, start, _)| *start);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_irc_message() {
        let line = "@badge-info=;badges=moderator/1;color=#FF0000;display-name=TestUser;emotes=;id=abc123;mod=1;room-id=12345;subscriber=0;tmi-sent-ts=1234567890;turbo=0;user-id=67890;user-type=mod :testuser!testuser@testuser.tmi.twitch.tv PRIVMSG #channel :Hello World";
        
        let msg = parse_irc_message(line).unwrap();
        assert_eq!(msg.user, "TestUser");
        assert_eq!(msg.message, "Hello World");
        assert_eq!(msg.id, "abc123");
        assert_eq!(msg.color, Some("#FF0000".to_string()));
        assert!(msg.badges.iter().any(|(k, v)| k == "moderator" && v == "1"));
    }

    #[test]
    fn test_parse_emotes() {
        let tags = "emotes=25:0-4,6-10/1902:16-20";
        let emotes = parse_emotes_tag(tags);
        
        assert_eq!(emotes.len(), 3);
        assert_eq!(emotes[0], ("25".to_string(), 0, 4));
        assert_eq!(emotes[1], ("25".to_string(), 6, 10));
        assert_eq!(emotes[2], ("1902".to_string(), 16, 20));
    }
}
