import { PanelLeft } from "lucide-react";
import { cn, formatViewers } from "../lib/utils";
import type { UserInfo, TopStream, ActiveTab } from "../types";

interface SidebarProps {
  isOpen: boolean;
  setIsOpen: (open: boolean) => void;
  activeTab: ActiveTab;
  currentChannel: string | null;
  onSelectChannel: (login: string) => void;
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
  onSelectChannel,
  followedChannels,
  isLoadingFollowed,
  isLoggedIn,
  topStreams,
  isLoadingBrowse,
}: SidebarProps) {
  return (
    <aside
      className={cn(
        "bg-[#1f1f23] transition-all duration-300 flex flex-col border-r border-black shadow-xl z-40",
        isOpen ? "w-60" : "w-12"
      )}
    >
      <div className="p-3 flex items-center justify-between">
        {isOpen && (
          <span className="font-bold text-[13px] uppercase tracking-wide opacity-90">
            {activeTab === "following" ? "Followed" : "Top Streams"}
          </span>
        )}
        <button
          onClick={() => setIsOpen(!isOpen)}
          className="hover:bg-[#2f2f35] p-1 rounded-md transition-colors"
        >
          <PanelLeft className="w-4 h-4" />
        </button>
      </div>

      <div className="flex-1 overflow-y-auto">
        {activeTab === "following" ? (
          <FollowingList
            channels={followedChannels}
            isLoading={isLoadingFollowed}
            isLoggedIn={isLoggedIn}
            isSidebarOpen={isOpen}
            currentChannel={currentChannel}
            onSelectChannel={onSelectChannel}
          />
        ) : (
          <TopStreamsList
            streams={topStreams}
            isLoading={isLoadingBrowse}
            isSidebarOpen={isOpen}
            currentChannel={currentChannel}
            onSelectChannel={onSelectChannel}
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
  onSelectChannel: (login: string) => void;
}

function FollowingList({
  channels,
  isLoading,
  isLoggedIn,
  isSidebarOpen,
  currentChannel,
  onSelectChannel,
}: FollowingListProps) {
  if (isLoading) {
    return isSidebarOpen ? (
      <div className="p-4 text-center text-[#adadb8] text-sm">Loading...</div>
    ) : null;
  }

  if (channels.length === 0) {
    return isSidebarOpen ? (
      <div className="p-4 text-center text-[#adadb8] text-sm">
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
          isSidebarOpen={isSidebarOpen}
          onSelect={() => onSelectChannel(c.login)}
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
  onSelectChannel: (login: string) => void;
}

function TopStreamsList({
  streams,
  isLoading,
  isSidebarOpen,
  currentChannel,
  onSelectChannel,
}: TopStreamsListProps) {
  if (isLoading) {
    return isSidebarOpen ? (
      <div className="p-4 text-center text-[#adadb8] text-sm">Loading top streams...</div>
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
          isSidebarOpen={isSidebarOpen}
          onSelect={() => onSelectChannel(s.broadcaster.login)}
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
  isSidebarOpen: boolean;
  onSelect: () => void;
}

function ChannelItem({
  login,
  displayName,
  profileImageURL,
  gameName,
  viewersCount,
  isActive,
  isSidebarOpen,
  onSelect,
}: ChannelItemProps) {
  return (
    <button
      onClick={onSelect}
      className={cn(
        "w-full flex items-center p-2 hover:bg-[#2f2f35] transition-colors group relative",
        isActive && "bg-[#2f2f35]"
      )}
    >
      <div className="w-8 h-8 bg-[#3f3f46] rounded-full flex-shrink-0 overflow-hidden border border-white/5">
        {profileImageURL ? (
          <img src={profileImageURL} alt={login} />
        ) : (
          <div className="w-full h-full bg-[#3f3f46]" />
        )}
      </div>
      {isSidebarOpen && (
        <div className="ml-3 flex-1 flex flex-col items-start overflow-hidden text-left">
          <span className="font-semibold text-[13px] truncate w-full">{displayName}</span>
          <span className="text-[11px] text-[#adadb8] truncate w-full italic">
            {gameName || "Streaming"}
          </span>
        </div>
      )}
      {isSidebarOpen && viewersCount !== undefined && (
        <div className="flex items-center gap-1">
          <div className="w-2 h-2 bg-red-600 rounded-full" />
          <span className="text-[11px] text-[#adadb8] font-medium">
            {formatViewers(viewersCount)}
          </span>
        </div>
      )}
      {!isSidebarOpen && (
        <div className="absolute left-14 bg-black text-white px-2 py-1 rounded text-xs opacity-0 group-hover:opacity-100 transition-opacity z-50 whitespace-nowrap shadow-lg border border-white/10 pointer-events-none">
          {displayName} {viewersCount !== undefined && `• ${formatViewers(viewersCount)}`}
        </div>
      )}
    </button>
  );
}
