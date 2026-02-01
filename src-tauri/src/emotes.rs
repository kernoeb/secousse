use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Emote {
    pub name: String,
    pub url: String,
}

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
                        let host_url = e.get("data").and_then(|d| d.get("host")).and_then(|h| h.get("url")).and_then(|u| u.as_str()).unwrap_or("");
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

pub async fn fetch_bttv_emotes(channel_id: &str) -> Vec<Emote> {
    let mut emotes = Vec::new();
    let client = reqwest::Client::new();

    let url = format!("https://api.betterttv.net/3/cached/users/twitch/{}", channel_id);
    if let Ok(res) = client.get(&url).send().await {
        if let Ok(json) = res.json::<serde_json::Value>().await {
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
                                if let Some(u) = urls.and_then(|u| u.get("2").or(u.get("1"))).and_then(|v| v.as_str()) {
                                    emotes.push(Emote {
                                        name: name.to_string(),
                                        url: if u.starts_with("http") { u.to_string() } else { format!("https:{}", u) },
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

pub async fn fetch_global_emotes() -> Vec<Emote> {
    let (stv_emotes, bttv_emotes) = tokio::join!(
        fetch_7tv_global_emotes(),
        fetch_bttv_global_emotes()
    );
    
    let mut emotes = Vec::new();
    emotes.extend(stv_emotes);
    emotes.extend(bttv_emotes);
    emotes
}

async fn fetch_7tv_global_emotes() -> Vec<Emote> {
    let mut emotes = Vec::new();
    let client = reqwest::Client::new();

    if let Ok(res) = client.get("https://7tv.io/v3/emote-sets/global").send().await {
        if let Ok(json) = res.json::<serde_json::Value>().await {
            if let Some(list) = json.get("emotes").and_then(|e| e.as_array()) {
                for e in list {
                    let name = e.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let host_url = e.get("data").and_then(|d| d.get("host")).and_then(|h| h.get("url")).and_then(|u| u.as_str()).unwrap_or("");
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

async fn fetch_bttv_global_emotes() -> Vec<Emote> {
    let mut emotes = Vec::new();
    let client = reqwest::Client::new();

    if let Ok(res) = client.get("https://api.betterttv.net/3/cached/emotes/global").send().await {
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
