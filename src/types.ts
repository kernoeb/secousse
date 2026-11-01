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
  /** ISO 8601 start time of the broadcast. */
  createdAt?: string;
  /** The VOD being recorded, from get_user_info. Null when the channel keeps no VODs. */
  archiveVideo?: { id: string } | null;
  game?: Game;
}

/** Response from get_vod_playback. */
export interface VodPlayback {
  url: string;
  lengthSeconds: number;
}

/** User/Channel information */
export interface UserInfo {
  id: string;
  login: string;
  displayName: string;
  profileImageURL: string;
  stream?: StreamInfo;
  /** Set by get_user_info only, and null when the channel streams alone. */
  collaboration?: Collaboration | null;
}

/** Another streamer live in the same Stream Together session. */
export interface CollabGuest {
  id: string;
  login: string;
  displayName: string;
  profileImageURL: string;
}

/** A channel's Stream Together session, from get_user_info or get_collaborations. */
export interface Collaboration {
  guests: CollabGuest[];
}

/** Response from get_collaborations, keyed by channel id. */
export type CollaborationMap = Record<string, Collaboration>;

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

/** A Twitch emote range from the IRC `emotes=` tag (positions are code-point indices). */
export interface ChatEmoteRange {
  id: string;
  start: number;
  end: number;
}

export interface ChatReplyParent {
  msg_id: string;
  user_login: string;
  display_name: string;
  body: string;
}

/** Why a message is shown greyed out: a moderator deleted it, timed out or banned its author, or cleared the chat. */
export type ChatDeletion = "message" | "timeout" | "ban" | "clear";

/** Chat message from IRC */
export interface ChatMessage {
  id: string;
  /** Display name. */
  user: string;
  /** Lowercase login. Missing on server notices. */
  login?: string;
  user_id?: string;
  message: string;
  color?: string;
  badges: [string, string][];
  emotes: ChatEmoteRange[];
  timestamp: number;
  channel: string;
  /** Client-side stable key for React. Falls back to a monotonic counter when Twitch's `id` tag is missing. */
  _renderKey?: string;
  /** Server notice (message refused, room mode changed) rather than a user message. */
  system?: boolean;
  reply?: ChatReplyParent | null;
  deleted?: ChatDeletion;
}

export interface ChatNotice {
  msg_id: string;
  message: string;
  channel: string;
}

/** Payload of `chat-clear`. No target at all means the whole chat was cleared. */
export interface ChatClear {
  channel: string;
  target_msg_id: string | null;
  target_user_id: string | null;
  target_login: string | null;
  ban_duration: number | null;
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
  /** null when GQL cannot resolve the channel (unknown id, or a transient service error) */
  user: {
    broadcastBadges: TwitchBadge[];
  } | null;
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
