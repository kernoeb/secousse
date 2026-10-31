import { useEffect, useRef, useState } from "react";
import { Check, Languages, Search, User, LogIn, X } from "lucide-react";
import {
  formatViewers,
  LANGUAGE_NAMES,
  LANGUAGES,
  setLanguagePreference,
  systemLanguage,
  useI18n,
  type LanguagePreference,
} from "../i18n";
import { cn } from "../lib/utils";
import { ChannelActionButtons } from "./ChannelActionButtons";
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
  onAddSearchResultToGrid: (login: string) => void;
  onOpenSearchResultPopout: (login: string) => void;
  gridChannels: string[];
  canAddToGrid: boolean;
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
  onAddSearchResultToGrid,
  onOpenSearchResultPopout,
  gridChannels,
  canAddToGrid,
  onClearSearch,
  onSearch,
  onOpenSidebar,
  onLoadTopStreams,
  hasTopStreams,
  onGoHome,
}: NavbarProps) {
  const { t, language } = useI18n();
  const searchRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!showSearchResults) return;
    const onPointerDown = (e: MouseEvent) => {
      if (!searchRef.current?.contains(e.target as Node)) {
        setShowSearchResults(false);
      }
    };
    document.addEventListener("mousedown", onPointerDown);
    return () => document.removeEventListener("mousedown", onPointerDown);
  }, [showSearchResults, setShowSearchResults]);

  return (
    <nav className="h-12 border-b border-border flex items-center justify-between px-4 bg-surface-alt z-50">
      <div className="flex items-center gap-4 h-full">
        <button
          onClick={onGoHome}
          className="w-8 h-8 rounded-md flex items-center justify-center cursor-pointer hover:opacity-80 transition-opacity"
          title={t("nav.home")}
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
              ? "text-twitch border-b-2 border-twitch"
              : "text-muted hover:text-[#e8e8ee]"
          )}
        >
          {t("nav.following")}
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
              ? "text-twitch border-b-2 border-twitch"
              : "text-muted hover:text-[#e8e8ee]"
          )}
        >
          {t("nav.browse")}
        </button>
      </div>

      <div ref={searchRef} className="flex-1 max-w-md mx-4 relative">
        <div className="relative group flex">
          <input
            type="text"
            placeholder={t("nav.search")}
            autoComplete="off"
            autoCorrect="off"
            autoCapitalize="off"
            spellCheck={false}
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            onFocus={() => searchQuery && setShowSearchResults(true)}
            className="w-full bg-surface border border-border rounded-l-md py-1 px-3 text-sm focus:outline-none focus:border-twitch transition-colors placeholder:text-muted/50"
          />
          {searchQuery && (
            <button
              onClick={onClearSearch}
              className="absolute right-10 top-1/2 -translate-y-1/2 p-1 hover:bg-hover rounded"
            >
              <X className="w-3 h-3 text-muted" />
            </button>
          )}
          <button
            onClick={onSearch}
            className="bg-elevated px-2 rounded-r-md border-y border-r border-border hover:bg-hover"
          >
            <Search className="w-4 h-4 text-[#e8e8ee]" />
          </button>
        </div>

        {/* Search Results Dropdown */}
        {showSearchResults && searchResults.length > 0 && (
          <div className="absolute top-full left-0 right-0 mt-1 bg-elevated border border-border rounded-md shadow-xl z-[60] max-h-96 overflow-y-auto scrollbar-thin">
            {searchResults.map((result) => {
              const isInGrid = gridChannels.includes(result.login);
              return (
                <div key={result.id} className="relative">
                  <button
                    onClick={(e) => {
                      if (e.shiftKey) {
                        e.preventDefault();
                        onAddSearchResultToGrid(result.login);
                      } else {
                        onSelectSearchResult(result);
                      }
                    }}
                    className="w-full flex items-center gap-3 p-3 pr-16 hover:bg-hover transition-colors text-left"
                    title={canAddToGrid ? t("hint.playOrAdd") : t("hint.playOrPopout")}
                  >
                    <div className="w-10 h-10 rounded-full overflow-hidden bg-surface flex-shrink-0">
                      {result.profileImageURL && (
                        <img src={result.profileImageURL} alt={result.login} className="w-full h-full object-cover" />
                      )}
                    </div>
                    <div className="flex-1 min-w-0">
                      <div className="font-semibold text-sm truncate">{result.displayName}</div>
                      {result.stream ? (
                        <div className="flex items-center gap-2 text-xs text-muted">
                          <span className="flex items-center gap-1">
                            <div className="w-2 h-2 bg-red-600 rounded-full" />
                            {formatViewers(result.stream.viewersCount, language)}
                          </span>
                          <span className="truncate">{result.stream.game?.displayName || t("channel.streaming")}</span>
                        </div>
                      ) : (
                        <div className="text-xs text-muted">{t("channel.offline")}</div>
                      )}
                    </div>
                  </button>
                  <ChannelActionButtons
                    variant="row"
                    canAddToGrid={canAddToGrid}
                    isInGrid={isInGrid}
                    onAddToGrid={() => onAddSearchResultToGrid(result.login)}
                    onOpenPopout={() => onOpenSearchResultPopout(result.login)}
                    className="absolute top-1/2 right-2 -translate-y-1/2"
                  />
                </div>
              );
            })}
          </div>
        )}
      </div>

      <div className="flex items-center gap-3">
        <LanguageMenu />
        {isLoggedIn ? (
          <button
            onClick={onLogout}
            className="flex items-center gap-2 bg-elevated hover:bg-hover px-3 py-1 rounded-md font-semibold text-sm transition-colors text-white"
          >
            {t("nav.logout")}
          </button>
        ) : (
          <button
            onClick={onLogin}
            className="flex items-center gap-2 bg-twitch hover:bg-twitch-dark px-3 py-1 rounded-md font-semibold text-sm transition-colors text-white"
          >
            <LogIn className="w-4 h-4" /> {t("nav.login")}
          </button>
        )}
        <div className="w-8 h-8 bg-elevated rounded-full flex items-center justify-center overflow-hidden border border-border">
          {isLoggedIn && selfInfo?.profileImageURL ? (
            <img src={selfInfo.profileImageURL} alt={t("nav.myProfile")} className="w-full h-full object-cover" />
          ) : (
            <User className="w-5 h-5 opacity-50" />
          )}
        </div>
      </div>
    </nav>
  );
}

function LanguageMenu() {
  const { t, preference } = useI18n();
  const [isOpen, setIsOpen] = useState(false);
  const options: { value: LanguagePreference; label: string }[] = [
    { value: "system", label: t("language.system", { language: LANGUAGE_NAMES[systemLanguage()] }) },
    ...LANGUAGES.map((l) => ({ value: l, label: LANGUAGE_NAMES[l] })),
  ];

  return (
    <div className="relative">
      <button
        onClick={() => setIsOpen((o) => !o)}
        className="p-1.5 hover:bg-hover rounded-md transition-colors text-muted hover:text-white"
        title={t("nav.language")}
      >
        <Languages className="w-4 h-4" />
      </button>
      {isOpen && (
        <>
          <div className="fixed inset-0 z-[60]" onClick={() => setIsOpen(false)} />
          <div className="absolute top-full right-0 mt-1 bg-elevated border border-border rounded-md shadow-xl z-[61] min-w-[180px] py-1">
            {options.map((o) => (
              <button
                key={o.value}
                onClick={() => {
                  setLanguagePreference(o.value);
                  setIsOpen(false);
                }}
                className={cn(
                  "w-full px-3 py-1.5 text-left text-sm hover:bg-hover flex items-center justify-between gap-3",
                  preference === o.value && "text-twitch"
                )}
              >
                {o.label}
                {preference === o.value && <Check className="w-3.5 h-3.5" />}
              </button>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
