import { useRef, useState, useEffect, useCallback } from "react";
import { useI18n } from "../i18n";
import Hls from "hls.js";
import { invoke } from "@tauri-apps/api/core";
import { debug, info, error as logError } from "@tauri-apps/plugin-log";
import { Play, Pause, Volume2, VolumeX, Settings, Maximize, Minimize, Loader2, RotateCcw, RotateCw } from "lucide-react";
import { TauriHlsLoader } from "../TauriHlsLoader";
import { SeekBar, formatBehind } from "./SeekBar";
import { useIdleTimer } from "../hooks";
import {
  cn,
  AUTO_QUALITY,
  getInitialVolume,
  persistVolume,
  getInitialMuted,
  persistMuted,
  getInitialPreferredQualityHeight,
  persistPreferredQualityHeight,
} from "../lib/utils";
import type { UserInfo, QualityLevel, VodPlayback } from "../types";

const VIDEO_STYLE: React.CSSProperties = { transform: "translateZ(0)" };
// WebKit draws the current frame shrunk for a moment when the loading overlay
// covers a playing video, so the video is hidden while a source loads.
const VIDEO_STYLE_LOADING: React.CSSProperties = { ...VIDEO_STYLE, visibility: "hidden" };

// Rewinding plays the VOD Twitch records during the stream. Too close to its
// end, hls.js has no segment to buffer ahead and stalls.
const VOD_EDGE_MARGIN = 15;
// hls.js treats the VOD as a live playlist and will not load its first
// segments, so a seek before this point waits forever.
const VOD_START_MARGIN = 30;
// A click on the timeline this close to "now" means "stay live".
const LIVE_SNAP = 15;

interface VideoPlayerProps {
  channel: string;
  userInfo: UserInfo | null;
  isFullscreen: boolean;
  setIsFullscreen?: (fullscreen: boolean) => void;
  forceMuted?: boolean;
  compact?: boolean;
  onRequestFocus?: () => void;
  /** Whether the arrow keys seek this player. Only the focused grid tile gets them. */
  hotkeys?: boolean;
}

export function VideoPlayer({
  channel,
  userInfo,
  isFullscreen,
  setIsFullscreen,
  forceMuted = false,
  compact = false,
  onRequestFocus,
  hotkeys = true,
}: VideoPlayerProps) {
  const { t, viewers } = useI18n();
  const videoRef = useRef<HTMLVideoElement>(null);
  const hlsRef = useRef<Hls | null>(null);
  const listenersCleanupRef = useRef<(() => void) | null>(null);

  const [isPaused, setIsPaused] = useState(false);
  const [isMuted, setIsMuted] = useState(getInitialMuted);
  const [volume, setVolume] = useState(getInitialVolume);
  const [isVolumeHovered, setIsVolumeHovered] = useState(false);
  const [qualities, setQualities] = useState<QualityLevel[]>([]);
  const [currentQuality, setCurrentQuality] = useState<number>(AUTO_QUALITY);
  const [showQualityMenu, setShowQualityMenu] = useState(false);
  const [isLoadingStream, setIsLoadingStream] = useState(true);
  // Rewinding or going back to live keeps the last frame on screen while the
  // other playlist loads, instead of a black screen.
  const freezeRef = useRef<HTMLCanvasElement>(null);
  const [frozen, setFrozen] = useState(false);
  const [isBuffering, setIsBuffering] = useState(false);
  const { isActive, markActive, reset: resetIdle } = useIdleTimer(2500);

  const showOverlay = isActive || isPaused || showQualityMenu;

  // Rewind state is tagged with its stream, so the first render after a
  // channel switch or a new broadcast is already back to live.
  const streamKey = `${channel}:${userInfo?.stream?.id ?? ""}`;
  const archiveId = userInfo?.stream?.archiveVideo?.id ?? null;
  const [vodSession, setVodSession] = useState<{ key: string; vodId: string } | null>(null);
  const [vodBlockedKey, setVodBlockedKey] = useState<string | null>(null);
  const vodId = vodSession?.key === streamKey ? vodSession.vodId : null;
  // Without this, coming back to the same broadcast would resume the old rewind.
  useEffect(() => { setVodSession(null); }, [streamKey]);
  const vodTargetRef = useRef(0);
  const vodLoadStoppedRef = useRef(false);
  const [vodPosition, setVodPosition] = useState(0);
  const [vodEdge, setVodEdge] = useState(0);
  const createdAtMs = userInfo?.stream?.createdAt ? Date.parse(userInfo.stream.createdAt) : NaN;
  // A rewound player keeps its controls even if the archive id disappears, or it could not go back to live.
  const showTimeline = !Number.isNaN(createdAtMs) && (!!vodId || (!!archiveId && vodBlockedKey !== streamKey));
  const [nowMs, setNowMs] = useState(Date.now);
  useEffect(() => {
    if (!showTimeline || !showOverlay) return;
    setNowMs(Date.now());
    const id = setInterval(() => setNowMs(Date.now()), 1000);
    return () => clearInterval(id);
  }, [showTimeline, showOverlay]);
  const elapsed = Math.max(0, (nowMs - createdAtMs) / 1000);

  useEffect(() => { setIsLoadingStream(true); setFrozen(false); setIsBuffering(false); }, [channel]);
  useEffect(() => {
    if (userInfo && !userInfo.stream) setIsLoadingStream(false);
  }, [userInfo]);

  // Refs so the async stream-load effect reads the latest preferences
  // without making them deps (which would re-create the HLS instance).
  const volumeRef = useRef(volume);
  const isMutedRef = useRef(isMuted);
  const forceMutedRef = useRef(forceMuted);
  const compactRef = useRef(compact);
  const preferredQualityHeightRef = useRef<number | null>(null);
  if (preferredQualityHeightRef.current === null) {
    preferredQualityHeightRef.current = getInitialPreferredQualityHeight();
  }
  useEffect(() => { volumeRef.current = volume; }, [volume]);
  useEffect(() => { isMutedRef.current = isMuted; }, [isMuted]);
  useEffect(() => { forceMutedRef.current = forceMuted; }, [forceMuted]);
  useEffect(() => { compactRef.current = compact; }, [compact]);

  // In compact (grid) mode the focused tile owns audio unconditionally; the
  // user's persisted mute preference is ignored to avoid stale-state surprises
  // when focus moves. In single-stream mode we honor the preference as before.
  const desiredMuted = () => forceMutedRef.current || (!compactRef.current && isMutedRef.current);

  // Apply a target muted state to the <video> and the React mirror in one step.
  // The DOM `volumechange` event doesn't fire for no-op assignments, so the
  // icon state must be synced manually here.
  const applyMuted = useCallback((wanted: boolean) => {
    const video = videoRef.current;
    if (video && video.muted !== wanted) video.muted = wanted;
    setIsMuted((prev) => prev === wanted ? prev : wanted);
  }, []);

  useEffect(() => {
    applyMuted(desiredMuted());
  }, [forceMuted, compact, applyMuted]);

  useEffect(() => {
    if (!channel || !userInfo?.stream) return;
    let cancelled = false;
    info(`[VideoPlayer] effect fired: channel=${channel} streamId=${userInfo.stream.id}`);

    const key = `${channel}:${userInfo.stream.id}`;

    async function loadStream() {
      try {
        let url: string;
        let startAt = 0;
        if (vodId) {
          const vod: VodPlayback = await invoke("get_vod_playback", { vodId });
          url = vod.url;
          const latest = vod.lengthSeconds > 0 ? vod.lengthSeconds - VOD_EDGE_MARGIN : Infinity;
          startAt = Math.max(VOD_START_MARGIN, Math.min(vodTargetRef.current, latest));
          setVodEdge(vod.lengthSeconds);
          setVodPosition(startAt);
          vodLoadStoppedRef.current = false;
        } else {
          url = await invoke("get_stream_url", { login: channel });
        }
        info(`[VideoPlayer] got url len=${url.length} vod=${vodId ?? "none"}`);

        if (cancelled || !videoRef.current) {
          info(`[VideoPlayer] aborted before HLS init: cancelled=${cancelled} videoRef=${!!videoRef.current}`);
          return;
        }

        if (hlsRef.current) {
          hlsRef.current.destroy();
        }

        // Twitch doesn't speak standard LL-HLS (no EXT-X-PART-INF in the
        // manifest); their low-latency mechanism is the proprietary
        // EXT-X-TWITCH-PREFETCH tag combined with chunked transfer encoding.
        // Setting lowLatencyMode: true here was misleading hls.js into LL-HLS
        // expectations that never matched the wire. The latency win comes from
        // promotePrefetches() in TauriHlsLoader, which starts the fetch of a
        // segment while the encoder is still producing it.
        const baseConfig = {
          loader: TauriHlsLoader,
          enableWorker: true,
          backBufferLength: 30,
          abrEwmaDefaultEstimate: 5_000_000,
        };
        const hls = new Hls(vodId ? {
          ...baseConfig,
          // hls.js treats the VOD's EVENT playlist as live. The default max
          // latency of Infinity is what stops it jumping back to the edge.
          startPosition: startAt,
        } : {
          ...baseConfig,
          liveSyncDuration: 4,
          liveMaxLatencyDuration: 15,
          // Twitch signs every EXT-X-TWITCH-PREFETCH URL separately, so a
          // prefetch we promoted to an EXTINF segment reappears in the next
          // playlist under a different path at the same media sequence
          // number. hls.js reads that as "media sequence mismatch" and raises
          // a non-fatal levelParsingError on every refresh, and that error
          // path returns before arming the live reload timer, so refreshes
          // degrade to error-retry backoff. The flag only silences the event:
          // mapFragmentIntersection still bails at the prefetch tail, so the
          // merge stays incomplete.
          ignorePlaylistParsingErrors: true,
        });
        // Progressive (chunk-by-chunk) segment feeding stays OFF. hls.js'
        // enableStreamingMode() refuses it for custom loaders, and measuring
        // it against Twitch proved that refusal right: Twitch now ships fMP4
        // segments, and feeding them to the transmuxer in pieces produced
        // fragments whose parsed duration was almost never the 2.000s the
        // playlist declares. The mis-timed appends shattered the buffer into
        // up to 9 ranges, and the player then spent its time seeking over
        // holes it had just created. Measured over three matched pairs of
        // 130s runs: progressive on gave 8-20 buffer errors per run and a
        // median 5.9s latency, progressive off gave 0-1 errors, every
        // fragment at the declared 2.000s, and a median 4.4s latency. It is
        // slower, not faster — the stalls cost more than the head start.
        let errorLogCount = 0;
        hls.on(Hls.Events.ERROR, (_event, data) => {
          if (errorLogCount < 6) {
            info(`[VideoPlayer] HLS error type=${data.type} details=${data.details} fatal=${data.fatal}`);
            errorLogCount++;
            if (errorLogCount === 6) info(`[VideoPlayer] (further HLS errors suppressed)`);
          }
          if (data.fatal && vodId && hls.levels.length === 0) {
            // Usher refused the VOD playlist (sub-only or deleted): go back to live.
            // A timeout is not a refusal, so it leaves the timeline for a retry.
            hls.destroy();
            const status = data.response?.code ?? 0;
            if (status >= 400 && status < 500) setVodBlockedKey(key);
            setVodSession(null);
            return;
          }
          if (data.fatal) {
            switch (data.type) {
              case Hls.ErrorTypes.NETWORK_ERROR:
                if (!videoRef.current?.paused) hls.startLoad();
                break;
              case Hls.ErrorTypes.MEDIA_ERROR:
                hls.recoverMediaError();
                break;
              default:
                hls.destroy();
                setIsLoadingStream(false);
                break;
            }
          }
        });

        hls.on(Hls.Events.MANIFEST_PARSED, () => {
          info(`[VideoPlayer] manifest parsed: ${hls.levels.length} levels`);

          const levels = hls.levels.map((level, index) => ({
            id: index,
            label: level.name || `${level.height}p${level.frameRate ? Math.round(level.frameRate) : ""}`,
            height: level.height,
          }));
          levels.sort((a, b) => b.height - a.height);
          setQualities(levels);
          debug(`[VideoPlayer] Available qualities: ${JSON.stringify(levels)}`);

          const preferredHeight = preferredQualityHeightRef.current;
          let initialLevelId: number = AUTO_QUALITY;
          if (preferredHeight !== null && preferredHeight > 0) {
            const match = levels.find(l => l.height === preferredHeight);
            if (match) {
              initialLevelId = match.id;
              hls.currentLevel = match.id;
              debug(`[VideoPlayer] Applied preferred quality: ${match.label}`);
            }
          }
          setCurrentQuality(initialLevelId);

          if (videoRef.current) {
            videoRef.current.volume = volumeRef.current;
            applyMuted(desiredMuted());
            videoRef.current.play().then(
              () => info(`[VideoPlayer] play() resolved (muted=${videoRef.current?.muted})`),
              () => {
                // Browser blocked autoplay with sound — fall back to muted.
                if (videoRef.current) {
                  applyMuted(true);
                  videoRef.current.play().then(
                    () => info(`[VideoPlayer] play() resolved on muted fallback`),
                    e => logError(`[VideoPlayer] Playback failed: ${e}`),
                  );
                }
              },
            );
          }
        });

        const video = videoRef.current;
        const onPlaying = () => info(`[VideoPlayer] <video> playing readyState=${video.readyState}`);
        const onWaiting = () => info(`[VideoPlayer] <video> waiting readyState=${video.readyState}`);
        const onStalled = () => info(`[VideoPlayer] <video> stalled`);

        const onVideoError = () => info(`[VideoPlayer] <video> error code=${video.error?.code} msg=${video.error?.message}`);
        // First decoded frame — that's when the black screen actually ends.
        // MANIFEST_PARSED is too early: the spinner disappears but no frame
        // has been demuxed and decoded yet.
        const onLoadedData = () => {
          setIsLoadingStream(false);
          setFrozen(false);
          // Rewinds pressed while the VOD was loading moved the target further back.
          if (vodId && vodTargetRef.current < video.currentTime - 1) video.currentTime = vodTargetRef.current;
        };
        video.addEventListener("playing", onPlaying);
        video.addEventListener("waiting", onWaiting);
        video.addEventListener("stalled", onStalled);
        video.addEventListener("error", onVideoError);
        video.addEventListener("loadeddata", onLoadedData);
        // Cleanup: hls destroy already in outer return, listeners are attached
        // to a video element that React keeps mounted across pin cycles, so
        // remove them when the loader effect re-runs.
        const cleanupListeners = () => {
          video.removeEventListener("playing", onPlaying);
          video.removeEventListener("waiting", onWaiting);
          video.removeEventListener("stalled", onStalled);
          video.removeEventListener("error", onVideoError);
          video.removeEventListener("loadeddata", onLoadedData);
        };
        // Stash the cleanup so the outer effect's return can call it.
        listenersCleanupRef.current = cleanupListeners;

        hls.on(Hls.Events.LEVEL_SWITCHED, (_event, data) => {
          debug(`[VideoPlayer] Quality switched to level: ${data.level}`);
        });
        if (vodId) {
          hls.on(Hls.Events.LEVEL_LOADED, (_event, data) => setVodEdge(data.details.edge));
        }

        hls.loadSource(url);
        hls.attachMedia(videoRef.current);
        hlsRef.current = hls;
      } catch (err) {
        logError(`[VideoPlayer] Failed to load stream: ${err}`);
        if (cancelled) return;
        if (vodId) {
          // Sub-only or missing VOD: hide the timeline. Any other error only goes back to live.
          if (/restricted|No VOD token/.test(String(err))) setVodBlockedKey(key);
          setVodSession(null);
          return;
        }
        setIsLoadingStream(false);
      }
    }

    loadStream();

    return () => {
      cancelled = true;
      if (hlsRef.current) {
        hlsRef.current.destroy();
        hlsRef.current = null;
      }
      if (listenersCleanupRef.current) {
        listenersCleanupRef.current();
        listenersCleanupRef.current = null;
      }
    };
  }, [channel, userInfo?.stream?.id, vodId, setIsLoadingStream]);

  const changeQuality = useCallback((levelId: number) => {
    const hls = hlsRef.current;
    if (!hls) return;

    if (levelId === AUTO_QUALITY) {
      hls.currentLevel = AUTO_QUALITY;
      preferredQualityHeightRef.current = AUTO_QUALITY;
      persistPreferredQualityHeight(AUTO_QUALITY);
      debug("[VideoPlayer] Quality set to auto");
    } else {
      const quality = qualities.find(q => q.id === levelId);
      if (quality) {
        hls.currentLevel = levelId;
        preferredQualityHeightRef.current = quality.height;
        persistPreferredQualityHeight(quality.height);
        debug(`[VideoPlayer] Quality set to: ${quality.label}`);
      }
    }
    setCurrentQuality(levelId);
    setShowQualityMenu(false);
  }, [qualities]);

  const togglePlayPause = useCallback(() => {
    const video = videoRef.current;
    if (video) {
      if (video.paused) video.play();
      else video.pause();
    }
  }, []);

  const toggleMute = useCallback(() => {
    const video = videoRef.current;
    if (!video) return;
    if (forceMutedRef.current) return;
    const next = !video.muted;
    video.muted = next;
    if (!compact) persistMuted(next);
  }, [compact]);

  const handleVolumeChange = useCallback((newVolume: number) => {
    const video = videoRef.current;
    if (!video) return;
    if (forceMutedRef.current) return;
    video.volume = newVolume;
    const muted = newVolume === 0;
    video.muted = muted;
    if (!compact) {
      persistVolume(newVolume);
      if (muted !== isMutedRef.current) {
        persistMuted(muted);
      }
    }
  }, [compact]);

  const freezeFrame = useCallback(() => {
    const video = videoRef.current;
    const canvas = freezeRef.current;
    if (!video || !canvas || video.readyState < 2 || !video.videoWidth) return;
    canvas.width = video.videoWidth;
    canvas.height = video.videoHeight;
    canvas.getContext("2d")?.drawImage(video, 0, 0);
    setFrozen(true);
  }, []);

  const goLive = useCallback(() => {
    freezeFrame();
    setVodSession(null);
    setIsLoadingStream(true);
  }, [freezeFrame]);

  // `target` is in seconds from stream start, which is also the VOD's own time.
  const seekStream = (target: number) => {
    if (!vodId) {
      if (!archiveId) return;
      vodTargetRef.current = Math.max(VOD_START_MARGIN, target);
      setVodPosition(vodTargetRef.current);
      freezeFrame();
      setVodSession({ key: streamKey, vodId: archiveId });
      setIsLoadingStream(true);
      return;
    }
    // vodEdge is stale until the VOD playlist loads, so it cannot end a rewind yet.
    if (!isLoadingStream && target >= vodEdge - VOD_EDGE_MARGIN) {
      goLive();
      return;
    }
    vodTargetRef.current = Math.max(VOD_START_MARGIN, target);
    setVodPosition(vodTargetRef.current);
    const video = videoRef.current;
    // Until the first frame, currentTime is 0; the target is applied on loadeddata.
    if (!video || isLoadingStream) return;
    video.currentTime = vodTargetRef.current;
    setVodPosition(video.currentTime);
    // Loading stops on pause; load the new position so its frame shows.
    if (video.paused && hlsRef.current) {
      vodLoadStoppedRef.current = false;
      hlsRef.current.startLoad(video.currentTime);
    }
  };

  const skip = (seconds: number) => {
    const from = !vodId ? (Date.now() - createdAtMs) / 1000
      : isLoadingStream ? vodTargetRef.current
      : videoRef.current?.currentTime ?? vodPosition;
    seekStream(from + seconds);
  };

  const skipRef = useRef(skip);
  skipRef.current = skip;
  const canSeekRef = useRef(false);
  canSeekRef.current = showTimeline;
  const isRewoundRef = useRef(false);
  isRewoundRef.current = !!vodId;
  const markActiveRef = useRef(markActive);
  markActiveRef.current = markActive;

  useEffect(() => {
    if (!hotkeys) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
      if (e.defaultPrevented || e.metaKey || e.ctrlKey || e.altKey || e.shiftKey) return;
      // Leave the arrows to text fields and the volume slider.
      const el = e.target as HTMLElement | null;
      if (el && (el.isContentEditable || el.closest("input, textarea, select, [role=slider]"))) return;
      if (!canSeekRef.current) return;
      // Live is already the furthest point, so only a rewound player moves forward.
      if (e.key === "ArrowRight" && !isRewoundRef.current) return;
      e.preventDefault();
      markActiveRef.current();
      skipRef.current(e.key === "ArrowLeft" ? -10 : 10);
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [hotkeys]);

  const volumePercent = Math.round((isMuted ? 0 : volume) * 100);

  if (userInfo && !userInfo.stream && !isLoadingStream) {
    return (
      <div className="flex-1 relative bg-black min-h-0 min-w-0 overflow-hidden">
        <div className="absolute inset-0 flex items-center justify-center bg-base">
          <div className="flex flex-col items-center gap-4 text-center">
            {userInfo?.profileImageURL && (
              <img
                src={userInfo.profileImageURL}
                alt={userInfo.displayName}
                className="w-24 h-24 rounded-full border-4 border-border"
              />
            )}
            <div>
              <h2 className="text-xl font-bold text-white">{userInfo?.displayName || channel}</h2>
              <p className="text-muted mt-1">{t("channel.offlineLong")}</p>
            </div>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div
      className={cn(
        "flex-1 relative bg-black min-h-0 min-w-0 overflow-hidden",
        !showOverlay && "cursor-none"
      )}
      onMouseMove={markActive}
      onMouseLeave={resetIdle}
    >
      <video
        ref={videoRef}
        style={isLoadingStream ? VIDEO_STYLE_LOADING : VIDEO_STYLE}
        className="w-full h-full object-contain shadow-2xl"
        autoPlay
        playsInline
        onClick={() => {
          onRequestFocus?.();
          applyMuted(desiredMuted());
        }}
        onPlay={() => {
          setIsPaused(false);
          if (!vodId) {
            hlsRef.current?.startLoad(-1);
          } else if (vodLoadStoppedRef.current) {
            // Only after our own stopLoad: on the first play no fragment is
            // loaded yet, and startLoad(-1) would skip the rewind target.
            vodLoadStoppedRef.current = false;
            hlsRef.current?.startLoad(-1);
          }
        }}
        onWaiting={() => setIsBuffering(true)}
        onPlaying={() => setIsBuffering(false)}
        onCanPlay={() => setIsBuffering(false)}
        onPause={() => {
          setIsPaused(true);
          setIsBuffering(false);
          hlsRef.current?.stopLoad();
          if (vodId) vodLoadStoppedRef.current = true;
        }}
        onTimeUpdate={vodId && showOverlay ? (e) => setVodPosition(Math.floor(e.currentTarget.currentTime)) : undefined}
        onVolumeChange={(e) => {
          const video = e.currentTarget;
          setIsMuted(video.muted);
          setVolume(video.volume);
        }}
        onDoubleClick={() => setIsFullscreen?.(!isFullscreen)}
      />

      <canvas
        ref={freezeRef}
        className={cn(
          "absolute inset-0 w-full h-full object-contain pointer-events-none",
          !(frozen && isLoadingStream) && "hidden"
        )}
      />

      {/* Loading overlay */}
      {isLoadingStream && !frozen ? (
        <div className="absolute inset-0 flex items-center justify-center bg-black/50">
          <div className="flex flex-col items-center gap-3">
            <Loader2 className="w-10 h-10 text-twitch animate-spin" />
            <span className="text-white text-sm">{t("player.loading")}</span>
          </div>
        </div>
      ) : (isLoadingStream || isBuffering) && (
        <div className="absolute inset-0 flex items-center justify-center pointer-events-none">
          <Loader2 className="w-10 h-10 text-twitch animate-spin drop-shadow-lg" />
        </div>
      )}

      {userInfo?.stream && (
        <div className={cn(
          "absolute top-4 left-4 flex items-center gap-2 transition-opacity",
          showOverlay ? "opacity-100" : "opacity-0 pointer-events-none"
        )}>
          <div className={cn(
            "text-white text-xs font-bold px-2 py-1 rounded flex items-center gap-1",
            vodId ? "bg-black/70" : "bg-red-600"
          )}>
            <div className={cn("w-2 h-2 bg-white rounded-full", vodId ? "opacity-50" : "animate-pulse")} />
            {t("player.live")}
          </div>
          <div className="bg-black/70 text-white text-xs px-2 py-1 rounded">
            {viewers(userInfo.stream.viewersCount)}
          </div>
        </div>
      )}

      {/* Video Controls Overlay */}
      <div className={cn(
        "absolute bottom-0 left-0 right-0 bg-gradient-to-t from-black/80 to-transparent p-2 transition-opacity",
        showOverlay ? "opacity-100" : "opacity-0 pointer-events-none"
      )}>
        {showTimeline && (
          <div className="px-2">
            <SeekBar
              duration={elapsed}
              position={vodId ? vodPosition : elapsed}
              onSeek={(target) => {
                if (vodId || target < elapsed - LIVE_SNAP) seekStream(target);
              }}
            />
          </div>
        )}
        <div className="flex items-center gap-1">
          {/* Play/Pause Button */}
          <button onClick={togglePlayPause} className="p-2 hover:bg-white/20 rounded transition-colors">
            {isPaused ? (
              <Play className="w-5 h-5 text-white fill-white" />
            ) : (
              <Pause className="w-5 h-5 text-white fill-white" />
            )}
          </button>

          {/* Volume Control */}
          <div
            className="relative flex items-center"
            onMouseEnter={() => setIsVolumeHovered(true)}
            onMouseLeave={() => setIsVolumeHovered(false)}
          >
            {isVolumeHovered && (
              <div className="absolute bottom-full left-0 mb-2 px-2 py-1 rounded bg-elevated border border-border text-white text-xs whitespace-nowrap shadow-lg pointer-events-none">
                {forceMuted
                  ? t("player.mutedUnfocused")
                  : isMuted
                    ? t("player.muted")
                    : t("player.volume", { percent: volumePercent })}
              </div>
            )}

            <button
              onClick={toggleMute}
              disabled={forceMuted}
              className="p-2 hover:bg-white/20 rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {isMuted || volume === 0 ? (
                <VolumeX className="w-5 h-5 text-white" />
              ) : (
                <Volume2 className="w-5 h-5 text-white" />
              )}
            </button>

            {/* py-2 keeps the 12px thumb inside the clip box while the width animates */}
            <div
              className={cn(
                "flex items-center overflow-hidden py-2 transition-all duration-200",
                isVolumeHovered && !forceMuted ? "w-20 opacity-100" : "w-0 opacity-0"
              )}
            >
              <input
                type="range"
                min="0"
                max="1"
                step="0.05"
                value={isMuted ? 0 : volume}
                disabled={forceMuted}
                onChange={(e) => handleVolumeChange(parseFloat(e.target.value))}
                className="w-full h-1 bg-white/30 rounded-full appearance-none cursor-pointer disabled:cursor-not-allowed disabled:opacity-50 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:w-3 [&::-webkit-slider-thumb]:h-3 [&::-webkit-slider-thumb]:bg-white [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:cursor-pointer"
              />
            </div>
          </div>

          {showTimeline && (
            <>
              {[30, 10].map((seconds) => (
                <button
                  key={seconds}
                  onClick={() => skip(-seconds)}
                  title={t("player.rewind", { seconds })}
                  className="p-2 hover:bg-white/20 rounded transition-colors flex items-center gap-0.5 text-white text-xs"
                >
                  <RotateCcw className="w-4 h-4" />
                  {seconds}
                </button>
              ))}
              {vodId && (
                <>
                  <button
                    onClick={() => skip(10)}
                    title={t("player.forward", { seconds: 10 })}
                    className="p-2 hover:bg-white/20 rounded transition-colors flex items-center gap-0.5 text-white text-xs"
                  >
                    10
                    <RotateCw className="w-4 h-4" />
                  </button>
                  <span className="px-1 text-white text-xs tabular-nums">{formatBehind(elapsed - vodPosition)}</span>
                  <button
                    onClick={goLive}
                    title={t("player.backToLive")}
                    className="px-2 py-1 hover:bg-white/20 rounded transition-colors flex items-center gap-1 text-white text-xs font-bold"
                  >
                    <div className="w-2 h-2 bg-white/50 rounded-full" />
                    {t("player.live")}
                  </button>
                </>
              )}
            </>
          )}

          <div className="flex-1" />

          {/* Quality Selector */}
          <div className="relative">
            <button
              onClick={() => setShowQualityMenu(q => !q)}
              className="p-2 hover:bg-white/20 rounded transition-colors flex items-center gap-1"
            >
              <Settings className="w-5 h-5 text-white" />
              <span className="text-white text-xs">
                {currentQuality === AUTO_QUALITY ? t("player.auto") : qualities.find(q => q.id === currentQuality)?.label || t("player.auto")}
              </span>
            </button>

            {showQualityMenu && (
              <>
                <div className="fixed inset-0 z-10" onClick={() => setShowQualityMenu(false)} />
                <div className="absolute bottom-full right-0 mb-2 bg-elevated border border-border rounded-lg shadow-xl overflow-hidden min-w-[140px] z-20">
                  <div className="p-2 border-b border-border text-xs text-muted font-semibold">
                    {t("player.quality")}
                  </div>
                  <div className="max-h-64 overflow-y-auto scrollbar-thin">
                    <button
                      onClick={() => changeQuality(AUTO_QUALITY)}
                      className={cn(
                        "w-full px-3 py-2 text-left text-sm hover:bg-hover flex items-center justify-between",
                        currentQuality === AUTO_QUALITY && "text-twitch"
                      )}
                    >
                      {t("player.auto")}
                      {currentQuality === AUTO_QUALITY && <span className="text-xs">✓</span>}
                    </button>
                    {qualities.map((q) => (
                      <button
                        key={q.id}
                        onClick={() => changeQuality(q.id)}
                        className={cn(
                          "w-full px-3 py-2 text-left text-sm hover:bg-hover flex items-center justify-between",
                          currentQuality === q.id && "text-twitch"
                        )}
                      >
                        {q.label}
                        {currentQuality === q.id && <span className="text-xs">✓</span>}
                      </button>
                    ))}
                  </div>
                </div>
              </>
            )}
          </div>

          {setIsFullscreen && (
            <button
              onClick={() => setIsFullscreen(!isFullscreen)}
              className="p-2 hover:bg-white/20 rounded transition-colors"
            >
              {isFullscreen ? (
                <Minimize className="w-5 h-5 text-white" />
              ) : (
                <Maximize className="w-5 h-5 text-white" />
              )}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
