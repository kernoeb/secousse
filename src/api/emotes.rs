//! Third-party emote providers (7TV, BTTV, FFZ)
//!
//! Fetches emotes from various emote services for enhanced chat experience.
#![allow(dead_code, clippy::collapsible_if)]

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::state::Settings;

/// Represents an emote with its name and URL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Emote {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EmoteCache {
    fetched_at: u64,
    emotes: Vec<Emote>,
}

const GLOBAL_CACHE_TTL_SECS: u64 = 24 * 60 * 60;
const CHANNEL_CACHE_TTL_SECS: u64 = 6 * 60 * 60;

fn cache_dir() -> PathBuf {
    Settings::data_dir().join("emotes")
}

fn cache_path(key: &str) -> PathBuf {
    cache_dir().join(format!("{}.json", key))
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn load_cache(path: &PathBuf, ttl_secs: u64) -> Option<Vec<Emote>> {
    let contents = std::fs::read_to_string(path).ok()?;
    let cache: EmoteCache = serde_json::from_str(&contents).ok()?;
    let age = now_unix_secs().saturating_sub(cache.fetched_at);
    if age <= ttl_secs {
        Some(cache.emotes)
    } else {
        None
    }
}

fn save_cache(path: &PathBuf, emotes: &[Emote]) {
    if let Some(parent) = path.parent() {
        if std::fs::create_dir_all(parent).is_err() {
            return;
        }
    }

    let cache = EmoteCache {
        fetched_at: now_unix_secs(),
        emotes: emotes.to_vec(),
    };

    if let Ok(json) = serde_json::to_string(&cache) {
        let _ = std::fs::write(path, json);
    }
}

pub fn load_cached_global_emotes() -> Vec<Emote> {
    let path = cache_path("global");
    load_cache(&path, GLOBAL_CACHE_TTL_SECS).unwrap_or_default()
}

pub fn load_cached_channel_emotes(channel_id: &str) -> Vec<Emote> {
    let key = format!("channel_{}", channel_id);
    let path = cache_path(&key);
    load_cache(&path, CHANNEL_CACHE_TTL_SECS).unwrap_or_default()
}

/// Fetch all channel emotes from all providers
pub async fn fetch_channel_emotes(channel_id: &str) -> Vec<Emote> {
    let (stv, bttv, ffz) = tokio::join!(
        fetch_7tv_emotes(channel_id),
        fetch_bttv_emotes(channel_id),
        fetch_ffz_emotes(channel_id)
    );

    let mut emotes = Vec::with_capacity(stv.len() + bttv.len() + ffz.len());
    emotes.extend(stv);
    emotes.extend(bttv);
    emotes.extend(ffz);
    let key = format!("channel_{}", channel_id);
    let path = cache_path(&key);
    save_cache(&path, &emotes);
    emotes
}

/// Fetch all global emotes from all providers
pub async fn fetch_global_emotes() -> Vec<Emote> {
    let (stv_emotes, bttv_emotes) =
        tokio::join!(fetch_7tv_global_emotes(), fetch_bttv_global_emotes());

    let mut emotes = Vec::with_capacity(stv_emotes.len() + bttv_emotes.len());
    emotes.extend(stv_emotes);
    emotes.extend(bttv_emotes);
    let path = cache_path("global");
    save_cache(&path, &emotes);
    emotes
}

/// Fetch 7TV channel emotes
pub async fn fetch_7tv_emotes(channel_id: &str) -> Vec<Emote> {
    let url = format!("https://7tv.io/v3/users/twitch/{}", channel_id);
    let client = reqwest::Client::new();
    let mut emotes = Vec::new();

    if let Ok(res) = client.get(&url).send().await {
        if let Ok(json) = res.json::<serde_json::Value>().await {
            if let Some(set) = json.get("emote_set").and_then(|s| s.get("emotes")) {
                if let Some(list) = set.as_array() {
                    for e in list {
                        let name = e.get("name").and_then(|v| v.as_str()).unwrap_or("");
                        let host_url = e
                            .get("data")
                            .and_then(|d| d.get("host"))
                            .and_then(|h| h.get("url"))
                            .and_then(|u| u.as_str())
                            .unwrap_or("");

                        if !name.is_empty() && !host_url.is_empty() {
                            emotes.push(Emote {
                                name: name.to_string(),
                                url: format!("https:{}/2x.webp", host_url),
                            });
                        }
                    }
                }
            }
        }
    }

    emotes
}

/// Fetch BTTV channel emotes
pub async fn fetch_bttv_emotes(channel_id: &str) -> Vec<Emote> {
    let mut emotes = Vec::new();
    let client = reqwest::Client::new();

    let url = format!(
        "https://api.betterttv.net/3/cached/users/twitch/{}",
        channel_id
    );

    if let Ok(res) = client.get(&url).send().await {
        if let Ok(json) = res.json::<serde_json::Value>().await {
            // Channel emotes
            if let Some(channel_emotes) = json.get("channelEmotes").and_then(|e| e.as_array()) {
                for e in channel_emotes {
                    let id = e.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    let code = e.get("code").and_then(|v| v.as_str()).unwrap_or("");

                    if !id.is_empty() && !code.is_empty() {
                        emotes.push(Emote {
                            name: code.to_string(),
                            url: format!("https://cdn.betterttv.net/emote/{}/2x.webp", id),
                        });
                    }
                }
            }

            // Shared emotes
            if let Some(shared_emotes) = json.get("sharedEmotes").and_then(|e| e.as_array()) {
                for e in shared_emotes {
                    let id = e.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    let code = e.get("code").and_then(|v| v.as_str()).unwrap_or("");

                    if !id.is_empty() && !code.is_empty() {
                        emotes.push(Emote {
                            name: code.to_string(),
                            url: format!("https://cdn.betterttv.net/emote/{}/2x.webp", id),
                        });
                    }
                }
            }
        }
    }

    emotes
}

/// Fetch FFZ channel emotes
pub async fn fetch_ffz_emotes(channel_id: &str) -> Vec<Emote> {
    let mut emotes = Vec::new();
    let client = reqwest::Client::new();

    let url = format!("https://api.frankerfacez.com/v1/room/id/{}", channel_id);

    if let Ok(res) = client.get(&url).send().await {
        if let Ok(json) = res.json::<serde_json::Value>().await {
            if let Some(sets) = json.get("sets").and_then(|s| s.as_object()) {
                for set in sets.values() {
                    if let Some(emoticons) = set.get("emoticons").and_then(|e| e.as_array()) {
                        for e in emoticons {
                            let name = e.get("name").and_then(|v| v.as_str()).unwrap_or("");
                            let urls = e.get("urls").and_then(|u| u.as_object());

                            if !name.is_empty() {
                                if let Some(u) = urls
                                    .and_then(|u| u.get("2").or(u.get("1")))
                                    .and_then(|v| v.as_str())
                                {
                                    emotes.push(Emote {
                                        name: name.to_string(),
                                        url: if u.starts_with("http") {
                                            u.to_string()
                                        } else {
                                            format!("https:{}", u)
                                        },
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    emotes
}

/// Fetch 7TV global emotes
async fn fetch_7tv_global_emotes() -> Vec<Emote> {
    let mut emotes = Vec::new();
    let client = reqwest::Client::new();

    if let Ok(res) = client
        .get("https://7tv.io/v3/emote-sets/global")
        .send()
        .await
    {
        if let Ok(json) = res.json::<serde_json::Value>().await {
            if let Some(list) = json.get("emotes").and_then(|e| e.as_array()) {
                for e in list {
                    let name = e.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let host_url = e
                        .get("data")
                        .and_then(|d| d.get("host"))
                        .and_then(|h| h.get("url"))
                        .and_then(|u| u.as_str())
                        .unwrap_or("");

                    if !name.is_empty() && !host_url.is_empty() {
                        emotes.push(Emote {
                            name: name.to_string(),
                            url: format!("https:{}/2x.webp", host_url),
                        });
                    }
                }
            }
        }
    }

    emotes
}

/// Fetch BTTV global emotes
async fn fetch_bttv_global_emotes() -> Vec<Emote> {
    let mut emotes = Vec::new();
    let client = reqwest::Client::new();

    if let Ok(res) = client
        .get("https://api.betterttv.net/3/cached/emotes/global")
        .send()
        .await
    {
        if let Ok(json) = res.json::<serde_json::Value>().await {
            if let Some(list) = json.as_array() {
                for e in list {
                    let id = e.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    let code = e.get("code").and_then(|v| v.as_str()).unwrap_or("");

                    if !id.is_empty() && !code.is_empty() {
                        emotes.push(Emote {
                            name: code.to_string(),
                            url: format!("https://cdn.betterttv.net/emote/{}/2x.webp", id),
                        });
                    }
                }
            }
        }
    }

    emotes
}
