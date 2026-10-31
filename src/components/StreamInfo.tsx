import { useEffect, useState } from "react";
import { formatViewers, useI18n } from "../i18n";
import { Clock, Heart, Video } from "lucide-react";
import { cn, formatUptime } from "../lib/utils";
import type { UserInfo } from "../types";

interface StreamInfoProps {
  channel: string;
  userInfo: UserInfo | null;
  isFollowing: boolean;
  isLoggedIn: boolean;
  onFollow: () => void;
  onSelectChannel: (login: string) => void;
  onAddToGrid: (login: string) => void;
}

export function StreamInfo({ channel, userInfo, isFollowing, isLoggedIn, onFollow, onSelectChannel, onAddToGrid }: StreamInfoProps) {
  const { t, language } = useI18n();
  const guests = userInfo?.collaboration?.guests ?? [];

  return (
    <div className="bg-surface p-4 flex items-start gap-4 border-t border-border">
      <div className="w-16 h-16 bg-elevated rounded-full flex-shrink-0 overflow-hidden border-2 border-twitch shadow-lg shadow-twitch/10">
        {userInfo?.profileImageURL && <img src={userInfo.profileImageURL} alt="pfp" />}
      </div>
      <div className="flex-1 min-w-0">
        <h1 className="text-lg font-bold truncate leading-tight tracking-tight">
          {userInfo?.stream?.title || `${channel} - Streaming`}
        </h1>
        <div className="flex flex-wrap items-center gap-x-2 gap-y-1 mt-1 text-sm">
          <span className="text-twitch font-bold hover:underline cursor-pointer">
            {userInfo?.displayName || channel}
          </span>
          {guests.length > 0 && (
            <>
              <span className="text-muted">{t("collab.with")}</span>
              {guests.map((g) => (
                <button
                  key={g.id}
                  onClick={(e) => (e.shiftKey ? onAddToGrid(g.login) : onSelectChannel(g.login))}
                  className="flex items-center gap-1 text-twitch font-bold hover:underline cursor-pointer"
                  title={t("collab.watch", { name: g.displayName })}
                >
                  <img src={g.profileImageURL} alt="" className="w-4 h-4 rounded-full" />
                  {g.displayName}
                </button>
              ))}
            </>
          )}
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
            {isFollowing ? t("info.following") : t("info.follow")}
          </button>
        </div>
        {userInfo?.stream && (
          <div className="flex items-center gap-4 text-sm font-semibold">
            <div className="flex items-center gap-1.5 text-red-600">
              <Video className="w-4 h-4" />
              {formatViewers(userInfo.stream.viewersCount, language)}
            </div>
            {userInfo.stream.createdAt && <Uptime startedAt={userInfo.stream.createdAt} />}
          </div>
        )}
      </div>
    </div>
  );
}

function Uptime({ startedAt }: { startedAt: string }) {
  const { t } = useI18n();
  const start = Date.parse(startedAt);
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (Number.isNaN(start)) return;
    const phase = (((now - start) % 1000) + 1000) % 1000;
    const id = setTimeout(() => setNow(Date.now()), 1000 - phase + 5);
    return () => clearTimeout(id);
  }, [now, start]);
  if (Number.isNaN(start)) return null;
  return (
    <div className="flex items-center gap-1.5 text-muted tabular-nums" title={t("info.uptime")}>
      <Clock className="w-3 h-3" />
      {formatUptime(start, now)}
    </div>
  );
}
