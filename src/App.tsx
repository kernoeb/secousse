import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { info, error as logError, attachConsole } from "@tauri-apps/plugin-log";

import { useAuth, useChat, useEmotes, useSearch, useTopStreams } from "./hooks";
import { Navbar, Sidebar, VideoPlayer, Chat, StreamInfo, BrowseGrid } from "./components";
import { getInitialChannel, getInitialActiveTab, persistChannel, persistActiveTab } from "./lib/utils";
import type { UserInfo, ActiveTab, GetUserInfoResponse } from "./types";

export default function App() {
  // Channel state
  const [channel, setChannelInternal] = useState<string | null>(getInitialChannel);
  const [userInfo, setUserInfo] = useState<UserInfo | null>(null);
  const [isLoadingStream, setIsLoadingStream] = useState(false);
  const [isFollowing, setIsFollowing] = useState(false);
  
  // UI state
  const [activeTab, setActiveTabInternal] = useState<ActiveTab>(getInitialActiveTab);
  const [isSidebarOpen, setIsSidebarOpen] = useState(true);
  const [isChatOpen, setIsChatOpen] = useState(true);
  const [isFullscreen, setIsFullscreen] = useState(false);

  // Custom hooks
  const { isLoggedIn, selfInfo, followedChannels, isLoadingFollowed, login, logout, refreshFollowedChannels } = useAuth();
  const { allEmotes, globalBadges, channelBadges, loadChannelEmotes } = useEmotes();
  const { topStreams, isLoading: isLoadingBrowse, loadTopStreams } = useTopStreams();
  const chat = useChat(channel, isLoggedIn);
  
  // Wrapper to persist channel
  const setChannel = useCallback((newChannel: string | null) => {
    persistChannel(newChannel);
    setChannelInternal(newChannel);
  }, []);

  // Wrapper to persist active tab
  const setActiveTab = useCallback((tab: ActiveTab) => {
    persistActiveTab(tab);
    setActiveTabInternal(tab);
  }, []);

  // Search hook with channel selection callback
  const search = useSearch(setChannel);

  // Initialize app
  useEffect(() => {
    attachConsole();
    info("[App] Initializing...");
    loadTopStreams();

    // ESC key to exit video fullscreen
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setIsFullscreen(false);
      }
    };
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [loadTopStreams]);

  // Auto-refresh sidebar data every 60 seconds
  useEffect(() => {
    const REFRESH_INTERVAL = 60 * 1000;

    const refreshData = () => {
      if (activeTab === "following" && isLoggedIn && selfInfo?.id) {
        info("[App] Auto-refreshing followed channels...");
        refreshFollowedChannels(true);
      } else if (activeTab === "browse") {
        info("[App] Auto-refreshing top streams...");
        loadTopStreams(true);
      }
    };

    const intervalId = setInterval(refreshData, REFRESH_INTERVAL);
    return () => clearInterval(intervalId);
  }, [activeTab, isLoggedIn, selfInfo?.id, refreshFollowedChannels, loadTopStreams]);

  // Auto-refresh current stream info every 60 seconds
  useEffect(() => {
    if (!channel) return;

    const REFRESH_INTERVAL = 60 * 1000;

    const refreshStreamInfo = async () => {
      try {
        info(`[App] Auto-refreshing stream info for: ${channel}`);
        const data = await invoke<GetUserInfoResponse>("get_user_info", { login: channel });
        setUserInfo(data.user);

        if (isLoggedIn && selfInfo && data.user.stream) {
          await invoke("update_watch_state", {
            channelLogin: data.user.login,
            channelId: data.user.id,
            streamId: data.user.stream.id,
            userId: selfInfo.id,
          });
        }
      } catch (err) {
        logError(`[App] Failed to refresh stream info: ${err}`);
      }
    };

    const intervalId = setInterval(refreshStreamInfo, REFRESH_INTERVAL);
    return () => clearInterval(intervalId);
  }, [channel, isLoggedIn, selfInfo]);

  // Load channel data when channel changes
  useEffect(() => {
    if (!channel) {
      setUserInfo(null);
      return;
    }

    async function loadChannelData() {
      try {
        info(`[App] Loading data for channel: ${channel}`);
        setIsLoadingStream(true);
        
        const data = await invoke<GetUserInfoResponse>("get_user_info", { login: channel });
        setUserInfo(data.user);

        if (!data.user?.stream) {
          info(`[App] Channel ${channel} is offline`);
          setIsLoadingStream(false);
          return;
        }

        // Load emotes for this channel
        await loadChannelEmotes(data.user.id);

        // Update watch state
        if (isLoggedIn && selfInfo && data.user.stream) {
          await invoke("update_watch_state", {
            channelLogin: data.user.login,
            channelId: data.user.id,
            streamId: data.user.stream.id,
            userId: selfInfo.id,
          });
        }
      } catch (err) {
        logError(`[App] Failed to load channel data: ${err}`);
        setIsLoadingStream(false);
      }
    }

    loadChannelData();
  }, [channel, isLoggedIn, selfInfo, loadChannelEmotes]);

  // Check follow status when channel changes
  useEffect(() => {
    if (isLoggedIn && userInfo) {
      const isInFollowedList = followedChannels.some(c => c.id === userInfo.id);
      setIsFollowing(isInFollowedList);
    } else {
      setIsFollowing(false);
    }
  }, [userInfo, followedChannels, isLoggedIn]);

  // Follow/unfollow handler
  const handleFollow = useCallback(async () => {
    if (!isLoggedIn || !userInfo || !selfInfo?.id) return;

    try {
      if (isFollowing) {
        info(`[App] Unfollowing ${userInfo.displayName}...`);
        await invoke("unfollow_channel", { fromUserId: selfInfo.id, toUserId: userInfo.id });
        setIsFollowing(false);
        info(`[App] Unfollowed ${userInfo.displayName}`);
      } else {
        info(`[App] Following ${userInfo.displayName}...`);
        await invoke("follow_channel", { fromUserId: selfInfo.id, toUserId: userInfo.id });
        setIsFollowing(true);
        info(`[App] Followed ${userInfo.displayName}`);
      }
      refreshFollowedChannels();
    } catch (err) {
      logError(`[App] Follow/unfollow error: ${err}`);
    }
  }, [isLoggedIn, userInfo, selfInfo?.id, isFollowing, refreshFollowedChannels]);

  return (
    <div className="flex flex-col h-screen w-full bg-[#0e0e10] text-[#efeff1]">
      <Navbar
        activeTab={activeTab}
        setActiveTab={setActiveTab}
        isLoggedIn={isLoggedIn}
        selfInfo={selfInfo}
        onLogin={login}
        onLogout={logout}
        searchQuery={search.query}
        setSearchQuery={search.setQuery}
        searchResults={search.results}
        showSearchResults={search.showResults}
        setShowSearchResults={search.setShowResults}
        onSelectSearchResult={search.selectResult}
        onClearSearch={search.clearSearch}
        onSearch={() => {}}
        onOpenSidebar={() => setIsSidebarOpen(true)}
        onLoadTopStreams={() => loadTopStreams()}
        hasTopStreams={topStreams.length > 0}
        onGoHome={() => {
          setChannel(null);
          setActiveTab("browse");
          if (topStreams.length === 0) {
            loadTopStreams();
          }
        }}
      />

      <div className="flex flex-1 overflow-hidden">
        <Sidebar
          isOpen={isSidebarOpen}
          setIsOpen={setIsSidebarOpen}
          activeTab={activeTab}
          currentChannel={channel}
          onSelectChannel={setChannel}
          followedChannels={followedChannels}
          isLoadingFollowed={isLoadingFollowed}
          isLoggedIn={isLoggedIn}
          topStreams={topStreams}
          isLoadingBrowse={isLoadingBrowse}
        />

        <main className="flex-1 bg-[#0e0e10] flex flex-col relative overflow-hidden">
          {channel ? (
            <>
              <VideoPlayer
                channel={channel}
                userInfo={userInfo}
                isLoadingStream={isLoadingStream}
                setIsLoadingStream={setIsLoadingStream}
                isFullscreen={isFullscreen}
                setIsFullscreen={setIsFullscreen}
              />
              <StreamInfo
                channel={channel}
                userInfo={userInfo}
                isFollowing={isFollowing}
                isLoggedIn={isLoggedIn}
                onFollow={handleFollow}
              />
            </>
          ) : (
            <BrowseGrid
              streams={topStreams}
              isLoading={isLoadingBrowse}
              isLoggedIn={isLoggedIn}
              onSelectChannel={setChannel}
              onRetry={() => loadTopStreams()}
            />
          )}
        </main>

        <Chat
          isOpen={isChatOpen}
          setIsOpen={setIsChatOpen}
          messages={chat.messages}
          emotes={allEmotes}
          globalBadges={globalBadges}
          channelBadges={channelBadges}
          isLoggedIn={isLoggedIn}
          isAtBottom={chat.isAtBottom}
          chatContainerRef={chat.chatContainerRef}
          chatEndRef={chat.chatEndRef}
          onScroll={chat.handleScroll}
          onScrollToBottom={chat.scrollToBottom}
          onSendMessage={chat.sendMessage}
        />
      </div>
    </div>
  );
}
