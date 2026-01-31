use futures_util::{SinkExt, StreamExt};
use log::{info, error};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Window};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tokio::sync::mpsc;


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub id: String,
    pub user: String,
    pub message: String,
    pub color: Option<String>,
    pub badges: Vec<(String, String)>,
    pub channel: String,
}

pub struct ChatConnection {
    pub sender: mpsc::Sender<String>,
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
    
    // Spawn task to handle incoming messages
    let window_clone = window.clone();
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
                                // Include channel info so frontend can filter
                                parsed.channel = channel_for_read.clone();
                                let _ = window_clone.emit("chat-message", parsed);
                            }
                        } else if line.contains("NOTICE") {
                            // Handle notices (e.g., slow mode, sub only, etc.)
                            info!("[Chat] Notice: {}", line);
                            // Emit notice to frontend
                            let _ = window_clone.emit("chat-notice", line.to_string());
                        } else if line.contains("USERNOTICE") {
                            // Handle user notices (subs, raids, etc.)
                            info!("[Chat] UserNotice: {}", line);
                        }
                    }
                }
                Err(e) => {
                    error!("[Chat] Read error: {}", e);
                    // Emit disconnect event so frontend can reconnect
                    let _ = window_clone.emit("chat-disconnected", channel_clone.clone());
                    break;
                }
                _ => {}
            }
        }
        info!("[Chat] Read loop ended for #{}", channel_clone);
        // Emit disconnect event
        let _ = window_clone.emit("chat-disconnected", channel_clone.clone());
    });

    // Spawn task to handle outgoing messages and pings
    let channel_for_write = channel.clone();
    tokio::spawn(async move {
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

    Ok(ChatConnection { sender: tx })
}

fn parse_irc_message(text: &str) -> Option<ChatMessage> {
    let parts: Vec<&str> = text.splitn(2, " PRIVMSG #").collect();
    if parts.len() < 2 { return None; }

    let tags_part = parts[0];
    let content_parts: Vec<&str> = parts[1].splitn(2, " :").collect();
    if content_parts.len() < 2 { return None; }

    let message = content_parts[1].trim();
    
    // Extract message ID for deduplication
    let id = tags_part.split(';')
        .find(|s| s.starts_with("id="))
        .and_then(|s| s.split('=').nth(1))
        .unwrap_or("")
        .to_string();
    
    let user = tags_part.split(';')
        .find(|s| s.starts_with("display-name="))
        .and_then(|s| s.split('=').nth(1))
        .unwrap_or("Unknown");

    let color = tags_part.split(';')
        .find(|s| s.starts_with("color="))
        .and_then(|s| s.split('=').nth(1))
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let badges_str = tags_part.split(';')
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

    Some(ChatMessage {
        id,
        user: user.to_string(),
        message: message.to_string(),
        color,
        badges,
        channel: String::new(), // Will be set by caller
    })
}
