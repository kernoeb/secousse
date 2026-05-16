import { useRef, useState } from "react";
import { PanelLeft } from "lucide-react";
import { cn, formatViewers } from "../lib/utils";
import { ChannelActionButtons } from "./ChannelActionButtons";
import type { UserInfo, TopStream, ActiveTab } from "../types";

interface SidebarProps {
  isOpen: boolean;
  setIsOpen: (open: boolean) => void;
  activeTab: ActiveTab;
  currentChannel: string | null;
  gridChannels: string[];
  canAddToGrid: boolean;
  onSelectChannel: (login: string) => void;
  onAddToGrid: (login: string) => void;
  onOpenPopout: (login: string) => void;
  // Following tab
  followedChannels: UserInfo[];
  isLoadingFollowed: boolean;
  isLoggedIn: boolean;
  // Browse tab
  topStreams: TopStream[];
  isLoadingBrowse: boolean;
}

export function Sidebar({
  isOpen,
  setIsOpen,
  activeTab,
  currentChannel,
  gridChannels,
  canAddToGrid,
  onSelectChannel,
  onAddToGrid,
  onOpenPopout,
  followedChannels,
  isLoadingFollowed,
  isLoggedIn,
  topStreams,
  isLoadingBrowse,
}: SidebarProps) {
  return (
    <aside
      className={cn(
        "bg-surface transition-all duration-300 flex flex-col border-r border-border z-40",
        isOpen ? "w-60" : "w-12"
      )}
    >
      <div className="p-3 flex items-center justify-between">
        {isOpen && (
          <span className="font-bold text-[13px] uppercase tracking-wide text-muted">
            {activeTab === "following" ? "Followed" : "Top Streams"}
          </span>
        )}
        <button
          onClick={() => setIsOpen(!isOpen)}
          className="hover:bg-hover p-1 rounded-md transition-colors"
        >
          <PanelLeft className="w-4 h-4" />
        </button>
      </div>

      <div className="flex-1 overflow-y-auto overflow-x-hidden scrollbar-thin">
        {activeTab === "following" ? (
          <FollowingList
            channels={followedChannels}
            isLoading={isLoadingFollowed}
            isLoggedIn={isLoggedIn}
            isSidebarOpen={isOpen}
            currentChannel={currentChannel}
            gridChannels={gridChannels}
            canAddToGrid={canAddToGrid}
            onSelectChannel={onSelectChannel}
            onAddToGrid={onAddToGrid}
            onOpenPopout={onOpenPopout}
          />
        ) : (
          <TopStreamsList
            streams={topStreams}
            isLoading={isLoadingBrowse}
            isSidebarOpen={isOpen}
            currentChannel={currentChannel}
            gridChannels={gridChannels}
            canAddToGrid={canAddToGrid}
            onSelectChannel={onSelectChannel}
            onAddToGrid={onAddToGrid}
            onOpenPopout={onOpenPopout}
          />
        )}
      </div>
    </aside>
  );
}

interface FollowingListProps {
  channels: UserInfo[];
  isLoading: boolean;
  isLoggedIn: boolean;
  isSidebarOpen: boolean;
  currentChannel: string | null;
  gridChannels: string[];
  canAddToGrid: boolean;
  onSelectChannel: (login: string) => void;
  onAddToGrid: (login: string) => void;
  onOpenPopout: (login: string) => void;
}

function FollowingList({
  channels,
  isLoading,
  isLoggedIn,
  isSidebarOpen,
  currentChannel,
  gridChannels,
  canAddToGrid,
  onSelectChannel,
  onAddToGrid,
  onOpenPopout,
}: FollowingListProps) {
  if (isLoading) {
    return isSidebarOpen ? (
      <div className="p-4 text-center text-muted text-sm">Loading...</div>
    ) : null;
  }

  if (channels.length === 0) {
    return isSidebarOpen ? (
      <div className="p-4 text-center text-muted text-sm">
        {isLoggedIn ? "No live followed channels" : "Log in to see followed channels"}
      </div>
    ) : null;
  }

  return (
    <>
      {channels.map((c) => (
        <ChannelItem
          key={c.login}
          login={c.login}
          displayName={c.displayName}
          profileImageURL={c.profileImageURL}
          gameName={c.stream?.game?.name}
          viewersCount={c.stream?.viewersCount}
          isActive={currentChannel === c.login}
          isInGrid={gridChannels.includes(c.login)}
          canAddToGrid={canAddToGrid}
          isSidebarOpen={isSidebarOpen}
          onSelect={() => onSelectChannel(c.login)}
          onAddToGrid={() => onAddToGrid(c.login)}
          onOpenPopout={() => onOpenPopout(c.login)}
        />
      ))}
    </>
  );
}

interface TopStreamsListProps {
  streams: TopStream[];
  isLoading: boolean;
  isSidebarOpen: boolean;
  currentChannel: string | null;
  gridChannels: string[];
  canAddToGrid: boolean;
  onSelectChannel: (login: string) => void;
  onAddToGrid: (login: string) => void;
  onOpenPopout: (login: string) => void;
}

function TopStreamsList({
  streams,
  isLoading,
  isSidebarOpen,
  currentChannel,
  gridChannels,
  canAddToGrid,
  onSelectChannel,
  onAddToGrid,
  onOpenPopout,
}: TopStreamsListProps) {
  if (isLoading) {
    return isSidebarOpen ? (
      <div className="p-4 text-center text-muted text-sm">Loading top streams...</div>
    ) : null;
  }

  return (
    <>
      {streams.map((s) => (
        <ChannelItem
          key={s.id}
          login={s.broadcaster.login}
          displayName={s.broadcaster.displayName}
          profileImageURL={s.broadcaster.profileImageURL}
          gameName={s.game?.name}
          viewersCount={s.viewersCount}
          isActive={currentChannel === s.broadcaster.login}
          isInGrid={gridChannels.includes(s.broadcaster.login)}
          canAddToGrid={canAddToGrid}
          isSidebarOpen={isSidebarOpen}
          onSelect={() => onSelectChannel(s.broadcaster.login)}
          onAddToGrid={() => onAddToGrid(s.broadcaster.login)}
          onOpenPopout={() => onOpenPopout(s.broadcaster.login)}
        />
      ))}
    </>
  );
}

interface ChannelItemProps {
  login: string;
  displayName: string;
  profileImageURL: string;
  gameName?: string;
  viewersCount?: number;
  isActive: boolean;
  isInGrid: boolean;
  canAddToGrid: boolean;
  isSidebarOpen: boolean;
  onSelect: () => void;
  onAddToGrid: () => void;
  onOpenPopout: () => void;
}

function ChannelItem({
  login,
  displayName,
  profileImageURL,
  gameName,
  viewersCount,
  isActive,
  isInGrid,
  canAddToGrid,
  isSidebarOpen,
  onSelect,
  onAddToGrid,
  onOpenPopout,
}: ChannelItemProps) {
  const buttonRef = useRef<HTMLButtonElement>(null);
  const [tooltipPos, setTooltipPos] = useState<{ top: number; left: number } | null>(null);

  const handleClick = (e: React.MouseEvent) => {
    if (e.shiftKey && !isInGrid) {
      e.preventDefault();
      onAddToGrid();
    } else {
      onSelect();
    }
  };

  const handleMouseEnter = () => {
    if (isSidebarOpen || !buttonRef.current) return;
    const r = buttonRef.current.getBoundingClientRect();
    setTooltipPos({ top: r.top + r.height / 2, left: r.right + 8 });
  };

  return (
    <div className={cn("group/row relative", isActive && "bg-elevated")}>
      <button
        ref={buttonRef}
        onClick={handleClick}
        onMouseEnter={handleMouseEnter}
        onMouseLeave={() => setTooltipPos(null)}
        className="w-full flex items-center p-2 hover:bg-hover transition-colors"
        title={isSidebarOpen ? "Click to play • Shift+click to add to grid" : undefined}
      >
        <div className="w-8 h-8 bg-elevated rounded-full flex-shrink-0 overflow-hidden border border-border">
          {profileImageURL ? (
            <img src={profileImageURL} alt={login} />
          ) : (
            <div className="w-full h-full bg-elevated" />
          )}
        </div>
        {isSidebarOpen && (
          <div className="ml-3 flex-1 min-w-0 flex flex-col items-start overflow-hidden text-left">
            <span className="font-semibold text-[13px] truncate w-full">{displayName}</span>
            <span className="text-[11px] text-muted truncate w-full italic">
              {gameName || "Streaming"}
            </span>
          </div>
        )}
        {isSidebarOpen && viewersCount !== undefined && (
          <div className="flex items-center gap-1 flex-shrink-0 ml-2">
            <div className="w-2 h-2 bg-red-600 rounded-full" />
            <span className="text-[11px] text-muted font-medium">
              {formatViewers(viewersCount)}
            </span>
          </div>
        )}
      </button>
      {tooltipPos && !isSidebarOpen && (
        <div
          style={{ position: "fixed", top: tooltipPos.top, left: tooltipPos.left }}
          className="-translate-y-1/2 bg-elevated text-white px-2 py-1 rounded text-xs z-50 whitespace-nowrap shadow-lg border border-border pointer-events-none"
        >
          {displayName}
          {viewersCount !== undefined && ` • ${formatViewers(viewersCount)}`}
        </div>
      )}

      {isSidebarOpen && (
        <ChannelActionButtons
          variant="row"
          canAddToGrid={canAddToGrid}
          isInGrid={isInGrid}
          onAddToGrid={onAddToGrid}
          onOpenPopout={onOpenPopout}
          className="absolute top-1/2 right-1 -translate-y-1/2 opacity-0 group-hover/row:opacity-100 transition-opacity"
        />
      )}
    </div>
  );
}
