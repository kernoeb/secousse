import { memo, useCallback, useEffect, useRef, useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import { warn } from "@tauri-apps/plugin-log";
import { cn } from "../lib/utils";

// WKWebView allocates one serial dispatch_queue (`WebCore::ImageFrameWorkQueue`)
// per animated <img>; the macOS dispatch pool caps near 512 workers per
// process, so a busy chat with WebP-animated emotes freezes within minutes.
// We side-step the pool by decoding in Rust and rendering to <canvas>:
// no animated <img> is ever instantiated, so zero IFWQs are allocated.
// (WebCodecs ImageDecoder would also work, but WKWebView gates it behind
// an experimental flag that isn't exposed at runtime.)

type EmoteStatus = "loading" | "static" | "animated" | "error";

interface CachedEmote {
  status: EmoteStatus;
  frames: ImageBitmap[];
  durations: number[];
  totalDuration: number;
  lastTouchedAt: number;
  subscribers: Set<() => void>;
  refCount: number;
  evictTimer?: ReturnType<typeof setTimeout>;
}

function entryBytes(entry: CachedEmote): number {
  const first = entry.frames[0];
  return first ? first.width * first.height * 4 * entry.frames.length : 0;
}

// Cap by raw pixel bytes, not URL count: a 30-frame WebP at 32×32 costs
// ~122 KB while a 1-frame PNG costs 4 KB. Sizing the cache by URL count
// (e.g. 400) lets the byte total swing from 1 MB to 50 MB, which
// snowballs into 1-2 GB of GPU compositor copies under sustained load.
const MAX_CACHED_BYTES = 32 * 1024 * 1024;
// Don't drop below ~30s. Shorter values thrash the cache under chat bursts:
// emotes scroll out, get evicted, then re-decode when they reappear,
// snowballing memory because allocations beat GC.
const EVICT_DELAY_MS = 30_000;
// Cap parallel decodes so a burst doesn't triple-stack ArrayBuffer +
// Uint8ClampedArray + in-flight ImageBitmap allocations before each one
// is GC'd. 6 keeps throughput up; lower values can backlog the queue.
const DECODE_CONCURRENCY = 6;

let totalCachedBytes = 0;

const cache = new Map<string, CachedEmote>();
const decodeQueue: Array<() => Promise<void>> = [];
let decodeInFlight = 0;

function pumpDecodeQueue() {
  // LIFO: under a chat burst, the most recently mounted emote is the most
  // likely to still be onscreen, so prioritize newer requests.
  while (decodeInFlight < DECODE_CONCURRENCY && decodeQueue.length > 0) {
    const task = decodeQueue.pop()!;
    decodeInFlight++;
    task().finally(() => {
      decodeInFlight--;
      pumpDecodeQueue();
    });
  }
}

function notifyAll(entry: CachedEmote) {
  for (const cb of entry.subscribers) cb();
}

function freeFrames(entry: CachedEmote) {
  totalCachedBytes -= entryBytes(entry);
  for (const f of entry.frames) f.close();
  entry.frames = [];
  entry.durations = [];
  entry.totalDuration = 0;
  if (entry.evictTimer) {
    clearTimeout(entry.evictTimer);
    entry.evictTimer = undefined;
  }
}

function evict(url: string) {
  const entry = cache.get(url);
  if (!entry || entry.refCount > 0) return;
  freeFrames(entry);
  cache.delete(url);
}

function maybeEvictLRU() {
  if (totalCachedBytes <= MAX_CACHED_BYTES) return;
  const candidates: Array<[string, CachedEmote]> = [];
  for (const e of cache) if (e[1].refCount === 0) candidates.push(e);
  candidates.sort((a, b) => a[1].lastTouchedAt - b[1].lastTouchedAt);
  for (const [url, entry] of candidates) {
    freeFrames(entry);
    cache.delete(url);
    if (totalCachedBytes <= MAX_CACHED_BYTES) return;
  }
}

async function decodeUrl(url: string, entry: CachedEmote) {
  try {
    const buf = await invoke<ArrayBuffer>("decode_emote", { url });
    const view = new DataView(buf);
    const w = view.getUint32(0, true);
    const h = view.getUint32(4, true);
    const count = view.getUint32(8, true);
    const frameBytes = w * h * 4;
    const expected = 12 + count * (4 + frameBytes);
    if (buf.byteLength !== expected) {
      throw new Error(`buffer length ${buf.byteLength} != expected ${expected}`);
    }

    const durations: number[] = new Array(count);
    const imageDatas: ImageData[] = new Array(count);
    let total = 0;
    let off = 12;
    for (let i = 0; i < count; i++) {
      const d = view.getUint32(off, true);
      off += 4;
      durations[i] = d;
      total += d;
      imageDatas[i] = new ImageData(new Uint8ClampedArray(buf, off, frameBytes), w, h);
      off += frameBytes;
    }
    const frames = await Promise.all(imageDatas.map((d) => createImageBitmap(d)));

    if (cache.get(url) !== entry) {
      for (const f of frames) f.close();
      return;
    }
    entry.frames = frames;
    entry.durations = durations;
    entry.totalDuration = total;
    totalCachedBytes += frameBytes * count;
    entry.status = count > 1 ? "animated" : "static";
  } catch (e) {
    entry.status = "error";
    warn(`[EmoteImg] decode failed for ${url.slice(-50)}: ${e}`);
  }

  if (cache.get(url) !== entry) {
    freeFrames(entry);
    return;
  }
  notifyAll(entry);
  maybeEvictLRU();
}

interface ActiveCanvas {
  canvas: HTMLCanvasElement;
  ctx: CanvasRenderingContext2D;
  url: string;
  startedAt: number;
  lastFrameIdx: number;
}

const active = new Set<ActiveCanvas>();
let rafHandle = 0;
// Reused across ticks to avoid per-frame Map allocation under heavy chat load.
const tickFrameIdx = new Map<string, number>();

function tick(now: DOMHighResTimeStamp) {
  rafHandle = 0;
  if (active.size === 0) return;

  tickFrameIdx.clear();
  for (const ac of active) {
    const entry = cache.get(ac.url);
    if (!entry || entry.status !== "animated") continue;
    let idx = tickFrameIdx.get(ac.url);
    if (idx === undefined) {
      const t = (now - ac.startedAt) % entry.totalDuration;
      let acc = 0;
      idx = 0;
      for (let i = 0; i < entry.frames.length; i++) {
        acc += entry.durations[i];
        if (t < acc) {
          idx = i;
          break;
        }
      }
      tickFrameIdx.set(ac.url, idx);
    }
    if (idx !== ac.lastFrameIdx) {
      ac.ctx.clearRect(0, 0, ac.canvas.width, ac.canvas.height);
      ac.ctx.drawImage(entry.frames[idx], 0, 0, ac.canvas.width, ac.canvas.height);
      ac.lastFrameIdx = idx;
    }
  }
  startTick();
}

function startTick() {
  if (rafHandle === 0 && active.size > 0) {
    rafHandle = requestAnimationFrame(tick);
  }
}

function useEmote(url: string): CachedEmote | null {
  const subscribe = useCallback(
    (cb: () => void) => {
      let entry = cache.get(url);
      if (!entry) {
        const newEntry: CachedEmote = {
          status: "loading",
          frames: [],
          durations: [],
          totalDuration: 0,
          subscribers: new Set(),
          refCount: 0,
          lastTouchedAt: 0,
        };
        entry = newEntry;
        cache.set(url, newEntry);
        decodeQueue.push(() => decodeUrl(url, newEntry));
        pumpDecodeQueue();
      }
      entry.refCount++;
      entry.lastTouchedAt = performance.now();
      if (entry.evictTimer) {
        clearTimeout(entry.evictTimer);
        entry.evictTimer = undefined;
      }
      entry.subscribers.add(cb);
      return () => {
        const e = cache.get(url);
        if (!e) return;
        e.subscribers.delete(cb);
        e.refCount--;
        if (e.refCount === 0) {
          e.evictTimer = setTimeout(() => evict(url), EVICT_DELAY_MS);
        }
      };
    },
    [url],
  );
  const getSnapshot = useCallback(() => cache.get(url)?.status ?? "loading", [url]);
  useSyncExternalStore(subscribe, getSnapshot);
  return cache.get(url) ?? null;
}

interface EmoteImgProps {
  url: string;
  alt: string;
  className?: string;
  /** Fires when the rendered size is final, so the chat can re-pin scroll. */
  onReady?: () => void;
}

const BASE_CLASS = "inline-block h-6 mx-0.5 align-middle";

export const EmoteImg = memo(function EmoteImg({ url, alt, className, onReady }: EmoteImgProps) {
  const entry = useEmote(url);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const onReadyRef = useRef(onReady);
  onReadyRef.current = onReady;

  const firstFrame = entry?.frames[0];

  useEffect(() => {
    if (!entry || !firstFrame) return;
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    canvas.width = firstFrame.width;
    canvas.height = firstFrame.height;
    ctx.drawImage(firstFrame, 0, 0);
    onReadyRef.current?.();

    if (entry.status === "static") return;

    const ac: ActiveCanvas = {
      canvas,
      ctx,
      url,
      startedAt: performance.now(),
      lastFrameIdx: 0,
    };
    active.add(ac);
    startTick();

    return () => {
      active.delete(ac);
    };
  }, [url, entry?.status, firstFrame]);

  if (!entry || entry.status === "loading" || entry.status === "error") {
    return (
      <span
        className={cn(BASE_CLASS, "w-6", className)}
        aria-label={alt}
        title={entry?.status === "error" ? alt : undefined}
      />
    );
  }
  return (
    <canvas
      ref={canvasRef}
      aria-label={alt}
      className={cn(BASE_CLASS, className)}
      style={firstFrame ? { aspectRatio: `${firstFrame.width} / ${firstFrame.height}` } : undefined}
    />
  );
});
