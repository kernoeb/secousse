// ============================================
// Twitch API Response Types
// ============================================

/** Badge from Twitch GQL API */
export interface TwitchBadge {
  setID: string;
  version: string;
  imageURL: string;
}

/** Game/Category information */
export interface Game {
  id?: string;
  name: string;
  displayName: string;
}

/** Stream information */
export interface StreamInfo {
  id: string;
  title: string;
  viewersCount: number;
  game?: Game;
}

/** User/Channel information */
export interface UserInfo {
  id: string;
  login: string;
  displayName: string;
  profileImageURL: string;
  stream?: StreamInfo;
}

/** Self (logged-in user) information from GQL viewer query */
export interface SelfInfo {
  id: string;
  login: string;
  displayName: string;
  profileImageURL: string;
}

/** Top stream from browse/discovery */
export interface TopStream {
  id: string;
  broadcaster: {
    id: string;
    login: string;
    displayName: string;
    profileImageURL: string;
  };
  viewersCount: number;
  title: string;
  game?: Game;
  previewImageURL: string;
}

/** Search result for channel search */
export interface SearchResult {
  id: string;
  login: string;
  displayName: string;
  profileImageURL: string;
  stream?: {
    id: string;
    viewersCount: number;
    game?: {
      displayName: string;
    };
  };
}

// ============================================
// Chat Types
// ============================================

/** Chat message from IRC */
export interface ChatMessage {
  user: string;
  message: string;
  color?: string;
  badges: [string, string][];
  timestamp: number;
  channel: string;
}

// ============================================
// Emote Types
// ============================================

/** Emote from 7TV/BTTV/FFZ */
export interface Emote {
  name: string;
  url: string;
}

/** Twitch emote from Helix API */
export interface TwitchEmote {
  id: string;
  name: string;
  images: {
    url_1x: string;
    url_2x: string;
    url_4x: string;
  };
  format: string[];
  scale: string[];
  theme_mode: string[];
}

// ============================================
// GQL Response Wrapper Types
// ============================================

/** Response from get_user_info */
export interface GetUserInfoResponse {
  user: UserInfo;
}

/** Response from get_self_info */
export interface GetSelfInfoResponse {
  viewer: SelfInfo;
}

/** Edge wrapper for GQL connections */
export interface Edge<T> {
  node: T;
}

/** Response from get_followed_channels */
export interface GetFollowedChannelsResponse {
  user: {
    followedLiveUsers: {
      edges: Edge<UserInfo>[];
    };
  };
}

/** Response from get_top_streams */
export interface GetTopStreamsResponse {
  streams: {
    edges: Edge<TopStream>[];
  };
}

/** Response from search_channels */
export interface SearchChannelsResponse {
  searchUsers: {
    edges: Edge<SearchResult>[];
  };
}

/** Response from get_global_badges */
export interface GetGlobalBadgesResponse {
  badges: TwitchBadge[];
}

/** Response from get_channel_badges */
export interface GetChannelBadgesResponse {
  user: {
    broadcastBadges: TwitchBadge[];
  };
}

/** Response from get_twitch_global_emotes / get_twitch_channel_emotes */
export interface GetTwitchEmotesResponse {
  data: TwitchEmote[];
}

// ============================================
// Video Player Types
// ============================================

/** Quality level for HLS stream */
export interface QualityLevel {
  id: number;
  label: string;
  height: number;
}

// ============================================
// UI State Types
// ============================================

export type ActiveTab = "following" | "browse";
