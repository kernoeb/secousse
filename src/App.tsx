import { useState, useEffect, useCallback, useRef, useDeferredValue } from "react";
import { invoke } from "@tauri-apps/api/core";
import { info, error as logError, attachConsole } from "@tauri-apps/plugin-log";

import { useAuth, useChat, useEmotes, useIdleTimer, useSearch, useTopStreams, useUpdater, useUserInfo, useWindowFullscreen } from "./hooks";
import { Navbar, Sidebar, StreamGrid, Chat, StreamInfo, BrowseGrid } from "./components";
import {
  getInitialActiveTab,
  persistActiveTab,
  getInitialSidebarOpen,
  getInitialChatOpen,
  persistSidebarOpen,
  persistChatOpen,
  persistGridChannels,
  getInitialFocusedIndex,
  persistFocusedIndex,
  persistChannel,
  getInitialGridOrLegacyChannel,
  GRID_MAX_TILES,
} from "./lib/utils";
import { startSpam, stopSpam } from "./lib/spamSim";
import type { ActiveTab } from "./types";

export default function App() {
  const [channels, setChannelsInternal] = useState<string[]>(getInitialGridOrLegacyChannel);
  const [focusedIndex, setFocusedIndexInternal] = useState<number>(() => {
    const idx = getInitialFocusedIndex();
    return idx >= channels.length ? 0 : idx;
  });

  const focusedChannel = channels[focusedIndex] ?? null;
  // Heavy consumers (chat list, stream info fetch, sidebar highlight) read this
  // deferred copy. The StreamGrid keeps the urgent focusedIndex so the ring +
  // audio swap stays immediate on click — the chat/info swap glides in after.
  const deferredFocusedChannel = useDeferredValue(focusedChannel);

  const { userInfo } = useUserInfo(deferredFocusedChannel);
  const [isFollowing, setIsFollowing] = useState(false);

  const [activeTab, setActiveTabInternal] = useState<ActiveTab>(getInitialActiveTab);
  const [isSidebarOpen, setIsSidebarOpenInternal] = useState(getInitialSidebarOpen);
  const [isChatOpen, setIsChatOpenInternal] = useState(getInitialChatOpen);
  const [isFullscreen, setIsFullscreen] = useWindowFullscreen();

  const { isActive: isPointerActive, markActive: markPointerActive, reset: resetPointerActive } = useIdleTimer(2500);

  const { isLoggedIn, selfInfo, followedChannels, isLoadingFollowed, login, logout, refreshFollowedChannels } = useAuth();
  const { allEmotes, globalBadges, channelBadges, loadChannelEmotes } = useEmotes();
  const { topStreams, isLoading: isLoadingBrowse, loadTopStreams } = useTopStreams();
  const chat = useChat(deferredFocusedChannel, isLoggedIn);
  useUpdater();

  const setChannels = useCallback((updater: string[] | ((prev: string[]) => string[])) => {
    setChannelsInternal((prev) => {
      const next = typeof updater === "function" ? updater(prev) : updater;
      persistGridChannels(next);
      persistChannel(next[0] ?? null);
      return next;
    });
  }, []);

  const setFocusedIndex = useCallback((updater: number | ((prev: number) => number)) => {
    setFocusedIndexInternal((prev) => {
      const next = typeof updater === "function" ? updater(prev) : updater;
      persistFocusedIndex(next);
      return next;
    });
  }, []);

  const openPopout = useCallback(async (c: string) => {
    try {
      await invoke("open_popout", { channel: c });
    } catch (err) {
      logError(`[App] open_popout failed: ${err}`);
    }
  }, []);

  const selectChannel = useCallback((c: string | null) => {
    setChannels(c === null ? [] : [c]);
    setFocusedIndex(0);
  }, [setChannels, setFocusedIndex]);

  const addToGrid = useCallback((c: string) => {
    const existing = channels.indexOf(c);
    if (existing >= 0) {
      setFocusedIndex(existing);
      return;
    }
    if (channels.length >= GRID_MAX_TILES) {
      info(`[App] Grid full (${GRID_MAX_TILES} tiles), opening popout for ${c}`);
      openPopout(c);
      return;
    }
    setChannels([...channels, c]);
    setFocusedIndex(channels.length);
  }, [channels, setChannels, setFocusedIndex, openPopout]);

  const removeTile = useCallback((idx: number) => {
    const next = channels.filter((_, i) => i !== idx);
    setChannels(next);
    if (next.length === 0) {
      setFocusedIndex(0);
    } else if (idx < focusedIndex) {
      setFocusedIndex(Math.max(0, focusedIndex - 1));
    } else if (idx === focusedIndex) {
      setFocusedIndex(Math.min(focusedIndex, next.length - 1));
    }
  }, [channels, focusedIndex, setChannels, setFocusedIndex]);

  const setActiveTab = useCallback((tab: ActiveTab) => {
    persistActiveTab(tab);
    setActiveTabInternal(tab);
  }, []);

  const setIsSidebarOpen = useCallback((open: boolean) => {
    persistSidebarOpen(open);
    setIsSidebarOpenInternal(open);
  }, []);

  const setIsChatOpen = useCallback((open: boolean) => {
    persistChatOpen(open);
    setIsChatOpenInternal(open);
  }, []);

  const search = useSearch(selectChannel);

  const autospamFiredRef = useRef(false);
  const allEmotesRef = useRef(allEmotes);
  allEmotesRef.current = allEmotes;
  useEffect(() => {
    if (!import.meta.env.DEV || !focusedChannel) return;
    window.__spam = {
      start: (opts = {}) => startSpam(focusedChannel, Array.from(allEmotesRef.current.keys()), opts),
      stop: stopSpam,
    };

    const autospam = import.meta.env.VITE_AUTOSPAM as string | undefined;
    if (autospam && !autospamFiredRef.current && allEmotes.size > 0) {
      autospamFiredRef.current = true;
      const opts = JSON.parse(autospam);
      info(`[App] VITE_AUTOSPAM detected, starting in 5s with ${JSON.stringify(opts)}`);
      setTimeout(() => {
        const map = allEmotesRef.current;
        const names = Array.from(map.keys());
        const urls = Array.from(map.values());
        const animated = urls.filter((u) => /\/animated\/|(\.webp|\.gif)(\?|$)/i.test(u) || /7tv\.app|betterttv\.net|frankerfacez\.com/i.test(u)).length;
        info(`[App] starting spam: pool=${names.length} emotes (likely-animated≈${animated})`);
        startSpam(focusedChannel, names, opts);
      }, 5000);
    }

    return () => stopSpam();
  }, [focusedChannel, allEmotes]);

  useEffect(() => {
    attachConsole();
    info("[App] Initializing...");

    setTimeout(() => {
      invoke("show_main_window");
    }, 0);

    loadTopStreams();

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setIsFullscreen(false);
      }
    };
    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [loadTopStreams]);

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

    const id = setInterval(refreshData, REFRESH_INTERVAL);
    return () => clearInterval(id);
  }, [activeTab, isLoggedIn, selfInfo?.id, refreshFollowedChannels, loadTopStreams]);

  useEffect(() => {
    if (!userInfo?.id) return;
    loadChannelEmotes(userInfo.id);
  }, [userInfo?.id, loadChannelEmotes]);

  useEffect(() => {
    if (!isLoggedIn || !selfInfo?.id || !userInfo?.stream?.id) return;

    const ping = () => {
      invoke("update_watch_state", {
        channelLogin: userInfo.login,
        channelId: userInfo.id,
        streamId: userInfo.stream!.id,
        userId: selfInfo.id,
      }).catch((err) => logError(`[App] update_watch_state failed: ${err}`));
    };

    ping();
    const id = setInterval(ping, 60_000);
    return () => clearInterval(id);
  }, [userInfo?.id, userInfo?.login, userInfo?.stream?.id, isLoggedIn, selfInfo?.id]);

  useEffect(() => {
    if (isLoggedIn && userInfo) {
      const isInFollowedList = followedChannels.some(c => c.id === userInfo.id);
      setIsFollowing(isInFollowedList);
    } else {
      setIsFollowing(false);
    }
  }, [userInfo, followedChannels, isLoggedIn]);

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

  const canAddMoreTiles = channels.length < GRID_MAX_TILES;

  return (
    <div className="flex flex-col h-screen w-full bg-base text-[#e8e8ee]">
      {!isFullscreen && (
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
          onAddSearchResultToGrid={addToGrid}
          onOpenSearchResultPopout={openPopout}
          gridChannels={channels}
          canAddToGrid={canAddMoreTiles}
          onClearSearch={search.clearSearch}
          onSearch={() => {}}
          onOpenSidebar={() => setIsSidebarOpen(true)}
          onLoadTopStreams={() => loadTopStreams()}
          hasTopStreams={topStreams.length > 0}
          onGoHome={() => {
            selectChannel(null);
            setActiveTab("browse");
            if (topStreams.length === 0) {
              loadTopStreams();
            }
          }}
        />
      )}

      <div
        className="flex flex-1 overflow-hidden"
        onMouseMove={markPointerActive}
        onMouseLeave={resetPointerActive}
      >
        {!isFullscreen && (
          <Sidebar
            isOpen={isSidebarOpen}
            setIsOpen={setIsSidebarOpen}
            activeTab={activeTab}
            currentChannel={deferredFocusedChannel}
            gridChannels={channels}
            canAddToGrid={canAddMoreTiles}
            onSelectChannel={selectChannel}
            onAddToGrid={addToGrid}
            onOpenPopout={openPopout}
            followedChannels={followedChannels}
            isLoadingFollowed={isLoadingFollowed}
            isLoggedIn={isLoggedIn}
            topStreams={topStreams}
            isLoadingBrowse={isLoadingBrowse}
          />
        )}

        <main className="flex-1 bg-base flex flex-col relative overflow-hidden">
          {channels.length > 0 ? (
            <>
              <StreamGrid
                channels={channels}
                focusedIndex={focusedIndex}
                onFocusChange={setFocusedIndex}
                onRemoveTile={removeTile}
                onOpenPopout={openPopout}
                isFullscreen={isFullscreen}
                setIsFullscreen={setIsFullscreen}
              />
              {!isFullscreen && deferredFocusedChannel && (
                <StreamInfo
                  channel={deferredFocusedChannel}
                  userInfo={userInfo}
                  isFollowing={isFollowing}
                  isLoggedIn={isLoggedIn}
                  onFollow={handleFollow}
                />
              )}
            </>
          ) : (
            <BrowseGrid
              streams={topStreams}
              isLoading={isLoadingBrowse}
              isLoggedIn={isLoggedIn}
              onSelectChannel={selectChannel}
              onAddToGrid={addToGrid}
              canAddToGrid={canAddMoreTiles}
              onOpenPopout={openPopout}
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
          isConnected={chat.isConnected}
          onSendMessage={chat.sendMessage}
          isFullscreen={isFullscreen}
          openButtonVisible={isPointerActive}
        />
      </div>
    </div>
  );
}
