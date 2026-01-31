pub mod twitch;
pub mod chat;
pub mod emotes;

use log::{info, error};
use tauri::{State, Window, Manager, Emitter};
use twitch::{TwitchClient, CHROME_UA};
use tokio::sync::Mutex;
use emotes::Emote;
use reqwest::header::USER_AGENT;
use tauri_plugin_store::StoreExt;

pub struct WatchState {
    pub channel_login: String,
    pub channel_id: String,
    pub stream_id: String,
    pub user_id: String,
}

pub struct AppState {
    pub twitch_client: Mutex<TwitchClient>,
    pub chat_handle: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    pub chat_sender: Mutex<Option<tokio::sync::mpsc::Sender<String>>>,
    pub watch_state: Mutex<Option<WatchState>>,
}

#[tauri::command]
async fn get_stream_url(state: State<'_, AppState>, login: String) -> Result<String, String> {
    let client = state.twitch_client.lock().await;
    let token = client.get_playback_access_token(&login).await.map_err(|e| e.to_string())?;
    Ok(client.get_usher_url(&login, &token))
}

#[tauri::command]
async fn fetch_m3u8(state: State<'_, AppState>, url: String) -> Result<String, String> {
    let client_lock = state.twitch_client.lock().await;
    let res = client_lock.client.get(&url)
        .header(USER_AGENT, CHROME_UA)
        .header("Referer", "https://www.twitch.tv/")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    Ok(res.text().await.map_err(|e| e.to_string())?)
}

#[tauri::command]
async fn fetch_bytes(state: State<'_, AppState>, url: String) -> Result<Vec<u8>, String> {
    let client_lock = state.twitch_client.lock().await;
    let res = client_lock.client.get(&url)
        .header(USER_AGENT, CHROME_UA)
        .header("Referer", "https://www.twitch.tv/")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let bytes = res.bytes().await.map_err(|e| e.to_string())?;
    Ok(bytes.to_vec())
}

#[tauri::command]
async fn get_user_info(state: State<'_, AppState>, login: String) -> Result<serde_json::Value, String> {
    let client = state.twitch_client.lock().await;
    client.get_user_info(&login).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_users_info(state: State<'_, AppState>, logins: Vec<String>) -> Result<serde_json::Value, String> {
    let client = state.twitch_client.lock().await;
    client.get_users_info(logins).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_self_info(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let client = state.twitch_client.lock().await;
    if !client.is_authenticated() {
        return Err("Not logged in".to_string());
    }
    client.get_self_info().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_followed_channels(state: State<'_, AppState>, user_id: String) -> Result<serde_json::Value, String> {
    let client = state.twitch_client.lock().await;
    if !client.is_authenticated() {
        return Err("Not logged in".to_string());
    }
    client.get_followed_channels(&user_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_global_badges(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let client = state.twitch_client.lock().await;
    client.get_global_badges().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_channel_badges(state: State<'_, AppState>, channel_id: String) -> Result<serde_json::Value, String> {
    let client = state.twitch_client.lock().await;
    client.get_channel_badges(&channel_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_channel_emotes(channel_id: String) -> Result<Vec<Emote>, String> {
    let mut all_emotes = Vec::new();
    let (stv, bttv, ffz) = tokio::join!(
        emotes::fetch_7tv_emotes(&channel_id),
        emotes::fetch_bttv_emotes(&channel_id),
        emotes::fetch_ffz_emotes(&channel_id)
    );
    all_emotes.extend(stv);
    all_emotes.extend(bttv);
    all_emotes.extend(ffz);
    Ok(all_emotes)
}

#[tauri::command]
async fn get_global_emotes() -> Result<Vec<Emote>, String> {
    Ok(emotes::fetch_global_emotes().await)
}

#[tauri::command]
async fn get_twitch_global_emotes(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let client = state.twitch_client.lock().await;
    client.get_twitch_global_emotes().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_twitch_channel_emotes(state: State<'_, AppState>, channel_id: String) -> Result<serde_json::Value, String> {
    let client = state.twitch_client.lock().await;
    client.get_twitch_channel_emotes(&channel_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn connect_to_chat(state: State<'_, AppState>, window: Window, channel: String) -> Result<(), String> {
    // Abort existing chat connection
    {
        let mut handle_lock = state.chat_handle.lock().await;
        if let Some(handle) = handle_lock.take() {
            handle.abort();
        }
    }
    
    // Clear existing sender
    {
        let mut sender_lock = state.chat_sender.lock().await;
        *sender_lock = None;
    }
    
    // Get auth info for authenticated chat
    let access_token = {
        let client = state.twitch_client.lock().await;
        client.access_token.clone()
    };
    
    // If authenticated, get the username
    let username: Option<String> = if access_token.is_some() {
        let client = state.twitch_client.lock().await;
        match client.get_self_info().await {
            Ok(data) => data.get("viewer").and_then(|v| v.get("login")).and_then(|l| l.as_str()).map(|s| s.to_string()),
            Err(_) => None,
        }
    } else {
        None
    };
    
    // Connect to chat
    match chat::connect_chat(channel.clone(), window, access_token, username).await {
        Ok(connection) => {
            let mut sender_lock = state.chat_sender.lock().await;
            *sender_lock = Some(connection.sender);
            info!("Chat connected to #{}", channel);
            Ok(())
        }
        Err(e) => {
            error!("Chat connection error: {}", e);
            Err(e.to_string())
        }
    }
}

#[tauri::command]
async fn send_chat_message(state: State<'_, AppState>, message: String) -> Result<(), String> {
    let sender_lock = state.chat_sender.lock().await;
    if let Some(sender) = &*sender_lock {
        sender.send(message).await.map_err(|e| e.to_string())
    } else {
        Err("Not connected to chat".to_string())
    }
}

#[tauri::command]
async fn update_watch_state(
    state: State<'_, AppState>, 
    channel_login: String, 
    channel_id: String, 
    stream_id: String, 
    user_id: String
) -> Result<(), String> {
    let mut watch_lock = state.watch_state.lock().await;
    *watch_lock = Some(WatchState {
        channel_login,
        channel_id,
        stream_id,
        user_id,
    });
    Ok(())
}

#[tauri::command]
async fn login(handle: tauri::AppHandle) -> Result<(), String> {
    let scopes = [
        "channel:edit:commercial", "channel:manage:broadcast", "channel:manage:moderators",
        "channel:manage:raids", "channel:manage:vips", "channel:moderate", "chat:edit",
        "chat:read", "moderator:manage:announcements", "moderator:manage:banned_users",
        "moderator:manage:chat_messages", "moderator:manage:chat_settings", "moderator:read:chatters",
        "moderator:read:followers", "user:manage:chat_color", "user:manage:whispers",
        "user:read:chat", "user:read:email", "user:read:emotes", "user:read:follows", "user:write:chat",
    ].join("+");

    let client_id = twitch::CLIENT_ID;
    let redirect_uri = "http://localhost:17563";
    
    let auth_url = format!(
        "https://id.twitch.tv/oauth2/authorize?client_id={}&redirect_uri={}&response_type=token&scope={}",
        client_id, redirect_uri, scopes
    );

    let handle_clone = handle.clone();
    
    // Start a local HTTP server to capture the OAuth redirect
    tauri::async_runtime::spawn(async move {
        use tokio::net::TcpListener;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        
        let listener = match TcpListener::bind("127.0.0.1:17563").await {
            Ok(l) => l,
            Err(e) => {
                error!("Failed to start OAuth callback server: {}", e);
                return;
            }
        };
        
        info!("OAuth callback server listening on http://localhost:17563");
        
        // Keep server running until we get a token
        loop {
            if let Ok((mut socket, _)) = listener.accept().await {
                let mut buffer = [0; 8192];
                if let Ok(n) = socket.read(&mut buffer).await {
                    let request = String::from_utf8_lossy(&buffer[..n]);
                    
                    // Check if this is the callback with the token
                    if request.contains("/callback?token=") {
                        if let Some(start) = request.find("token=") {
                            let token_start = start + 6;
                            let token_end = request[token_start..].find(|c| c == ' ' || c == '&' || c == '\r' || c == '\n')
                                .map(|i| token_start + i)
                                .unwrap_or(request.len());
                            let token = request[token_start..token_end].to_string();
                            
                            if token.len() > 10 {
                                info!("Token captured! Length: {}", token.len());
                                
                                // Update the client
                                {
                                    let state = handle_clone.state::<AppState>();
                                    let mut client_lock = state.twitch_client.lock().await;
                                    let device_id = client_lock.get_device_id().to_string();
                                    *client_lock = TwitchClient::new(Some(token.clone()), Some(device_id));
                                    info!("TwitchClient state updated with new token");
                                }
                                
                                // Save token
                                if let Ok(store) = handle_clone.store("settings.bin") {
                                    store.set("access_token", serde_json::Value::String(token.clone()));
                                    let _ = store.save();
                                    info!("Token saved to disk");
                                }
                                
                                // Emit success event
                                let _ = handle_clone.emit("login-success", token);
                                
                                // Send success response
                                let html = r#"<!DOCTYPE html>
<html>
<head><title>Secousse - Login Success</title>
<style>
body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #0e0e10; color: #efeff1; display: flex; justify-content: center; align-items: center; height: 100vh; margin: 0; flex-direction: column; }
.success { color: #00c853; font-size: 24px; margin-bottom: 10px; }
</style>
</head>
<body>
<div class="success">Login successful!</div>
<div>You can close this tab and return to Secousse.</div>
</body>
</html>"#;
                                let response = format!(
                                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}",
                                    html.len(), html
                                );
                                let _ = socket.write_all(response.as_bytes()).await;
                                
                                info!("Login complete - server stopping");
                                break;
                            }
                        }
                    }
                    
                    // Serve the token extraction page
                    // The token comes in the URL fragment (#access_token=...)
                    // which browsers don't send to servers, so we extract it via JS
                    let html = r#"<!DOCTYPE html>
<html>
<head><title>Secousse - Login</title>
<style>
body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #0e0e10; color: #efeff1; display: flex; justify-content: center; align-items: center; height: 100vh; margin: 0; flex-direction: column; }
.spinner { border: 4px solid #3f3f46; border-top: 4px solid #9146ff; border-radius: 50%; width: 40px; height: 40px; animation: spin 1s linear infinite; margin-bottom: 20px; }
@keyframes spin { 0% { transform: rotate(0deg); } 100% { transform: rotate(360deg); } }
.success { color: #00c853; }
.error { color: #ff4444; }
</style>
</head>
<body>
<div class="spinner" id="spinner"></div>
<div id="status">Processing login...</div>
<script>
const hash = window.location.hash.substring(1);
const params = new URLSearchParams(hash);
const token = params.get('access_token');
if (token) {
    fetch('/callback?token=' + token)
        .then(() => {
            document.getElementById('spinner').style.display = 'none';
            document.getElementById('status').innerHTML = '<span class="success">Login successful!</span><br><br>You can close this tab and return to Secousse.';
        })
        .catch(() => {
            document.getElementById('status').innerHTML = '<span class="error">Failed to save token</span>';
        });
} else {
    document.getElementById('spinner').style.display = 'none';
    document.getElementById('status').innerHTML = '<span class="error">No token received. Please try again.</span>';
}
</script>
</body>
</html>"#;
                    
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}",
                        html.len(), html
                    );
                    let _ = socket.write_all(response.as_bytes()).await;
                }
            }
        }
    });
    
    // Open the auth URL in the default browser
    tauri_plugin_opener::open_url(&auth_url, None::<&str>).map_err(|e| e.to_string())?;
    
    Ok(())
}

#[tauri::command]
async fn logout(state: State<'_, AppState>, handle: tauri::AppHandle) -> Result<(), String> {
    let mut client_lock = state.twitch_client.lock().await;
    let device_id = client_lock.get_device_id().to_string();
    *client_lock = TwitchClient::new(None, Some(device_id));
    if let Ok(store) = handle.store("settings.bin") {
        store.delete("access_token");
        let _ = store.save();
    }
    Ok(())
}

#[tauri::command]
async fn is_logged_in(state: State<'_, AppState>) -> Result<bool, String> {
    let client = state.twitch_client.lock().await;
    Ok(client.is_authenticated())
}

#[tauri::command]
async fn set_access_token(state: State<'_, AppState>, token: String) -> Result<(), String> {
    let mut client_lock = state.twitch_client.lock().await;
    let device_id = client_lock.get_device_id().to_string();
    *client_lock = TwitchClient::new(Some(token), Some(device_id));
    Ok(())
}

#[tauri::command]
async fn search_channels(state: State<'_, AppState>, query: String) -> Result<serde_json::Value, String> {
    let client = state.twitch_client.lock().await;
    client.search_channels(&query).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn follow_channel(state: State<'_, AppState>, from_user_id: String, to_user_id: String) -> Result<(), String> {
    let client = state.twitch_client.lock().await;
    if !client.is_authenticated() {
        return Err("Must be logged in to follow".to_string());
    }
    client.follow_user(&from_user_id, &to_user_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn unfollow_channel(state: State<'_, AppState>, from_user_id: String, to_user_id: String) -> Result<(), String> {
    let client = state.twitch_client.lock().await;
    if !client.is_authenticated() {
        return Err("Must be logged in to unfollow".to_string());
    }
    client.unfollow_user(&from_user_id, &to_user_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_top_streams(state: State<'_, AppState>, limit: Option<u32>) -> Result<serde_json::Value, String> {
    let client = state.twitch_client.lock().await;
    client.get_top_streams(limit.unwrap_or(30)).await.map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new()
            .level(log::LevelFilter::Info)
            .target(tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout))
            .build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .setup(|app| {
            let store = app.store("settings.bin")?;
            let device_id = store.get("device_id").and_then(|v| v.as_str().map(|s| s.to_string()));
            let access_token = store.get("access_token").and_then(|v| v.as_str().map(|s| s.to_string()));
            
            // Create client (token will be validated asynchronously)
            let client = TwitchClient::new(access_token.clone(), device_id.clone());
            
            if device_id.is_none() {
                let new_id = client.get_device_id().to_string();
                store.set("device_id", serde_json::Value::String(new_id));
                let _ = store.save();
            }
            
            app.manage(AppState {
                twitch_client: Mutex::new(client),
                chat_handle: Mutex::new(None),
                chat_sender: Mutex::new(None),
                watch_state: Mutex::new(None),
            });

            // Validate token on startup
            if access_token.is_some() {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let state = handle.state::<AppState>();
                    let client = state.twitch_client.lock().await;
                    
                    // Try to get self info to validate the token
                    match client.get_self_info().await {
                        Ok(_) => {
                            info!("Stored token is valid");
                        }
                        Err(e) => {
                            info!("Stored token is invalid: {}, clearing...", e);
                            drop(client); // Release lock before modifying
                            
                            // Clear invalid token
                            let mut client_lock = state.twitch_client.lock().await;
                            let device_id = client_lock.get_device_id().to_string();
                            *client_lock = TwitchClient::new(None, Some(device_id));
                            
                            if let Ok(store) = handle.store("settings.bin") {
                                store.delete("access_token");
                                let _ = store.save();
                            }
                        }
                    }
                });
            }

            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
                loop {
                    interval.tick().await;
                    let state = handle.state::<AppState>();
                    let watch_opt = state.watch_state.lock().await;
                    if let Some(w) = &*watch_opt {
                        let client = state.twitch_client.lock().await;
                        if client.is_authenticated() {
                            let _ = client.send_spade_event(&w.channel_login, &w.channel_id, &w.stream_id, &w.user_id).await;
                        }
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_stream_url, fetch_m3u8, fetch_bytes, connect_to_chat, send_chat_message,
            get_user_info, get_users_info, get_self_info, get_followed_channels,
            get_channel_emotes, get_global_emotes, get_global_badges, get_channel_badges,
            get_twitch_global_emotes, get_twitch_channel_emotes,
            login, logout, is_logged_in, update_watch_state, set_access_token,
            search_channels, follow_channel, unfollow_channel, get_top_streams
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
