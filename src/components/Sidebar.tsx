import { useRef, useState } from "react";
import { PanelLeft } from "lucide-react";
import { cn, formatViewers } from "../lib/utils";
import { useCollaborations } from "../hooks";
import { ChannelActionButtons } from "./ChannelActionButtons";
import type { UserInfo, TopStream, ActiveTab, Collaboration, CollaborationMap } from "../types";

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
  const channelIds =
    activeTab === "following" ? followedChannels.map((c) => c.id) : topStreams.map((s) => s.broadcaster.id);
  const collabs = useCollaborations(channelIds);

  return (
    <aside
      className={cn(
        "bg-surface transition-all duration-300 flex flex-col border-r border-border z-40",
        isOpen ? "w-60" : "w-12"
      )}
    >
      <div
        className={cn(
          "p-3 flex items-center",
          isOpen ? "justify-between" : "justify-center"
        )}
      >
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

      <div
        className={cn(
          "flex-1 overflow-y-auto overflow-x-hidden",
          isOpen ? "scrollbar-overlay" : "scrollbar-none"
        )}
      >
        {activeTab === "following" ? (
          <FollowingList
            channels={followedChannels}
            collabs={collabs}
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
            collabs={collabs}
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
  collabs: CollaborationMap;
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
  collabs,
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
          collab={collabs[c.id]}
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
  collabs: CollaborationMap;
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
  collabs,
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
          collab={collabs[s.broadcaster.id]}
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
  collab?: Collaboration;
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
  collab,
  isActive,
  isInGrid,
  canAddToGrid,
  isSidebarOpen,
  onSelect,
  onAddToGrid,
  onOpenPopout,
}: ChannelItemProps) {
  const buttonRef = useRef<HTMLButtonElement>(null);
  const guest = collab?.guests[0];
  const guestNames = collab?.guests.map((g) => g.displayName).join(", ");
  const [tooltipPos, setTooltipPos] = useState<{ top: number; left: number } | null>(null);

  const handleClick = (e: React.MouseEvent) => {
    if (e.shiftKey) {
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
        className="group/item w-full flex items-center p-2 hover:bg-hover transition-colors"
        title={
          isSidebarOpen
            ? [guestNames && `Live with ${guestNames}`, "Click to play • Shift+click to add to grid"]
                .filter(Boolean)
                .join("\n")
            : undefined
        }
      >
        <div className="relative w-8 h-8 flex-shrink-0">
          <div className="w-full h-full bg-elevated rounded-full overflow-hidden border border-border">
            {profileImageURL ? (
              <img src={profileImageURL} alt={login} />
            ) : (
              <div className="w-full h-full bg-elevated" />
            )}
          </div>
          {guest && (
            // The ring takes the row colour, so it reads as a gap cut into the main avatar.
            <img
              src={guest.profileImageURL}
              alt=""
              className={cn(
                "absolute -right-px -bottom-px w-4 h-4 rounded-full bg-elevated ring-2 group-hover/item:ring-hover",
                isActive ? "ring-elevated" : "ring-surface"
              )}
            />
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
          {guestNames && <div className="text-muted">with {guestNames}</div>}
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
