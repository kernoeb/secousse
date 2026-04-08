import { Heart, Video } from "lucide-react";
import { cn, formatViewers } from "../lib/utils";
import type { UserInfo } from "../types";

interface StreamInfoProps {
  channel: string;
  userInfo: UserInfo | null;
  isFollowing: boolean;
  isLoggedIn: boolean;
  onFollow: () => void;
}

export function StreamInfo({ channel, userInfo, isFollowing, isLoggedIn, onFollow }: StreamInfoProps) {
  return (
    <div className="bg-surface p-4 flex items-start gap-4 border-t border-border">
      <div className="w-16 h-16 bg-elevated rounded-full flex-shrink-0 overflow-hidden border-2 border-twitch shadow-lg shadow-twitch/10">
        {userInfo?.profileImageURL && <img src={userInfo.profileImageURL} alt="pfp" />}
      </div>
      <div className="flex-1 min-w-0">
        <h1 className="text-lg font-bold truncate leading-tight tracking-tight">
          {userInfo?.stream?.title || `${channel} - Streaming`}
        </h1>
        <div className="flex items-center gap-2 mt-1 text-sm">
          <span className="text-twitch font-bold hover:underline cursor-pointer">
            {userInfo?.displayName || channel}
          </span>
          <span className="text-muted opacity-50">&bull;</span>
          <span className="text-twitch hover:underline cursor-pointer font-semibold">
            {userInfo?.stream?.game?.displayName || "Just Chatting"}
          </span>
        </div>
      </div>
      <div className="flex flex-col items-end gap-3">
        <div className="flex gap-2">
          <button
            onClick={onFollow}
            disabled={!isLoggedIn}
            className={cn(
              "px-4 py-1.5 rounded-md font-bold text-sm flex items-center gap-2 transition-all active:scale-95 text-white",
              isFollowing
                ? "bg-elevated hover:bg-hover"
                : "bg-twitch hover:bg-twitch-dark shadow-lg shadow-twitch/20",
              !isLoggedIn && "opacity-50 cursor-not-allowed"
            )}
          >
            <Heart className={cn("w-4 h-4", isFollowing && "fill-current text-red-500")} />
            {isFollowing ? "Following" : "Follow"}
          </button>
        </div>
        {userInfo?.stream && (
          <div className="flex items-center gap-4 text-sm font-semibold">
            <div className="flex items-center gap-1.5 text-red-600">
              <Video className="w-4 h-4" />
              {formatViewers(userInfo.stream.viewersCount)}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
