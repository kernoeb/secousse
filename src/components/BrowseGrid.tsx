import { ChannelActionButtons } from "./ChannelActionButtons";
import { useI18n } from "../i18n";
import type { TopStream } from "../types";

interface BrowseGridProps {
  streams: TopStream[];
  isLoading: boolean;
  isLoggedIn: boolean;
  canAddToGrid: boolean;
  onSelectChannel: (login: string) => void;
  onAddToGrid: (login: string) => void;
  onOpenPopout: (login: string) => void;
  onRetry: () => void;
}

export function BrowseGrid({
  streams,
  isLoading,
  isLoggedIn,
  canAddToGrid,
  onSelectChannel,
  onAddToGrid,
  onOpenPopout,
  onRetry,
}: BrowseGridProps) {
  const { t } = useI18n();
  return (
    <div className="flex-1 overflow-y-auto overflow-x-hidden scrollbar-thin p-6">
      <h1 className="text-2xl font-bold mb-6">
        {isLoggedIn ? t("browse.liveChannels") : t("browse.topLive")}
      </h1>

      {isLoading ? (
        <div className="flex items-center justify-center h-64">
          <div className="text-muted">{t("browse.loading")}</div>
        </div>
      ) : streams.length === 0 ? (
        <div className="flex items-center justify-center h-64">
          <div className="text-muted">
            {t("browse.empty")}{" "}
            <button onClick={onRetry} className="text-twitch hover:underline">
              {t("browse.retry")}
            </button>
          </div>
        </div>
      ) : (
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
          {streams.map((stream) => (
            <StreamCard
              key={stream.id}
              stream={stream}
              canAddToGrid={canAddToGrid}
              onSelect={() => onSelectChannel(stream.broadcaster.login)}
              onAddToGrid={() => onAddToGrid(stream.broadcaster.login)}
              onOpenPopout={() => onOpenPopout(stream.broadcaster.login)}
            />
          ))}
        </div>
      )}
    </div>
  );
}

interface StreamCardProps {
  stream: TopStream;
  canAddToGrid: boolean;
  onSelect: () => void;
  onAddToGrid: () => void;
  onOpenPopout: () => void;
}

function StreamCard({ stream, canAddToGrid, onSelect, onAddToGrid, onOpenPopout }: StreamCardProps) {
  const { t, viewers } = useI18n();
  const handleClick = (e: React.MouseEvent) => {
    if (e.shiftKey) {
      e.preventDefault();
      onAddToGrid();
    } else {
      onSelect();
    }
  };

  return (
    <div className="relative group bg-surface rounded-lg overflow-hidden hover:bg-surface-alt transition-colors">
      <button
        onClick={handleClick}
        className="w-full text-left"
        title={canAddToGrid ? t("hint.playOrAdd") : t("hint.playOrPopout")}
      >
        <div className="relative aspect-video bg-base">
          {stream.previewImageURL && (
            <img src={stream.previewImageURL} alt={stream.title} className="w-full h-full object-cover" />
          )}
          <div className="absolute bottom-2 left-2 flex items-center gap-2">
            <div className="bg-red-600 text-white text-xs font-bold px-1.5 py-0.5 rounded">{t("player.live")}</div>
            <div className="bg-black/70 text-white text-xs px-1.5 py-0.5 rounded">
              {viewers(stream.viewersCount)}
            </div>
          </div>
        </div>

        <div className="p-3 flex gap-3">
          <div className="w-10 h-10 rounded-full overflow-hidden flex-shrink-0 bg-elevated">
            {stream.broadcaster.profileImageURL && (
              <img
                src={stream.broadcaster.profileImageURL}
                alt={stream.broadcaster.displayName}
                className="w-full h-full object-cover"
              />
            )}
          </div>
          <div className="flex-1 min-w-0">
            <h3 className="font-semibold text-sm truncate group-hover:text-twitch transition-colors">
              {stream.title}
            </h3>
            <p className="text-muted text-sm truncate">{stream.broadcaster.displayName}</p>
            <p className="text-muted text-xs truncate">{stream.game?.displayName || t("channel.streaming")}</p>
          </div>
        </div>
      </button>

      <ChannelActionButtons
        variant="card"
        canAddToGrid={canAddToGrid}
        onAddToGrid={onAddToGrid}
        onOpenPopout={onOpenPopout}
        className="absolute top-2 right-2 opacity-0 group-hover:opacity-100 transition-opacity"
      />
    </div>
  );
}
