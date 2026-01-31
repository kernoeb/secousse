import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { error as logError } from "@tauri-apps/plugin-log";
import type { SearchResult, SearchChannelsResponse } from "../types";

interface UseSearchReturn {
  query: string;
  setQuery: (query: string) => void;
  results: SearchResult[];
  showResults: boolean;
  setShowResults: (show: boolean) => void;
  selectResult: (result: SearchResult) => void;
  clearSearch: () => void;
}

export function useSearch(onSelectChannel: (login: string) => void): UseSearchReturn {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [showResults, setShowResults] = useState(false);
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const search = useCallback(async (searchQuery: string) => {
    if (!searchQuery.trim()) {
      setResults([]);
      setShowResults(false);
      return;
    }

    try {
      const data = await invoke<SearchChannelsResponse>("search_channels", { query: searchQuery });
      if (data?.searchUsers?.edges) {
        const searchResults = data.searchUsers.edges.map((e) => e.node);
        setResults(searchResults);
        setShowResults(true);
      } else {
        setResults([]);
        setShowResults(false);
      }
    } catch (err) {
      logError(`[useSearch] Search error: ${err}`);
      setResults([]);
    }
  }, []);

  // Debounced search
  useEffect(() => {
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current);
    }

    if (query.trim()) {
      timeoutRef.current = setTimeout(() => {
        search(query);
      }, 300);
    } else {
      setResults([]);
      setShowResults(false);
    }

    return () => {
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }
    };
  }, [query, search]);

  const selectResult = useCallback((result: SearchResult) => {
    onSelectChannel(result.login);
    setQuery("");
    setResults([]);
    setShowResults(false);
  }, [onSelectChannel]);

  const clearSearch = useCallback(() => {
    setQuery("");
    setResults([]);
    setShowResults(false);
  }, []);

  return {
    query,
    setQuery,
    results,
    showResults,
    setShowResults,
    selectResult,
    clearSearch,
  };
}
