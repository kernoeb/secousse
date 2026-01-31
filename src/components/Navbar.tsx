import { Search, User, LogIn, X } from "lucide-react";
import { cn, formatViewers } from "../lib/utils";
import type { SelfInfo, SearchResult, ActiveTab } from "../types";

interface NavbarProps {
  activeTab: ActiveTab;
  setActiveTab: (tab: ActiveTab) => void;
  isLoggedIn: boolean;
  selfInfo: SelfInfo | null;
  onLogin: () => void;
  onLogout: () => void;
  searchQuery: string;
  setSearchQuery: (query: string) => void;
  searchResults: SearchResult[];
  showSearchResults: boolean;
  setShowSearchResults: (show: boolean) => void;
  onSelectSearchResult: (result: SearchResult) => void;
  onClearSearch: () => void;
  onSearch: () => void;
  onOpenSidebar: () => void;
  onLoadTopStreams: () => void;
  hasTopStreams: boolean;
  onGoHome: () => void;
}

export function Navbar({
  activeTab,
  setActiveTab,
  isLoggedIn,
  selfInfo,
  onLogin,
  onLogout,
  searchQuery,
  setSearchQuery,
  searchResults,
  showSearchResults,
  setShowSearchResults,
  onSelectSearchResult,
  onClearSearch,
  onSearch,
  onOpenSidebar,
  onLoadTopStreams,
  hasTopStreams,
  onGoHome,
}: NavbarProps) {
  return (
    <nav className="h-12 border-b border-black flex items-center justify-between px-4 bg-[#18181b] z-50">
      <div className="flex items-center gap-4 h-full">
        <button
          onClick={onGoHome}
          className="w-8 h-8 rounded-md flex items-center justify-center cursor-pointer hover:opacity-80 transition-opacity"
          title="Home"
        >
          <img src="/app-icon.svg" alt="Secousse" className="w-8 h-8" />
        </button>
        <button
          onClick={() => {
            setActiveTab("following");
            onOpenSidebar();
          }}
          className={cn(
            "font-semibold text-sm px-3 h-full cursor-pointer transition-colors",
            activeTab === "following"
              ? "text-[#9146ff] border-b-2 border-[#9146ff]"
              : "hover:text-[#9146ff]"
          )}
        >
          Following
        </button>
        <button
          onClick={() => {
            setActiveTab("browse");
            if (!hasTopStreams) {
              onLoadTopStreams();
            }
          }}
          className={cn(
            "font-semibold text-sm px-3 h-full cursor-pointer transition-colors",
            activeTab === "browse"
              ? "text-[#9146ff] border-b-2 border-[#9146ff]"
              : "hover:text-[#9146ff]"
          )}
        >
          Browse
        </button>
      </div>

      <div className="flex-1 max-w-md mx-4 relative">
        <div className="relative group flex">
          <input
            type="text"
            placeholder="Search channels..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            onFocus={() => searchQuery && setShowSearchResults(true)}
            className="w-full bg-[#18181b] border border-[#3f3f46] rounded-l-md py-1 px-3 text-sm focus:outline-none focus:border-[#9146ff] transition-colors"
          />
          {searchQuery && (
            <button
              onClick={onClearSearch}
              className="absolute right-10 top-1/2 -translate-y-1/2 p-1 hover:bg-[#3f3f46] rounded"
            >
              <X className="w-3 h-3 text-[#adadb8]" />
            </button>
          )}
          <button
            onClick={onSearch}
            className="bg-[#2f2f35] px-2 rounded-r-md border-y border-r border-[#3f3f46] hover:bg-[#3f3f46]"
          >
            <Search className="w-4 h-4 text-[#efeff1]" />
          </button>
        </div>

        {/* Search Results Dropdown */}
        {showSearchResults && searchResults.length > 0 && (
          <div className="absolute top-full left-0 right-0 mt-1 bg-[#18181b] border border-[#3f3f46] rounded-md shadow-xl z-[60] max-h-96 overflow-y-auto">
            {searchResults.map((result) => (
              <button
                key={result.id}
                onClick={() => onSelectSearchResult(result)}
                className="w-full flex items-center gap-3 p-3 hover:bg-[#2f2f35] transition-colors text-left"
              >
                <div className="w-10 h-10 rounded-full overflow-hidden bg-[#3f3f46] flex-shrink-0">
                  {result.profileImageURL && (
                    <img src={result.profileImageURL} alt={result.login} className="w-full h-full object-cover" />
                  )}
                </div>
                <div className="flex-1 min-w-0">
                  <div className="font-semibold text-sm truncate">{result.displayName}</div>
                  {result.stream ? (
                    <div className="flex items-center gap-2 text-xs text-[#adadb8]">
                      <span className="flex items-center gap-1">
                        <div className="w-2 h-2 bg-red-600 rounded-full" />
                        {formatViewers(result.stream.viewersCount)}
                      </span>
                      <span className="truncate">{result.stream.game?.displayName || "Streaming"}</span>
                    </div>
                  ) : (
                    <div className="text-xs text-[#adadb8]">Offline</div>
                  )}
                </div>
              </button>
            ))}
          </div>
        )}

        {/* Click outside to close */}
        {showSearchResults && (
          <div
            className="fixed inset-0 z-[55]"
            onClick={() => setShowSearchResults(false)}
          />
        )}
      </div>

      <div className="flex items-center gap-3">
        {isLoggedIn ? (
          <button
            onClick={onLogout}
            className="flex items-center gap-2 bg-[#2f2f35] hover:bg-[#3f3f46] px-3 py-1 rounded-md font-semibold text-sm transition-colors text-white"
          >
            Logout
          </button>
        ) : (
          <button
            onClick={onLogin}
            className="flex items-center gap-2 bg-[#9146ff] hover:bg-[#772ce8] px-3 py-1 rounded-md font-semibold text-sm transition-colors text-white"
          >
            <LogIn className="w-4 h-4" /> Log In
          </button>
        )}
        <div className="w-8 h-8 bg-[#3f3f46] rounded-full flex items-center justify-center overflow-hidden border border-white/10 cursor-pointer">
          {isLoggedIn && selfInfo?.profileImageURL ? (
            <img src={selfInfo.profileImageURL} alt="My profile" className="w-full h-full object-cover" />
          ) : (
            <User className="w-5 h-5 opacity-50" />
          )}
        </div>
      </div>
    </nav>
  );
}
