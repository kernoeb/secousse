import {
  type Loader,
  type LoaderCallbacks,
  type LoaderConfiguration,
  type LoaderContext,
  type LoaderStats,
} from 'hls.js';
import { fetch } from '@tauri-apps/plugin-http';

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
    try {
      const isText = this.context.responseType === 'text';
      const res = await fetch(this.context.url, { signal });

      if (signal.aborted) return;

      let data: string | ArrayBuffer;
      let size: number;
      if (isText) {
        const text = await res.text();
        data = text;
        size = text.length;
      } else {
        const buf = await res.arrayBuffer();
        data = buf;
        size = buf.byteLength;
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
      this.callbacks?.onError({ code: 0, text: String(e) }, this.context, undefined, this.stats);
    }
  }
}
