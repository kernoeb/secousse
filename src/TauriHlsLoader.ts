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

export class TauriHlsLoader implements Loader<LoaderContext> {
  public context!: LoaderContext;
  public stats: LoaderStats;
  private callbacks: LoaderCallbacks<LoaderContext> | null = null;
  private abortController = new AbortController();

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
    this.abortController.abort();
  }

  abort(): void {
    this.stats.aborted = true;
    this.abortController.abort();
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
    this.abortController.abort();
    this.abortController = new AbortController();
    this.doFetch();
  }

  private async doFetch() {
    const { signal } = this.abortController;
    const urlTail = this.context.url.slice(-60);
    try {
      const isText = this.context.responseType === 'text';
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

      // Decode via TextDecoder rather than res.text() so the encoding is
      // controlled identically across platforms.
      const buf = await res.arrayBuffer();
      const data: string | ArrayBuffer = isText ? new TextDecoder('utf-8').decode(buf) : buf;
      const size = isText ? (data as string).length : buf.byteLength;

      const now = performance.now();
      this.stats.loading.end = now;
      this.stats.loaded = size;
      this.stats.total = size;
      this.stats.parsing = { start: now, end: now };
      this.stats.buffering = { start: this.stats.loading.first, first: this.stats.loading.first, end: now };

      this.callbacks?.onSuccess({ url: this.context.url, data }, this.stats, this.context, undefined);
    } catch (e) {
      if (signal.aborted) return;
      info(`[HlsLoader] fetch failed for ${urlTail}: ${e}`);
      this.callbacks?.onError({ code: 0, text: String(e) }, this.context, undefined, this.stats);
    }
  }
}
