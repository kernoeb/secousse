import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

/** Merge Tailwind classes with clsx */
export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

/** Format viewer count with K/M suffixes */
export function formatViewers(count: number): string {
  if (count >= 1000000) return (count / 1000000).toFixed(1) + "M";
  if (count >= 1000) return (count / 1000).toFixed(1) + "K";
  return count.toString();
}

/** Get initial channel from storage (for persistence across HMR and restarts) */
export function getInitialChannel(): string | null {
  if (typeof window !== "undefined") {
    return sessionStorage.getItem("currentChannel") || localStorage.getItem("lastChannel") || null;
  }
  return null;
}

/** Get initial active tab from storage */
export function getInitialActiveTab(): "following" | "browse" {
  if (typeof window !== "undefined") {
    const saved = localStorage.getItem("activeTab");
    if (saved === "following" || saved === "browse") return saved;
  }
  return "browse";
}

/** Persist channel to storage */
export function persistChannel(channel: string | null) {
  if (channel) {
    sessionStorage.setItem("currentChannel", channel);
    localStorage.setItem("lastChannel", channel);
  } else {
    sessionStorage.removeItem("currentChannel");
  }
}

/** Persist active tab to storage */
export function persistActiveTab(tab: "following" | "browse") {
  localStorage.setItem("activeTab", tab);
}
