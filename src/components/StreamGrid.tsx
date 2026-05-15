import { useUserInfo } from "../hooks";
import { VideoPlayer } from "./VideoPlayer";
import { ChannelActionButtons } from "./ChannelActionButtons";
import { cn } from "../lib/utils";

interface StreamGridProps {
  channels: string[];
  focusedIndex: number;
  onFocusChange: (index: number) => void;
  onRemoveTile: (index: number) => void;
  onOpenPopout: (channel: string) => void;
  isFullscreen: boolean;
  setIsFullscreen: (v: boolean) => void;
}

const LAYOUTS: Record<number, string> = {
  1: "grid-cols-1 grid-rows-1",
  2: "grid-cols-2 grid-rows-1",
  3: "grid-cols-3 grid-rows-1",
  4: "grid-cols-2 grid-rows-2",
};

export function StreamGrid({
  channels,
  focusedIndex,
  onFocusChange,
  onRemoveTile,
  onOpenPopout,
  isFullscreen,
  setIsFullscreen,
}: StreamGridProps) {
  const isMulti = channels.length > 1;
  const layout = LAYOUTS[channels.length] ?? LAYOUTS[4];

  return (
    <div className={cn("flex-1 grid gap-1 bg-base overflow-hidden", layout)}>
      {channels.map((channel, idx) => {
        const isFocused = idx === focusedIndex;
        return (
          <StreamGridTile
            key={channel}
            channel={channel}
            isFocused={isFocused}
            isMulti={isMulti}
            onRequestFocus={() => onFocusChange(idx)}
            onRemove={isMulti ? () => onRemoveTile(idx) : undefined}
            onOpenPopout={() => onOpenPopout(channel)}
            isFullscreen={isFocused ? isFullscreen : false}
            setIsFullscreen={isFocused ? setIsFullscreen : NOOP}
          />
        );
      })}
    </div>
  );
}

const NOOP = () => {};

interface StreamGridTileProps {
  channel: string;
  isFocused: boolean;
  isMulti: boolean;
  onRequestFocus: () => void;
  onRemove?: () => void;
  onOpenPopout: () => void;
  isFullscreen: boolean;
  setIsFullscreen: (v: boolean) => void;
}

function StreamGridTile({
  channel,
  isFocused,
  isMulti,
  onRequestFocus,
  onRemove,
  onOpenPopout,
  isFullscreen,
  setIsFullscreen,
}: StreamGridTileProps) {
  const { userInfo } = useUserInfo(channel);

  return (
    <div
      className={cn(
        "group relative overflow-hidden bg-black flex flex-col",
        isMulti && isFocused && "ring-2 ring-twitch ring-inset"
      )}
    >
      <VideoPlayer
        channel={channel}
        userInfo={userInfo}
        isFullscreen={isFullscreen}
        setIsFullscreen={setIsFullscreen}
        forceMuted={isMulti && !isFocused}
        compact={isMulti}
        onRequestFocus={onRequestFocus}
      />

      {isMulti && (
        <div className="absolute top-2 right-2 flex items-center gap-1 z-10 opacity-0 group-hover:opacity-100 transition-opacity">
          <div className="bg-black/70 text-white text-xs px-2 py-1 rounded pointer-events-none">
            {userInfo?.displayName || channel}
          </div>
          <ChannelActionButtons
            variant="overlay"
            canAddToGrid={false}
            onOpenPopout={onOpenPopout}
            onRemove={onRemove}
          />
        </div>
      )}
    </div>
  );
}
