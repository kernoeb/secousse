import {
  type Loader,
  type LoaderCallbacks,
  type LoaderConfiguration,
  type LoaderContext,
  type LoaderResponse,
  type LoaderStats,
} from 'hls.js';
import { invoke } from '@tauri-apps/api/core';

export class TauriHlsLoader implements Loader<LoaderContext> {
  public context!: LoaderContext;
  public stats: LoaderStats;
  private callbacks: LoaderCallbacks<LoaderContext> | null = null;
  private destroyed: boolean = false;

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
    this.destroyed = true;
    this.callbacks = null;
  }

  abort(): void {
    this.stats.aborted = true;
    if (this.callbacks?.onAbort) {
      this.callbacks.onAbort(this.stats, this.context, undefined);
    }
  }

  load(
    context: LoaderContext,
    _config: LoaderConfiguration,
    callbacks: LoaderCallbacks<LoaderContext>
  ): void {
    this.context = context;
    this.callbacks = callbacks;
    this.stats.loading.start = performance.now();

    const isPlaylist = context.url.includes('.m3u8');

    if (isPlaylist) {
      this.loadPlaylist();
    } else {
      // Twitch segments usually have CORS, but since we are seeing 403/CORS issues in the logs,
      // let's proxy them too but using fetch_bytes for efficiency.
      this.loadSegment();
    }
  }

  private async loadPlaylist() {
    try {
      const data: string = await invoke('fetch_m3u8', { url: this.context.url });
      if (this.destroyed || this.stats.aborted) return;

      const now = performance.now();
      if (!this.stats.loading.first) this.stats.loading.first = now;
      this.stats.loading.end = now;
      this.stats.loaded = data.length;
      this.stats.total = data.length;

      // Crucial: Initialize sub-objects if hls.js expects them to be there
      this.stats.parsing = { start: now, end: now };
      this.stats.buffering = { start: now, first: now, end: now };

      const response: LoaderResponse = {
        url: this.context.url,
        data: data,
      };

      this.callbacks?.onSuccess(response, this.stats, this.context, undefined);
    } catch (e) {
      console.error('[TauriHlsLoader] Playlist fetch failed:', this.context.url, e);
      if (this.destroyed || this.stats.aborted) return;
      this.callbacks?.onError({ code: 0, text: String(e) }, this.context, undefined, this.stats);
    }
  }

  private async loadSegment() {
    try {
      const data: number[] = await invoke('fetch_bytes', { url: this.context.url });
      if (this.destroyed || this.stats.aborted) return;

      const arrayBuffer = new Uint8Array(data).buffer;
      const now = performance.now();
      if (!this.stats.loading.first) this.stats.loading.first = now;
      this.stats.loading.end = now;
      this.stats.loaded = arrayBuffer.byteLength;
      this.stats.total = arrayBuffer.byteLength;

      this.stats.parsing = { start: now, end: now };
      this.stats.buffering = { start: now, first: now, end: now };

      const response: LoaderResponse = {
        url: this.context.url,
        data: arrayBuffer,
      };

      this.callbacks?.onSuccess(response, this.stats, this.context, undefined);
    } catch (e) {
      console.error('[TauriHlsLoader] Segment fetch failed:', this.context.url, e);
      if (this.destroyed || this.stats.aborted) return;
      this.callbacks?.onError({ code: 0, text: String(e) }, this.context, undefined, this.stats);
    }
  }
}
