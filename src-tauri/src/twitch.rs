use log::info;
use serde::{Deserialize, Serialize};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, ACCEPT};
use anyhow::Result;
use uuid::Uuid;

// Twitch internal GQL client ID (required for GQL API access - custom client IDs don't work)
pub const GQL_CLIENT_ID: &str = "kd1unb4b3q4t58fwlpcbzcbnm76a8fp";
// Secousse app Client ID - used for OAuth and Helix API
// Redirect URI: http://localhost:17563
pub const CLIENT_ID: &str = "jm293pd1wulfgmdfb8lsw2nkjp2717";
pub const HELIX_API_URL: &str = "https://api.twitch.tv/helix";
pub const GQL_URL: &str = "https://gql.twitch.tv/gql/";
pub const CHROME_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

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

#[derive(Debug, Serialize, Deserialize)]
pub struct AccessToken {
    pub signature: String,
    pub value: String,
}

pub struct TwitchClient {
    pub client: reqwest::Client,
    pub access_token: Option<String>,
    device_id: String,
}

impl TwitchClient {
    pub fn new(access_token: Option<String>, device_id: Option<String>) -> Self {
        let device_id = device_id.unwrap_or_else(|| Uuid::new_v4().to_string().replace("-", "")[..32].to_string());
        info!("TwitchClient using device_id: {}", device_id);
        
        let client = reqwest::Client::builder()
            .user_agent(CHROME_UA)
            .tcp_nodelay(true)
            .build()
            .unwrap();

        Self {
            client,
            access_token,
            device_id,
        }
    }

    /// Headers for GQL requests (uses Twitch's internal client ID - required for GQL access)
    fn gql_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("Client-Id", HeaderValue::from_static(GQL_CLIENT_ID));
        headers.insert("X-Device-Id", HeaderValue::from_str(&self.device_id).unwrap());
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert("Origin", HeaderValue::from_static("https://www.twitch.tv"));
        headers.insert("Referer", HeaderValue::from_static("https://www.twitch.tv/"));
        // Note: GQL requests are unauthenticated - tokens from our CLIENT_ID don't work with GQL_CLIENT_ID
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

        let res = self.client.post(GQL_URL)
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

    pub async fn get_user_info(&self, login: &str) -> Result<serde_json::Value> {
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

        let res = self.client.post(GQL_URL)
            .headers(self.gql_headers())
            .json(&payload)
            .send()
            .await?;

        let gql_res = res.json::<GQLResponse<serde_json::Value>>().await?;
        if let Some(data) = gql_res.data {
            if !data["user"].is_null() {
                return Ok(data);
            }
        }
        Err(anyhow::anyhow!("User not found: {:?}", gql_res.errors))
    }

    pub async fn get_users_info(&self, logins: Vec<String>) -> Result<serde_json::Value> {
        let query = r#"
            query GetUsers($logins: [String!]) {
                users(logins: $logins) {
                    id
                    login
                    displayName
                    profileImageURL(width: 70)
                    stream {
                        viewersCount
                        game {
                            name
                        }
                    }
                }
            }
        "#;

        let payload = serde_json::json!({
            "query": query,
            "variables": { "logins": logins }
        });

        let res = self.client.post(GQL_URL)
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

    /// Get current user info using Helix API (requires authentication)
    pub async fn get_self_info(&self) -> Result<serde_json::Value> {
        let url = format!("{}/users", HELIX_API_URL);
        
        let res = self.client.get(&url)
            .headers(self.helix_headers())
            .send()
            .await?;

        let status = res.status();
        if !status.is_success() {
            let body = res.text().await?;
            return Err(anyhow::anyhow!("Helix API error {}: {}", status, body));
        }

        let data: serde_json::Value = res.json().await?;
        
        // Transform Helix response to match expected format
        if let Some(users) = data.get("data").and_then(|d| d.as_array()) {
            if let Some(user) = users.first() {
                return Ok(serde_json::json!({
                    "viewer": {
                        "id": user.get("id"),
                        "login": user.get("login"),
                        "displayName": user.get("display_name"),
                        "profileImageURL": user.get("profile_image_url")
                    }
                }));
            }
        }
        Err(anyhow::anyhow!("No user data returned"))
    }

    /// Get followed live channels using Helix API (requires authentication)
    pub async fn get_followed_channels(&self, user_id: &str) -> Result<serde_json::Value> {
        // First get followed streams
        let url = format!("{}/streams/followed?user_id={}&first=100", HELIX_API_URL, user_id);
        
        let res = self.client.get(&url)
            .headers(self.helix_headers())
            .send()
            .await?;

        let status = res.status();
        if !status.is_success() {
            let body = res.text().await?;
            return Err(anyhow::anyhow!("Helix API error {}: {}", status, body));
        }

        let streams_data: serde_json::Value = res.json().await?;
        
        // Transform Helix response to match GQL format expected by frontend
        let mut edges = Vec::new();
        
        if let Some(streams) = streams_data.get("data").and_then(|d| d.as_array()) {
            for stream in streams {
                let node = serde_json::json!({
                    "id": stream.get("user_id"),
                    "login": stream.get("user_login"),
                    "displayName": stream.get("user_name"),
                    "profileImageURL": stream.get("thumbnail_url").and_then(|t| t.as_str())
                        .map(|t| t.replace("{width}", "70").replace("{height}", "70"))
                        .unwrap_or_default(),
                    "stream": {
                        "id": stream.get("id"),
                        "viewersCount": stream.get("viewer_count"),
                        "createdAt": stream.get("started_at"),
                        "game": {
                            "id": stream.get("game_id"),
                            "displayName": stream.get("game_name"),
                            "name": stream.get("game_name")
                        }
                    }
                });
                edges.push(serde_json::json!({ "node": node }));
            }
        }
        
        // Now fetch profile images for all users
        if !edges.is_empty() {
            let user_ids: Vec<String> = edges.iter()
                .filter_map(|e| e.get("node").and_then(|n| n.get("id")).and_then(|id| id.as_str()).map(|s| s.to_string()))
                .collect();
            
            if !user_ids.is_empty() {
                let user_query = user_ids.iter().map(|id| format!("id={}", id)).collect::<Vec<_>>().join("&");
                let users_url = format!("{}/users?{}", HELIX_API_URL, user_query);
                
                if let Ok(users_res) = self.client.get(&users_url).headers(self.helix_headers()).send().await {
                    if let Ok(users_data) = users_res.json::<serde_json::Value>().await {
                        if let Some(users) = users_data.get("data").and_then(|d| d.as_array()) {
                            for edge in edges.iter_mut() {
                                if let Some(node) = edge.get_mut("node") {
                                    if let Some(node_id) = node.get("id").and_then(|id| id.as_str()) {
                                        if let Some(user) = users.iter().find(|u| u.get("id").and_then(|id| id.as_str()) == Some(node_id)) {
                                            if let Some(obj) = node.as_object_mut() {
                                                obj.insert("profileImageURL".to_string(), 
                                                    user.get("profile_image_url").cloned().unwrap_or(serde_json::Value::Null));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Ok(serde_json::json!({
            "user": {
                "followedLiveUsers": {
                    "edges": edges
                }
            }
        }))
    }

    pub fn get_usher_url(&self, login: &str, token: &AccessToken) -> String {
        let mut rng = rand::thread_rng();
        let p: u32 = rand::Rng::gen_range(&mut rng, 0..9999999);
        
        format!(
            "https://usher.ttvnw.net/api/v2/channel/hls/{}.m3u8?allow_source=true&allow_audio_only=true&fast_bread=true&p={}&sig={}&token={}",
            login, p, token.signature, urlencoding::encode(&token.value)
        )
    }

    /// Get Twitch global emotes (LUL, Kappa, etc.)
    pub async fn get_twitch_global_emotes(&self) -> Result<serde_json::Value> {
        let url = format!("{}/chat/emotes/global", HELIX_API_URL);
        
        let res = self.client.get(&url)
            .headers(self.helix_headers())
            .send()
            .await?;

        let status = res.status();
        if !status.is_success() {
            let body = res.text().await?;
            return Err(anyhow::anyhow!("Helix API error {}: {}", status, body));
        }

        let data: serde_json::Value = res.json().await?;
        Ok(data)
    }

    /// Get Twitch channel emotes (subscriber emotes)
    pub async fn get_twitch_channel_emotes(&self, channel_id: &str) -> Result<serde_json::Value> {
        let url = format!("{}/chat/emotes?broadcaster_id={}", HELIX_API_URL, channel_id);
        
        let res = self.client.get(&url)
            .headers(self.helix_headers())
            .send()
            .await?;

        let status = res.status();
        if !status.is_success() {
            let body = res.text().await?;
            return Err(anyhow::anyhow!("Helix API error {}: {}", status, body));
        }

        let data: serde_json::Value = res.json().await?;
        Ok(data)
    }

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

        let res = self.client.post(GQL_URL)
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

        let res = self.client.post(GQL_URL)
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

    pub async fn send_spade_event(&self, channel_login: &str, channel_id: &str, stream_id: &str, user_id: &str) -> Result<()> {
        let url = format!("https://www.twitch.tv/{}", channel_login);
        let res = self.client.get(&url).send().await?.text().await?;

        let settings_re = regex::Regex::new(r"https://[\w.]+/config/settings\..+?\.js").unwrap();
        let settings_url = settings_re.find(&res).map(|m| m.as_str());

        if let Some(s_url) = settings_url {
            let s_res = self.client.get(s_url).send().await?.text().await?;
            let spade_re = regex::Regex::new(r#""(?:beacon_url|spade_url)":"(.*?)"#).unwrap();
            let spade_url = spade_re.captures(&s_res).and_then(|c| c.get(1)).map(|m| m.as_str());

            if let Some(sp_url) = spade_url {
                let body = serde_json::json!({
                    "event": "minute-watched",
                    "properties": {
                        "channel_id": channel_id,
                        "broadcast_id": stream_id,
                        "player": "site",
                        "user_id": user_id.parse::<u64>().unwrap_or(0)
                    }
                }).to_string();

                let data = base64::Engine::encode(&base64::prelude::BASE64_STANDARD, body);
                let payload = format!("data={}", data);

                self.client.post(sp_url)
                    .header("Content-Type", "application/x-www-form-urlencoded")
                    .body(payload)
                    .send()
                    .await?;
                
                info!("Sent Spade minute-watched event for {}", channel_login);
            }
        }
        Ok(())
    }

    pub fn get_device_id(&self) -> &str {
        &self.device_id
    }

    pub fn is_authenticated(&self) -> bool {
        self.access_token.is_some()
    }

    pub async fn search_channels(&self, query: &str) -> Result<serde_json::Value> {
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

        let res = self.client.post(GQL_URL)
            .headers(self.gql_headers())
            .json(&payload)
            .send()
            .await?;

        let gql_res = res.json::<GQLResponse<serde_json::Value>>().await?;
        if let Some(data) = gql_res.data {
            return Ok(data);
        }
        Err(anyhow::anyhow!("Search error: {:?}", gql_res.errors))
    }

    /// Follow a user using GQL mutation (Helix API removed follow endpoints in 2023)
    pub async fn follow_user(&self, _from_user_id: &str, to_user_id: &str) -> Result<()> {
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

        // GQL mutations require authentication via the integrity token flow
        // We need to use authenticated GQL headers
        let mut headers = self.gql_headers();
        if let Some(token) = &self.access_token {
            if let Ok(val) = reqwest::header::HeaderValue::from_str(&format!("OAuth {}", token)) {
                headers.insert(reqwest::header::AUTHORIZATION, val);
            }
        }

        let res = self.client.post(GQL_URL)
            .headers(headers)
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
    pub async fn unfollow_user(&self, _from_user_id: &str, to_user_id: &str) -> Result<()> {
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

        // GQL mutations require authentication
        let mut headers = self.gql_headers();
        if let Some(token) = &self.access_token {
            if let Ok(val) = reqwest::header::HeaderValue::from_str(&format!("OAuth {}", token)) {
                headers.insert(reqwest::header::AUTHORIZATION, val);
            }
        }

        let res = self.client.post(GQL_URL)
            .headers(headers)
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
        let url = format!("{}/channels/followed?user_id={}&broadcaster_id={}", 
            HELIX_API_URL, from_user_id, to_user_id);
        
        let res = self.client.get(&url)
            .headers(self.helix_headers())
            .send()
            .await?;

        let status = res.status();
        if !status.is_success() {
            return Ok(false);
        }

        let data: serde_json::Value = res.json().await?;
        // If data array is non-empty, user is following
        Ok(data.get("data").and_then(|d| d.as_array()).map(|a| !a.is_empty()).unwrap_or(false))
    }

    /// Get top live streams using GQL
    pub async fn get_top_streams(&self, limit: u32) -> Result<serde_json::Value> {
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

        let res = self.client.post(GQL_URL)
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
}
