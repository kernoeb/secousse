import {
  type Loader,
  type LoaderCallbacks,
  type LoaderConfiguration,
  type LoaderContext,
  type LoaderStats,
} from 'hls.js';
import { fetch } from '@tauri-apps/plugin-http';
import { info } from '@tauri-apps/plugin-log';

// Twitch's CDN edges 403 requests with the default reqwest User-Agent
// (observed on Windows v0.1.2: master playlist fetches OK but every
// variant returns 403). Matching what twitch.tv's web player sends.
const HLS_HEADERS = {
  'User-Agent':
    'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
  Origin: 'https://www.twitch.tv',
  Referer: 'https://www.twitch.tv/',
};

// Twitch low-latency tag: announces future segment URLs before they finish
// encoding. CDN serves them via chunked transfer, streaming bytes as the
// encoder produces them. hls.js doesn't understand the tag, so rewrite each
// `#EXT-X-TWITCH-PREFETCH:URL` into a standard `#EXTINF:dur,live\nURL` pair.
// The win is starting the request early: the first byte arrives ~30ms in and
// the CDN streams the rest over the ~2s of remaining encode time, so the
// segment is in hand as soon as it exists. The bytes are still handed to
// hls.js as one complete segment — see the progressive note in VideoPlayer
// for why feeding them in pieces measured worse.
//
// A promoted prefetch keeps its media sequence number when the same media
// turns up as a real EXTINF on the next refresh, but NOT its URL: Twitch
// signs each prefetch separately, so the path differs every time. hls.js
// compares URLs per sequence number and flags the difference as a playlist
// error, which is why the player sets `ignorePlaylistParsingErrors`.
//
// Returns the input unchanged when the playlist has no prefetch tags (e.g.
// the master playlist, or non-Twitch sources).
const TWITCH_PREFETCH_TAG = '#EXT-X-TWITCH-PREFETCH:';
const EXTINF_RE = /^#EXTINF:([\d.]+)/;

export function promotePrefetches(text: string): string {
  if (!text.startsWith('#EXTM3U') || !text.includes(TWITCH_PREFETCH_TAG)) {
    return text;
  }

  const lines = text.split('\n');
  let lastDuration = '4.0';
  const prefetches: string[] = [];
  const kept: string[] = [];

  for (const line of lines) {
    if (line.startsWith(TWITCH_PREFETCH_TAG)) {
      prefetches.push(line.slice(TWITCH_PREFETCH_TAG.length).trim());
    } else {
      const m = EXTINF_RE.exec(line);
      if (m) lastDuration = m[1];
      kept.push(line);
    }
  }

  if (prefetches.length === 0) return text;

  // Strip trailing blanks before appending so the EXTINF/URL pairs stay
  // contiguous (hls.js' parser is strict about line ordering).
  while (kept.length > 0 && kept[kept.length - 1].trim() === '') kept.pop();

  for (const url of prefetches) {
    kept.push(`#EXTINF:${lastDuration},live`);
    kept.push(url);
  }

  return kept.join('\n') + '\n';
}

export class TauriHlsLoader implements Loader<LoaderContext> {
  public context!: LoaderContext;
  public stats: LoaderStats;
  private callbacks: LoaderCallbacks<LoaderContext> | null = null;
  // Tracks the in-flight fetch's abort controller, or null when no fetch is
  // active. Calling abort() on a completed fetch's controller still fires
  // the AbortSignal listeners that @tauri-apps/plugin-http attaches to the
  // body ReadableStream — those listeners call fetch_cancel_body on a
  // resource id that's already been released, causing "resource id N is
  // invalid" unhandled rejections (one per segment). Null when done.
  private currentAbort: AbortController | null = null;

  constructor() {
    this.stats = {
      aborted: false,
      loaded: 0,
      retry: 0,
      total: 0,
      chunkCount: 0,
      bwEstimate: 0,
      loading: { start: 0, first: 0, end: 0 },
      parsing: { start: 0, end: 0 },
      buffering: { start: 0, first: 0, end: 0 },
    };
  }

  destroy(): void {
    this.callbacks = null;
    this.cancelInFlight();
  }

  abort(): void {
    this.stats.aborted = true;
    this.cancelInFlight();
    this.callbacks?.onAbort?.(this.stats, this.context, undefined);
  }

  load(
    context: LoaderContext,
    _config: LoaderConfiguration,
    callbacks: LoaderCallbacks<LoaderContext>
  ): void {
    this.context = context;
    this.callbacks = callbacks;
    this.stats.loading.start = performance.now();
    this.cancelInFlight();
    this.doFetch();
  }

  private cancelInFlight() {
    if (this.currentAbort) {
      this.currentAbort.abort();
      this.currentAbort = null;
    }
  }

  private async doFetch() {
    const abort = new AbortController();
    this.currentAbort = abort;
    const { signal } = abort;
    const urlTail = this.context.url.slice(-60);
    const isText = this.context.responseType === 'text';
    try {
      const res = await fetch(this.context.url, { signal, headers: HLS_HEADERS });
      if (signal.aborted) return;

      // TTFB sample for hls.js: headers received here, body not yet read.
      // Previously `loading.first` was set after the full body arrived, so
      // `first === end`. hls.js then fed the total transfer time to its
      // TTFB estimator and computed `processingMs = total - ~total ≈ 0`,
      // which inflated the bandwidth estimate and pinned ABR to the top
      // level regardless of actual link capacity.
      this.stats.loading.first = performance.now();

      if (!res.ok) {
        const body = await res.text().catch(() => '');
        info(`[HlsLoader] HTTP ${res.status} on ${urlTail}: ${body.slice(0, 80)}`);
        this.callbacks?.onError(
          { code: res.status, text: `HTTP ${res.status}` },
          this.context,
          undefined,
          this.stats,
        );
        return;
      }

      if (isText) {
        await this.handleText(res, abort);
      } else {
        await this.handleStreamingSegment(res, urlTail, signal, abort);
      }
    } catch (e) {
      this.releaseAbort(abort);
      if (signal.aborted) return;
      info(`[HlsLoader] fetch failed for ${urlTail}: ${e}`);
      this.callbacks?.onError({ code: 0, text: String(e) }, this.context, undefined, this.stats);
    }
  }

  // Drops the loader's reference to a specific fetch's controller iff it's
  // still the active one. Guards a narrow race where a fetch completes after
  // an external load() has already replaced the controller — without the
  // identity check, the late completion would clobber the new fetch's abort.
  private releaseAbort(abort: AbortController) {
    if (this.currentAbort === abort) this.currentAbort = null;
  }

  // Playlists are small (~5-15KB) and the parser needs the whole file at once.
  // No benefit to streaming them, but media playlists need a rewrite pass
  // for Twitch's proprietary EXT-X-TWITCH-PREFETCH tags (see promotePrefetches).
  private async handleText(res: Response, abort: AbortController) {
    const buf = await res.arrayBuffer();
    const raw = new TextDecoder('utf-8').decode(buf);
    const rewritten = promotePrefetches(raw);

    const now = performance.now();
    this.stats.loading.end = now;
    this.stats.loaded = rewritten.length;
    this.stats.total = rewritten.length;
    this.stats.parsing = { start: now, end: now };
    this.stats.buffering = { start: this.stats.loading.first, first: this.stats.loading.first, end: now };

    // Body fully consumed → plugin-http auto-releases the response resource
    // on the Rust side. Drop our controller reference BEFORE onSuccess to
    // avoid a sync destroy() call from hls.js firing abort on a freed rid.
    this.releaseAbort(abort);
    this.callbacks?.onSuccess({ url: this.context.url, data: rewritten }, this.stats, this.context, undefined);
  }

  // Reads the body chunk-by-chunk so `stats.loaded` tracks a Twitch prefetch
  // as its chunked-transfer response trickles in over the encoder's remaining
  // ~2s, which keeps hls.js' bandwidth estimate honest. Chunks are handed on
  // individually only when hls.js asks for progress (config.progressive);
  // otherwise they are joined and delivered once, complete.
  private async handleStreamingSegment(res: Response, urlTail: string, signal: AbortSignal, abort: AbortController) {
    const onProgress = this.callbacks?.onProgress;

    if (!res.body) {
      // Fallback: should never happen on Twitch, but degrade gracefully.
      const buf = await res.arrayBuffer();
      this.releaseAbort(abort);
      this.finalizeSegment(buf);
      return;
    }

    const reader = res.body.getReader();
    // Only accumulate when nobody is consuming the chunks, since the whole
    // payload then has to go out through onSuccess instead.
    const chunks: Uint8Array[] | null = onProgress ? null : [];
    let totalBytes = 0;

    try {
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        if (signal.aborted) {
          try { reader.cancel(); } catch { /* noop */ }
          return;
        }

        chunks?.push(value);
        totalBytes += value.byteLength;
        this.stats.loaded = totalBytes;

        if (onProgress) {
          // plugin-http's ReadableStream yields fresh Uint8Arrays (via slice
          // on the Rust side) whose underlying buffer matches the view 1:1,
          // so the buffer can be passed directly to hls.js' transmuxer
          // without a copy. Guard kept for any future change in upstream.
          const chunkBuf = value.byteOffset === 0 && value.byteLength === value.buffer.byteLength
            ? value.buffer
            : value.buffer.slice(value.byteOffset, value.byteOffset + value.byteLength);
          onProgress(this.stats, this.context, chunkBuf, undefined);
        }
      }
    } catch (e) {
      if (signal.aborted) return;
      info(`[HlsLoader] stream read failed for ${urlTail}: ${e}`);
      this.callbacks?.onError({ code: 0, text: String(e) }, this.context, undefined, this.stats);
      return;
    }

    // Body fully consumed → plugin-http auto-releases the response resource
    // on the Rust side. Drop our controller reference BEFORE onSuccess to
    // avoid a sync destroy() call from hls.js firing abort on a freed rid.
    this.releaseAbort(abort);

    if (!chunks) {
      // Every byte already reached the transmuxer through onProgress. hls.js'
      // own fetch loader resolves the progressive path with an empty buffer
      // (see loadProgressively), and base-stream-controller relies on that:
      // handing it the full segment here makes it demux and append the same
      // media a second time, which shatters the buffer into ranges and sends
      // the player chasing holes it just created.
      this.finalizeSegment(new ArrayBuffer(0), totalBytes);
      return;
    }

    const full = new Uint8Array(totalBytes);
    let offset = 0;
    for (const c of chunks) {
      full.set(c, offset);
      offset += c.byteLength;
    }
    this.finalizeSegment(full.buffer, totalBytes);
  }

  private finalizeSegment(data: ArrayBuffer, transferred = data.byteLength) {
    const now = performance.now();
    this.stats.loading.end = now;
    this.stats.total = transferred;
    this.stats.loaded = transferred;
    this.stats.parsing = { start: now, end: now };
    this.stats.buffering = { start: this.stats.loading.first, first: this.stats.loading.first, end: now };

    this.callbacks?.onSuccess({ url: this.context.url, data }, this.stats, this.context, undefined);
  }
}
