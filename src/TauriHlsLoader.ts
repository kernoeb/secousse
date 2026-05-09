import {
  type Loader,
  type LoaderCallbacks,
  type LoaderConfiguration,
  type LoaderContext,
  type LoaderStats,
} from 'hls.js';
import { fetch } from '@tauri-apps/plugin-http';
import { info } from '@tauri-apps/plugin-log';

// Twitch's CDN edges sometimes 403 requests without these set; matches what
// real twitch.tv sends. plugin-http on Windows doesn't add them by default.
const HLS_HEADERS = {
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

      if (!res.ok) {
        const body = await res.text().catch(() => '');
        info(`[HlsLoader] HTTP ${res.status} on ${urlTail}: ${body.slice(0, 120)}`);
        this.callbacks?.onError(
          { code: res.status, text: `HTTP ${res.status}: ${body.slice(0, 80)}` },
          this.context,
          undefined,
          this.stats,
        );
        return;
      }

      // Decode via TextDecoder rather than res.text() so we control the
      // encoding identically across platforms (some Tauri http plugin paths
      // returned latin1-decoded text on Windows under earlier diagnoses).
      const buf = await res.arrayBuffer();
      const data: string | ArrayBuffer = isText ? new TextDecoder('utf-8').decode(buf) : buf;
      const size = isText ? (data as string).length : buf.byteLength;

      if (isText) {
        const ctxType = (this.context as { type?: string }).type ?? 'unknown';
        const head = (data as string).slice(0, 80).replace(/\n/g, ' \\n ');
        info(`[HlsLoader] ${ctxType} ${size}B head="${head}"`);
      }

      const now = performance.now();
      this.stats.loading.first ||= now;
      this.stats.loading.end = now;
      this.stats.loaded = size;
      this.stats.total = size;
      this.stats.parsing = { start: now, end: now };
      this.stats.buffering = { start: now, first: now, end: now };

      this.callbacks?.onSuccess({ url: this.context.url, data }, this.stats, this.context, undefined);
    } catch (e) {
      if (signal.aborted) return;
      info(`[HlsLoader] fetch failed for ${urlTail}: ${e}`);
      this.callbacks?.onError({ code: 0, text: String(e) }, this.context, undefined, this.stats);
    }
  }
}
