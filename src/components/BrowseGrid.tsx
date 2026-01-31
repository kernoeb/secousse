import { formatViewers } from "../lib/utils";
import type { TopStream } from "../types";

interface BrowseGridProps {
  streams: TopStream[];
  isLoading: boolean;
  isLoggedIn: boolean;
  onSelectChannel: (login: string) => void;
  onRetry: () => void;
}

export function BrowseGrid({ streams, isLoading, isLoggedIn, onSelectChannel, onRetry }: BrowseGridProps) {
  return (
    <div className="flex-1 overflow-y-auto p-6">
      <h1 className="text-2xl font-bold mb-6">
        {isLoggedIn ? "Live channels" : "Top Live Streams"}
      </h1>

      {isLoading ? (
        <div className="flex items-center justify-center h-64">
          <div className="text-[#adadb8]">Loading streams...</div>
        </div>
      ) : streams.length === 0 ? (
        <div className="flex items-center justify-center h-64">
          <div className="text-[#adadb8]">
            No streams found.{" "}
            <button onClick={onRetry} className="text-[#9146ff] hover:underline">
              Retry
            </button>
          </div>
        </div>
      ) : (
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
          {streams.map((stream) => (
            <StreamCard key={stream.id} stream={stream} onSelect={() => onSelectChannel(stream.broadcaster.login)} />
          ))}
        </div>
      )}
    </div>
  );
}

interface StreamCardProps {
  stream: TopStream;
  onSelect: () => void;
}

function StreamCard({ stream, onSelect }: StreamCardProps) {
  return (
    <button
      onClick={onSelect}
      className="bg-[#1f1f23] rounded-lg overflow-hidden hover:bg-[#2f2f35] transition-colors text-left group"
    >
      {/* Stream Preview */}
      <div className="relative aspect-video bg-[#18181b]">
        {stream.previewImageURL && (
          <img src={stream.previewImageURL} alt={stream.title} className="w-full h-full object-cover" />
        )}
        {/* Live badge and viewers */}
        <div className="absolute bottom-2 left-2 flex items-center gap-2">
          <div className="bg-red-600 text-white text-xs font-bold px-1.5 py-0.5 rounded">LIVE</div>
          <div className="bg-black/70 text-white text-xs px-1.5 py-0.5 rounded">
            {formatViewers(stream.viewersCount)} viewers
          </div>
        </div>
      </div>

      {/* Stream Info */}
      <div className="p-3 flex gap-3">
        <div className="w-10 h-10 rounded-full overflow-hidden flex-shrink-0 bg-[#3f3f46]">
          {stream.broadcaster.profileImageURL && (
            <img
              src={stream.broadcaster.profileImageURL}
              alt={stream.broadcaster.displayName}
              className="w-full h-full object-cover"
            />
          )}
        </div>
        <div className="flex-1 min-w-0">
          <h3 className="font-semibold text-sm truncate group-hover:text-[#9146ff] transition-colors">
            {stream.title}
          </h3>
          <p className="text-[#adadb8] text-sm truncate">{stream.broadcaster.displayName}</p>
          <p className="text-[#adadb8] text-xs truncate">{stream.game?.displayName || "Streaming"}</p>
        </div>
      </div>
    </button>
  );
}
