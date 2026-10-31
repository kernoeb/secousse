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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ReplyParent {
    pub msg_id: String,
    pub user_login: String,
    pub display_name: String,
    pub body: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub id: String,
    pub user: String,
    pub login: String,
    pub user_id: String,
    pub message: String,
    pub color: Option<String>,
    pub badges: Vec<(String, String)>,
    pub emotes: Vec<EmoteRange>,
    pub reply: Option<ReplyParent>,
    pub channel: String,
}

/// A moderator removed messages. No target at all means the whole chat was cleared.
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct ChatClear {
    pub channel: String,
    pub target_msg_id: Option<String>,
    pub target_user_id: Option<String>,
    pub target_login: Option<String>,
    /// Seconds. Set for a timeout, absent for a permanent ban.
    pub ban_duration: Option<u64>,
}

pub struct OutgoingMessage {
    pub text: String,
    pub reply: Option<ReplyParent>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatNotice {
    pub msg_id: String,
    pub message: String,
    pub channel: String,
}

pub struct ChatConnection {
    pub sender: mpsc::Sender<OutgoingMessage>,
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
    let (tx, mut rx) = mpsc::channel::<OutgoingMessage>(100);

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

    let pending_echo: Arc<Mutex<VecDeque<OutgoingMessage>>> = Arc::new(Mutex::new(VecDeque::new()));
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
                        match irc_command(line) {
                            "PRIVMSG" => {
                                if let Some(mut parsed) = parse_irc_message(line) {
                                    // Include channel info so frontend can filter
                                    parsed.channel = channel_for_read.clone();
                                    let _ = window_clone.emit("chat-message", parsed);
                                }
                            }
                            "USERSTATE" => {
                                // Our own message came back as a USERSTATE, so build
                                // the echo the frontend never receives as a PRIVMSG.
                                let sent = pending_for_read.lock().unwrap().pop_front();
                                if let Some(sent) = sent {
                                    let mut echo = parse_own_message(line, &sent, self_name.as_deref());
                                    echo.channel = channel_for_read.clone();
                                    let _ = window_clone.emit("chat-message", echo);
                                }
                            }
                            "NOTICE" => {
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
                            }
                            "CLEARMSG" | "CLEARCHAT" => {
                                if let Some(mut clear) = parse_clear(line) {
                                    clear.channel = channel_for_read.clone();
                                    let _ = window_clone.emit("chat-clear", clear);
                                }
                            }
                            "USERNOTICE" => {
                                // Handle user notices (subs, raids, etc.)
                                info!("[Chat] UserNotice: {}", line);
                            }
                            _ => {}
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
                        Some(out) => {
                            let irc_msg = format_privmsg(&channel_for_write, &out);
                            info!("[Chat] Sending message: {}", irc_msg);
                            match write.send(Message::Text(irc_msg.into())).await {
                                Ok(_) => {
                                    info!("[Chat] Message sent successfully");
                                    if echoes_back(&out.text) {
                                        let mut pending = pending_echo.lock().unwrap();
                                        // Pairing is positional, so a full queue
                                        // means we lost track. Drop everything
                                        // rather than echo the wrong text.
                                        if pending.len() >= MAX_PENDING_ECHO { pending.clear(); }
                                        pending.push_back(out);
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

// The command is the first word after the optional tags and source. Matching
// on it, not on `line.contains`, keeps a chat line that quotes "PRIVMSG" or
// "NOTICE" from being taken for that command.
fn irc_command(line: &str) -> &str {
    let mut words = line.split(' ');
    let mut word = words.next().unwrap_or("");
    if word.starts_with('@') { word = words.next().unwrap_or(""); }
    if word.starts_with(':') { word = words.next().unwrap_or(""); }
    word
}

// `:login!login@login.tmi.twitch.tv` in the part before the command.
fn irc_source_login(head: &str) -> &str {
    head.split(' ')
        .find_map(|w| w.strip_prefix(':'))
        .and_then(|s| s.split('!').next())
        .unwrap_or("")
}

fn unescape_tag(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some(':') => out.push(';'),
            Some('s') => out.push(' '),
            Some('r') => out.push('\r'),
            Some('n') => out.push('\n'),
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

fn format_privmsg(channel: &str, out: &OutgoingMessage) -> String {
    // A tag value cannot hold a space or ';'. Twitch ids never do, but drop
    // the tag rather than send a broken line if one ever did.
    let reply_id = out.reply.as_ref()
        .map(|r| r.msg_id.as_str())
        .filter(|id| !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
    match reply_id {
        Some(id) => format!("@reply-parent-msg-id={} PRIVMSG #{} :{}", id, channel, out.text),
        None => format!("PRIVMSG #{} :{}", channel, out.text),
    }
}

fn parse_reply(tags: &str) -> Option<ReplyParent> {
    let msg_id = irc_tag(tags, "reply-parent-msg-id").filter(|s| !s.is_empty())?;
    let tag = |key| unescape_tag(irc_tag(tags, key).unwrap_or(""));
    let user_login = tag("reply-parent-user-login");
    let display_name = Some(tag("reply-parent-display-name"))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| user_login.clone());
    Some(ReplyParent {
        msg_id: msg_id.to_string(),
        user_login,
        display_name,
        body: tag("reply-parent-msg-body"),
    })
}

// Twitch starts a reply with "@parentlogin ". The context line already names
// the parent, so drop the mention and shift the emote ranges, which count code
// points from the start of the original text.
fn strip_reply_mention(message: &str, emotes: &mut Vec<EmoteRange>, parent_login: &str) -> String {
    if parent_login.is_empty() { return message.to_string(); }
    let prefix = format!("@{} ", parent_login);
    let matches = message.get(..prefix.len()).is_some_and(|head| head.eq_ignore_ascii_case(&prefix));
    let rest = if matches { &message[prefix.len()..] } else { "" };
    if rest.trim().is_empty() { return message.to_string(); }

    let shift = prefix.chars().count();
    emotes.retain(|e| e.start >= shift);
    for e in emotes.iter_mut() {
        e.start -= shift;
        e.end -= shift;
    }
    rest.to_string()
}

fn parse_clear(line: &str) -> Option<ChatClear> {
    let tags = irc_tags(line);
    let tag = |key| irc_tag(tags, key).filter(|s| !s.is_empty()).map(str::to_string);
    match irc_command(line) {
        "CLEARMSG" => Some(ChatClear {
            target_msg_id: Some(tag("target-msg-id")?),
            target_login: tag("login"),
            ..Default::default()
        }),
        "CLEARCHAT" => {
            let target_login = line.split_once(" CLEARCHAT ")
                .and_then(|(_, rest)| rest.split_once(" :"))
                .map(|(_, login)| login.trim().to_lowercase())
                .filter(|s| !s.is_empty());
            Some(ChatClear {
                target_user_id: tag("target-user-id"),
                target_login,
                ban_duration: tag("ban-duration").and_then(|s| s.parse().ok()),
                ..Default::default()
            })
        }
        _ => None,
    }
}

// USERSTATE carries our name, colour and badges, and the message id when it
// answers a PRIVMSG, but no emote ranges: the frontend matches emotes by name.
fn parse_own_message(userstate: &str, sent: &OutgoingMessage, fallback_name: Option<&str>) -> ChatMessage {
    let tags = irc_tags(userstate);
    let login = fallback_name.unwrap_or("").to_lowercase();

    let user = irc_tag(tags, "display-name")
        .filter(|s| !s.is_empty())
        .map(unescape_tag)
        .or(fallback_name.map(str::to_string))
        .unwrap_or_else(|| "You".to_string());

    let id = irc_tag(tags, "id")
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("self-{}", ECHO_SEQ.fetch_add(1, Ordering::Relaxed)));

    let mut emotes = Vec::new();
    let message = match &sent.reply {
        Some(r) => strip_reply_mention(&sent.text, &mut emotes, &r.user_login),
        None => sent.text.clone(),
    };

    ChatMessage {
        id,
        user,
        login,
        user_id: String::new(),
        message,
        color: irc_tag(tags, "color").filter(|s| !s.is_empty()).map(|s| s.to_string()),
        badges: parse_badges(tags),
        emotes,
        reply: sent.reply.clone(),
        channel: String::new(),
    }
}

fn parse_irc_message(text: &str) -> Option<ChatMessage> {
    let parts: Vec<&str> = text.splitn(2, " PRIVMSG #").collect();
    if parts.len() < 2 { return None; }

    let tags_part = irc_tags(parts[0]);
    let content_parts: Vec<&str> = parts[1].splitn(2, " :").collect();
    if content_parts.len() < 2 { return None; }

    // Only the end: trimming the start would shift the emote ranges.
    let message = content_parts[1].trim_end();

    let id = irc_tag(tags_part, "id").unwrap_or("").to_string();
    let login = irc_source_login(parts[0]).to_lowercase();
    let user = irc_tag(tags_part, "display-name")
        .filter(|s| !s.is_empty())
        .map(unescape_tag)
        .unwrap_or_else(|| if login.is_empty() { "Unknown".to_string() } else { login.clone() });
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

    let reply = parse_reply(tags_part);
    let message = match &reply {
        Some(r) => strip_reply_mention(message, &mut emotes, &r.user_login),
        None => message.to_string(),
    };

    Some(ChatMessage {
        id,
        user,
        login,
        user_id: irc_tag(tags_part, "user-id").unwrap_or("").to_string(),
        message,
        color,
        badges,
        emotes,
        reply,
        channel: String::new(), // Will be set by caller
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clearmsg_targets_one_message() {
        let line = "@login=ronni;room-id=;target-msg-id=abc-123;tmi-sent-ts=1642720582342 :tmi.twitch.tv CLEARMSG #dallas :HeyGuys";
        assert_eq!(irc_command(line), "CLEARMSG");
        let clear = parse_clear(line).unwrap();
        assert_eq!(clear.target_msg_id.as_deref(), Some("abc-123"));
        assert_eq!(clear.target_login.as_deref(), Some("ronni"));
        assert_eq!(clear.target_user_id, None);
    }

    #[test]
    fn clearchat_with_user_is_a_timeout_or_ban() {
        let timeout = "@ban-duration=350;room-id=12345678;target-user-id=87654321;tmi-sent-ts=1642719320727 :tmi.twitch.tv CLEARCHAT #dallas :Ronni";
        let clear = parse_clear(timeout).unwrap();
        assert_eq!(clear.target_user_id.as_deref(), Some("87654321"));
        assert_eq!(clear.target_login.as_deref(), Some("ronni"));
        assert_eq!(clear.ban_duration, Some(350));
        assert_eq!(clear.target_msg_id, None);

        let ban = "@room-id=12345678;target-user-id=87654321;tmi-sent-ts=1642715756806 :tmi.twitch.tv CLEARCHAT #dallas :ronni";
        let clear = parse_clear(ban).unwrap();
        assert_eq!(clear.target_user_id.as_deref(), Some("87654321"));
        assert_eq!(clear.ban_duration, None);
    }

    #[test]
    fn clearchat_without_user_clears_everything() {
        let line = "@room-id=12345678;tmi-sent-ts=1642715695392 :tmi.twitch.tv CLEARCHAT #dallas";
        assert_eq!(parse_clear(line).unwrap(), ChatClear::default());
    }

    #[test]
    fn reply_tags_are_unescaped_and_mention_stripped() {
        // "@Parent_User hi Kappa": Kappa sits at code points 16-20 before the strip.
        let line = "@badges=;color=#FF0000;display-name=Child\\sName;emotes=25:16-20;id=m-1;\
reply-parent-display-name=Parent_User;reply-parent-msg-body=a\\sb\\:c\\\\d\\nxé;\
reply-parent-msg-id=p-1;reply-parent-user-id=42;reply-parent-user-login=parent_user;\
user-id=7 :child!child@child.tmi.twitch.tv PRIVMSG #dallas :@Parent_User hi Kappa";
        assert_eq!(irc_command(line), "PRIVMSG");
        let msg = parse_irc_message(line).unwrap();
        assert_eq!(msg.user, "Child Name");
        assert_eq!(msg.login, "child");
        assert_eq!(msg.user_id, "7");
        assert_eq!(msg.message, "hi Kappa");
        let reply = msg.reply.unwrap();
        assert_eq!(reply.msg_id, "p-1");
        assert_eq!(reply.user_login, "parent_user");
        assert_eq!(reply.display_name, "Parent_User");
        assert_eq!(reply.body, "a b;c\\d\nxé");
        assert_eq!(msg.emotes.len(), 1);
        let chars: Vec<char> = msg.message.chars().collect();
        let name: String = chars[msg.emotes[0].start..=msg.emotes[0].end].iter().collect();
        assert_eq!(name, "Kappa");
    }

    #[test]
    fn strip_keeps_text_without_the_mention() {
        let mut emotes = vec![EmoteRange { id: "1".into(), start: 0, end: 4 }];
        assert_eq!(strip_reply_mention("Kappa hi", &mut emotes, "bob"), "Kappa hi");
        assert_eq!(emotes[0].start, 0);
        // A bare mention stays: stripping it would leave an empty line.
        assert_eq!(strip_reply_mention("@bob ", &mut Vec::new(), "bob"), "@bob ");
        // Multi-byte text before the cut must not break the byte slicing.
        assert_eq!(strip_reply_mention("éé", &mut Vec::new(), "b"), "éé");
    }

    #[test]
    fn privmsg_text_quoting_a_command_is_still_a_privmsg() {
        let line = "@id=x :a!a@a.tmi.twitch.tv PRIVMSG #dallas :say CLEARCHAT and NOTICE";
        assert_eq!(irc_command(line), "PRIVMSG");
        assert_eq!(irc_command(":tmi.twitch.tv CLEARCHAT #dallas"), "CLEARCHAT");
        assert_eq!(irc_command("PING :tmi.twitch.tv"), "PING");
    }

    #[test]
    fn reply_is_sent_with_the_parent_tag_and_echoed_with_context() {
        let reply = ReplyParent {
            msg_id: "p-1".into(),
            user_login: "bob".into(),
            display_name: "Bob".into(),
            body: "hello".into(),
        };
        let out = OutgoingMessage { text: "@bob thanks".into(), reply: Some(reply.clone()) };
        assert_eq!(format_privmsg("dallas", &out), "@reply-parent-msg-id=p-1 PRIVMSG #dallas :@bob thanks");

        let userstate = "@badges=;color=#00FF00;display-name=Me;id=m-9 :tmi.twitch.tv USERSTATE #dallas";
        let echo = parse_own_message(userstate, &out, Some("Me"));
        assert_eq!(echo.id, "m-9");
        assert_eq!(echo.login, "me");
        assert_eq!(echo.message, "thanks");
        assert_eq!(echo.reply, Some(reply));

        let plain = OutgoingMessage { text: "hi".into(), reply: None };
        assert_eq!(format_privmsg("dallas", &plain), "PRIVMSG #dallas :hi");
        let echo = parse_own_message(":tmi.twitch.tv USERSTATE #dallas", &plain, Some("me"));
        assert!(echo.id.starts_with("self-"));
    }
}
