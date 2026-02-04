//! Twitch API client
//!
//! Handles all communication with Twitch's GQL and Helix APIs.

use anyhow::Result;
use log::info;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Twitch internal GQL client ID (required for GQL API access - custom client IDs don't work)
pub const GQL_CLIENT_ID: &str = "kd1unb4b3q4t58fwlpcbzcbnm76a8fp";

// Secousse app Client ID - used for OAuth and Helix API
// Redirect URI: http://localhost:17563
pub const CLIENT_ID: &str = "jm293pd1wulfgmdfb8lsw2nkjp2717";

pub const HELIX_API_URL: &str = "https://api.twitch.tv/helix";
pub const GQL_URL: &str = "https://gql.twitch.tv/gql/";
pub const CHROME_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

// OAuth configuration
pub const OAUTH_REDIRECT_PORT: u16 = 17563;
pub const OAUTH_REDIRECT_URI: &str = "http://localhost:17563";
pub const OAUTH_SCOPES: &[&str] = &[
    "user:read:follows",
    "user:read:email",
    "chat:read",
    "chat:edit",
];

#[derive(Debug, Serialize, Deserialize)]
pub struct GQLResponse<T> {
    pub data: Option<T>,
    pub errors: Option<Vec<GQLError>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GQLError {
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PlaybackAccessTokenResponse {
    #[serde(rename = "streamPlaybackAccessToken")]
    pub stream_playback_access_token: Option<AccessToken>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessToken {
    pub signature: String,
    pub value: String,
}

/// User information from Twitch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: String,
    pub login: String,
    pub display_name: String,
    pub profile_image_url: Option<String>,
}

/// Stream information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamInfo {
    pub id: String,
    pub title: String,
    pub viewer_count: u32,
    pub game_name: Option<String>,
    pub game_id: Option<String>,
    pub started_at: Option<String>,
    pub thumbnail_url: Option<String>,
}

/// Combined channel info with user and stream data
#[derive(Debug, Clone)]
pub struct ChannelInfo {
    pub user: UserInfo,
    pub stream: Option<StreamInfo>,
}

/// Twitch API client
pub struct TwitchClient {
    pub client: reqwest::Client,
    pub access_token: Option<String>,
    device_id: String,
}

impl TwitchClient {
    /// Create a new Twitch client
    pub fn new(access_token: Option<String>, device_id: Option<String>) -> Self {
        let device_id = device_id.unwrap_or_else(|| {
            Uuid::new_v4()
                .to_string()
                .replace("-", "")
                .chars()
                .take(32)
                .collect()
        });
        
        info!("TwitchClient using device_id: {}", device_id);

        let client = reqwest::Client::builder()
            .user_agent(CHROME_UA)
            .tcp_nodelay(true)
            .build()
            .expect("Failed to build HTTP client");

        Self {
            client,
            access_token,
            device_id,
        }
    }

    /// Headers for GQL requests (uses Twitch's internal client ID)
    fn gql_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("Client-Id", HeaderValue::from_static(GQL_CLIENT_ID));
        headers.insert(
            "X-Device-Id",
            HeaderValue::from_str(&self.device_id).unwrap(),
        );
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert("Origin", HeaderValue::from_static("https://www.twitch.tv"));
        headers.insert("Referer", HeaderValue::from_static("https://www.twitch.tv/"));
        headers
    }

    /// Headers for GQL requests with authentication
    fn gql_headers_auth(&self) -> HeaderMap {
        let mut headers = self.gql_headers();
        if let Some(token) = &self.access_token {
            if let Ok(val) = HeaderValue::from_str(&format!("OAuth {}", token)) {
                headers.insert(AUTHORIZATION, val);
            }
        }
        headers
    }

    /// Headers for Helix API requests
    fn helix_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("Client-Id", HeaderValue::from_static(CLIENT_ID));
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));

        if let Some(token) = &self.access_token {
            if let Ok(val) = HeaderValue::from_str(&format!("Bearer {}", token)) {
                headers.insert(AUTHORIZATION, val);
            }
        }
        headers
    }

    /// Get playback access token for a stream
    pub async fn get_playback_access_token(&self, login: &str) -> Result<AccessToken> {
        let payload = serde_json::json!({
            "operationName": "PlaybackAccessToken",
            "variables": {
                "isLive": true,
                "login": login,
                "isVod": false,
                "vodID": "",
                "platform": "web",
                "playerType": "site"
            },
            "extensions": {
                "persistedQuery": {
                    "version": 1,
                    "sha256Hash": "ed230aa1e33e07eebb8928504583da78a5173989fadfb1ac94be06a04f3cdbe9"
                }
            }
        });

        let res = self
            .client
            .post(GQL_URL)
            .headers(self.gql_headers())
            .json(&payload)
            .send()
            .await?;

        let gql_res = res.json::<GQLResponse<PlaybackAccessTokenResponse>>().await?;
        
        if let Some(data) = gql_res.data {
            if let Some(token) = data.stream_playback_access_token {
                return Ok(token);
            }
        }
        
        Err(anyhow::anyhow!("GQL Error: {:?}", gql_res.errors))
    }

    /// Get user information by login name
    pub async fn get_user_info(&self, login: &str) -> Result<ChannelInfo> {
        let query = r#"
            query GetUser($login: String!) {
                user(login: $login) {
                    id
                    login
                    displayName
                    profileImageURL(width: 300)
                    stream {
                        id
                        title
                        viewersCount
                        createdAt
                        game {
                            id
                            displayName
                        }
                    }
                }
            }
        "#;

        let payload = serde_json::json!({
            "query": query,
            "variables": { "login": login }
        });

        let res = self
            .client
            .post(GQL_URL)
            .headers(self.gql_headers())
            .json(&payload)
            .send()
            .await?;

        let gql_res = res.json::<GQLResponse<serde_json::Value>>().await?;
        
        if let Some(data) = gql_res.data {
            if let Some(user) = data.get("user") {
                if !user.is_null() {
                    return Ok(Self::parse_channel_info(user));
                }
            }
        }
        
        Err(anyhow::anyhow!("User not found: {:?}", gql_res.errors))
    }

    /// Parse channel info from GQL response
    fn parse_channel_info(user: &serde_json::Value) -> ChannelInfo {
        let user_info = UserInfo {
            id: user.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            login: user.get("login").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            display_name: user.get("displayName").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            profile_image_url: user.get("profileImageURL").and_then(|v| v.as_str()).map(|s| s.to_string()),
        };

        let stream = user.get("stream").and_then(|s| {
            if s.is_null() {
                None
            } else {
                Some(StreamInfo {
                    id: s.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    title: s.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    viewer_count: s.get("viewersCount").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                    game_name: s.get("game").and_then(|g| g.get("displayName")).and_then(|v| v.as_str()).map(|s| s.to_string()),
                    game_id: s.get("game").and_then(|g| g.get("id")).and_then(|v| v.as_str()).map(|s| s.to_string()),
                    started_at: s.get("createdAt").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    thumbnail_url: None,
                })
            }
        });

        ChannelInfo { user: user_info, stream }
    }

    /// Get current user info using Helix API (requires authentication)
    pub async fn get_self_info(&self) -> Result<UserInfo> {
        let url = format!("{}/users", HELIX_API_URL);

        let res = self
            .client
            .get(&url)
            .headers(self.helix_headers())
            .send()
            .await?;

        let status = res.status();
        if !status.is_success() {
            let body = res.text().await?;
            return Err(anyhow::anyhow!("Helix API error {}: {}", status, body));
        }

        let data: serde_json::Value = res.json().await?;

        if let Some(users) = data.get("data").and_then(|d| d.as_array()) {
            if let Some(user) = users.first() {
                return Ok(UserInfo {
                    id: user.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    login: user.get("login").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    display_name: user.get("display_name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    profile_image_url: user.get("profile_image_url").and_then(|v| v.as_str()).map(|s| s.to_string()),
                });
            }
        }
        
        Err(anyhow::anyhow!("No user data returned"))
    }

    /// Get followed live channels using Helix API
    pub async fn get_followed_live_streams(&self, user_id: &str) -> Result<Vec<ChannelInfo>> {
        let url = format!(
            "{}/streams/followed?user_id={}&first=100",
            HELIX_API_URL, user_id
        );

        let res = self
            .client
            .get(&url)
            .headers(self.helix_headers())
            .send()
            .await?;

        let status = res.status();
        if !status.is_success() {
            let body = res.text().await?;
            return Err(anyhow::anyhow!("Helix API error {}: {}", status, body));
        }

        let streams_data: serde_json::Value = res.json().await?;
        let mut channels = Vec::new();

        if let Some(streams) = streams_data.get("data").and_then(|d| d.as_array()) {
            // Collect user IDs to fetch profile images
            let user_ids: Vec<String> = streams
                .iter()
                .filter_map(|s| s.get("user_id").and_then(|v| v.as_str()))
                .map(|s| s.to_string())
                .collect();

            // Fetch user profile images
            let user_images = self.get_users_by_ids(&user_ids).await.unwrap_or_default();

            for stream in streams {
                let user_id = stream.get("user_id").and_then(|v| v.as_str()).unwrap_or("");
                let profile_image = user_images.get(user_id).cloned();

                let user = UserInfo {
                    id: user_id.to_string(),
                    login: stream.get("user_login").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    display_name: stream.get("user_name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    profile_image_url: profile_image,
                };

                let stream_info = StreamInfo {
                    id: stream.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    title: stream.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    viewer_count: stream.get("viewer_count").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                    game_name: stream.get("game_name").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    game_id: stream.get("game_id").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    started_at: stream.get("started_at").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    thumbnail_url: stream.get("thumbnail_url").and_then(|v| v.as_str()).map(|s| s.to_string()),
                };

                channels.push(ChannelInfo {
                    user,
                    stream: Some(stream_info),
                });
            }
        }

        Ok(channels)
    }

    /// Get users by IDs (helper for profile images)
    async fn get_users_by_ids(&self, user_ids: &[String]) -> Result<std::collections::HashMap<String, String>> {
        if user_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }

        let query = user_ids
            .iter()
            .map(|id| format!("id={}", id))
            .collect::<Vec<_>>()
            .join("&");
        let url = format!("{}/users?{}", HELIX_API_URL, query);

        let res = self
            .client
            .get(&url)
            .headers(self.helix_headers())
            .send()
            .await?;

        let data: serde_json::Value = res.json().await?;
        let mut map = std::collections::HashMap::new();

        if let Some(users) = data.get("data").and_then(|d| d.as_array()) {
            for user in users {
                if let (Some(id), Some(url)) = (
                    user.get("id").and_then(|v| v.as_str()),
                    user.get("profile_image_url").and_then(|v| v.as_str()),
                ) {
                    map.insert(id.to_string(), url.to_string());
                }
            }
        }

        Ok(map)
    }

    /// Construct Usher URL for HLS stream
    pub fn get_usher_url(&self, login: &str, token: &AccessToken) -> String {
        let mut rng = rand::thread_rng();
        let p: u32 = rand::Rng::gen_range(&mut rng, 0..9999999);

        format!(
            "https://usher.ttvnw.net/api/v2/channel/hls/{}.m3u8?allow_source=true&allow_audio_only=true&fast_bread=true&p={}&sig={}&token={}",
            login, p, token.signature, urlencoding::encode(&token.value)
        )
    }

    /// Get Twitch global emotes
    pub async fn get_twitch_global_emotes(&self) -> Result<serde_json::Value> {
        let url = format!("{}/chat/emotes/global", HELIX_API_URL);

        let res = self
            .client
            .get(&url)
            .headers(self.helix_headers())
            .send()
            .await?;

        let status = res.status();
        if !status.is_success() {
            let body = res.text().await?;
            return Err(anyhow::anyhow!("Helix API error {}: {}", status, body));
        }

        Ok(res.json().await?)
    }

    /// Get Twitch channel emotes
    pub async fn get_twitch_channel_emotes(&self, channel_id: &str) -> Result<serde_json::Value> {
        let url = format!("{}/chat/emotes?broadcaster_id={}", HELIX_API_URL, channel_id);

        let res = self
            .client
            .get(&url)
            .headers(self.helix_headers())
            .send()
            .await?;

        let status = res.status();
        if !status.is_success() {
            let body = res.text().await?;
            return Err(anyhow::anyhow!("Helix API error {}: {}", status, body));
        }

        Ok(res.json().await?)
    }

    /// Get global badges
    pub async fn get_global_badges(&self) -> Result<serde_json::Value> {
        let query = r#"
            query Badges {
                badges {
                    imageURL(size: DOUBLE)
                    setID
                    title
                    version
                }
            }
        "#;

        let payload = serde_json::json!({
            "query": query,
            "variables": {}
        });

        let res = self
            .client
            .post(GQL_URL)
            .headers(self.gql_headers())
            .json(&payload)
            .send()
            .await?;

        let gql_res = res.json::<GQLResponse<serde_json::Value>>().await?;
        
        if let Some(data) = gql_res.data {
            return Ok(data);
        }
        
        Err(anyhow::anyhow!("GQL Error: {:?}", gql_res.errors))
    }

    /// Get channel badges
    pub async fn get_channel_badges(&self, channel_id: &str) -> Result<serde_json::Value> {
        let query = r#"
            query UserBadges($id: ID) {
                user(id: $id, lookupType: ALL) {
                    broadcastBadges {
                        imageURL(size: DOUBLE)
                        setID
                        title
                        version
                    }
                }
            }
        "#;

        let payload = serde_json::json!({
            "query": query,
            "variables": { "id": channel_id }
        });

        let res = self
            .client
            .post(GQL_URL)
            .headers(self.gql_headers())
            .json(&payload)
            .send()
            .await?;

        let gql_res = res.json::<GQLResponse<serde_json::Value>>().await?;
        
        if let Some(data) = gql_res.data {
            return Ok(data);
        }
        
        Err(anyhow::anyhow!("GQL Error: {:?}", gql_res.errors))
    }

    /// Search for channels
    pub async fn search_channels(&self, query: &str) -> Result<Vec<ChannelInfo>> {
        let gql_query = r#"
            query SearchChannels($query: String!, $first: Int) {
                searchUsers(userQuery: $query, first: $first) {
                    edges {
                        node {
                            id
                            login
                            displayName
                            profileImageURL(width: 70)
                            stream {
                                id
                                viewersCount
                                game {
                                    displayName
                                }
                            }
                        }
                    }
                }
            }
        "#;

        let payload = serde_json::json!({
            "query": gql_query,
            "variables": { "query": query, "first": 20 }
        });

        let res = self
            .client
            .post(GQL_URL)
            .headers(self.gql_headers())
            .json(&payload)
            .send()
            .await?;

        let gql_res = res.json::<GQLResponse<serde_json::Value>>().await?;
        let mut channels = Vec::new();

        if let Some(data) = gql_res.data {
            if let Some(edges) = data.get("searchUsers").and_then(|s| s.get("edges")).and_then(|e| e.as_array()) {
                for edge in edges {
                    if let Some(node) = edge.get("node") {
                        channels.push(Self::parse_channel_info(node));
                    }
                }
            }
        }

        Ok(channels)
    }

    /// Get top live streams
    pub async fn get_top_streams(&self, limit: u32) -> Result<Vec<ChannelInfo>> {
        let query = r#"
            query GetTopStreams($first: Int) {
                streams(first: $first) {
                    edges {
                        node {
                            id
                            broadcaster {
                                id
                                login
                                displayName
                                profileImageURL(width: 70)
                            }
                            viewersCount
                            title
                            game {
                                id
                                displayName
                                name
                            }
                            previewImageURL(width: 440, height: 248)
                        }
                    }
                }
            }
        "#;

        let payload = serde_json::json!({
            "query": query,
            "variables": { "first": limit }
        });

        let res = self
            .client
            .post(GQL_URL)
            .headers(self.gql_headers())
            .json(&payload)
            .send()
            .await?;

        let gql_res = res.json::<GQLResponse<serde_json::Value>>().await?;
        let mut channels = Vec::new();

        if let Some(data) = gql_res.data {
            if let Some(edges) = data.get("streams").and_then(|s| s.get("edges")).and_then(|e| e.as_array()) {
                for edge in edges {
                    if let Some(node) = edge.get("node") {
                        let broadcaster = node.get("broadcaster");
                        
                        let user = UserInfo {
                            id: broadcaster.and_then(|b| b.get("id")).and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            login: broadcaster.and_then(|b| b.get("login")).and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            display_name: broadcaster.and_then(|b| b.get("displayName")).and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            profile_image_url: broadcaster.and_then(|b| b.get("profileImageURL")).and_then(|v| v.as_str()).map(|s| s.to_string()),
                        };

                        let stream = StreamInfo {
                            id: node.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            title: node.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            viewer_count: node.get("viewersCount").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                            game_name: node.get("game").and_then(|g| g.get("displayName")).and_then(|v| v.as_str()).map(|s| s.to_string()),
                            game_id: node.get("game").and_then(|g| g.get("id")).and_then(|v| v.as_str()).map(|s| s.to_string()),
                            started_at: None,
                            thumbnail_url: node.get("previewImageURL").and_then(|v| v.as_str()).map(|s| s.to_string()),
                        };

                        channels.push(ChannelInfo { user, stream: Some(stream) });
                    }
                }
            }
        }

        Ok(channels)
    }

    /// Follow a user using GQL mutation
    pub async fn follow_user(&self, to_user_id: &str) -> Result<()> {
        let gql_query = r#"
            mutation FollowButton_FollowUser($input: FollowUserInput!) {
                followUser(input: $input) {
                    follow {
                        disableNotifications
                        user {
                            id
                            displayName
                        }
                    }
                }
            }
        "#;

        let payload = serde_json::json!({
            "query": gql_query,
            "variables": {
                "input": {
                    "targetID": to_user_id,
                    "disableNotifications": false
                }
            }
        });

        let res = self
            .client
            .post(GQL_URL)
            .headers(self.gql_headers_auth())
            .json(&payload)
            .send()
            .await?;

        let gql_res = res.json::<GQLResponse<serde_json::Value>>().await?;
        
        if let Some(errors) = gql_res.errors {
            return Err(anyhow::anyhow!("Follow error: {:?}", errors));
        }
        
        Ok(())
    }

    /// Unfollow a user using GQL mutation
    pub async fn unfollow_user(&self, to_user_id: &str) -> Result<()> {
        let gql_query = r#"
            mutation FollowButton_UnfollowUser($input: UnfollowUserInput!) {
                unfollowUser(input: $input) {
                    follow {
                        user {
                            id
                            displayName
                        }
                    }
                }
            }
        "#;

        let payload = serde_json::json!({
            "query": gql_query,
            "variables": {
                "input": {
                    "targetID": to_user_id
                }
            }
        });

        let res = self
            .client
            .post(GQL_URL)
            .headers(self.gql_headers_auth())
            .json(&payload)
            .send()
            .await?;

        let gql_res = res.json::<GQLResponse<serde_json::Value>>().await?;
        
        if let Some(errors) = gql_res.errors {
            return Err(anyhow::anyhow!("Unfollow error: {:?}", errors));
        }
        
        Ok(())
    }

    /// Check if user follows a channel using Helix API
    pub async fn check_follow_status(&self, from_user_id: &str, to_user_id: &str) -> Result<bool> {
        let url = format!(
            "{}/channels/followed?user_id={}&broadcaster_id={}",
            HELIX_API_URL, from_user_id, to_user_id
        );

        let res = self
            .client
            .get(&url)
            .headers(self.helix_headers())
            .send()
            .await?;

        let status = res.status();
        if !status.is_success() {
            return Ok(false);
        }

        let data: serde_json::Value = res.json().await?;
        
        Ok(data
            .get("data")
            .and_then(|d| d.as_array())
            .map(|a| !a.is_empty())
            .unwrap_or(false))
    }

    /// Send Spade analytics event (minute-watched)
    pub async fn send_spade_event(
        &self,
        channel_login: &str,
        channel_id: &str,
        stream_id: &str,
        user_id: &str,
    ) -> Result<()> {
        let url = format!("https://www.twitch.tv/{}", channel_login);
        let res = self.client.get(&url).send().await?.text().await?;

        let settings_re = regex::Regex::new(r"https://[\w.]+/config/settings\..+?\.js").unwrap();
        let settings_url = settings_re.find(&res).map(|m| m.as_str());

        if let Some(s_url) = settings_url {
            let s_res = self.client.get(s_url).send().await?.text().await?;
            let spade_re = regex::Regex::new(r#""(?:beacon_url|spade_url)":"(.*?)"#).unwrap();
            let spade_url = spade_re
                .captures(&s_res)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str());

            if let Some(sp_url) = spade_url {
                let body = serde_json::json!({
                    "event": "minute-watched",
                    "properties": {
                        "channel_id": channel_id,
                        "broadcast_id": stream_id,
                        "player": "site",
                        "user_id": user_id.parse::<u64>().unwrap_or(0)
                    }
                })
                .to_string();

                let data = base64::Engine::encode(&base64::prelude::BASE64_STANDARD, body);
                let payload = format!("data={}", data);

                self.client
                    .post(sp_url)
                    .header("Content-Type", "application/x-www-form-urlencoded")
                    .body(payload)
                    .send()
                    .await?;

                info!("Sent Spade minute-watched event for {}", channel_login);
            }
        }
        
        Ok(())
    }

    /// Get device ID
    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    /// Check if client is authenticated
    pub fn is_authenticated(&self) -> bool {
        self.access_token.is_some()
    }

    /// Update access token
    pub fn set_access_token(&mut self, token: Option<String>) {
        self.access_token = token;
    }

    /// Generate OAuth URL for login
    pub fn get_oauth_url() -> String {
        let scopes = OAUTH_SCOPES.join("+");
        format!(
            "https://id.twitch.tv/oauth2/authorize?client_id={}&redirect_uri={}&response_type=token&scope={}",
            CLIENT_ID,
            urlencoding::encode(OAUTH_REDIRECT_URI),
            scopes
        )
    }
}
