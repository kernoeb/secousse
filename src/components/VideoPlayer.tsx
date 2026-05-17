import { useRef, useState, useEffect, useCallback } from "react";
import Hls from "hls.js";
import { invoke } from "@tauri-apps/api/core";
import { debug, info, error as logError } from "@tauri-apps/plugin-log";
import { Play, Pause, Volume2, VolumeX, Settings, Maximize, Minimize, Loader2 } from "lucide-react";
import { TauriHlsLoader } from "../TauriHlsLoader";
import { useIdleTimer } from "../hooks";
import {
  cn,
  formatViewers,
  AUTO_QUALITY,
  getInitialVolume,
  persistVolume,
  getInitialMuted,
  persistMuted,
  getInitialPreferredQualityHeight,
  persistPreferredQualityHeight,
} from "../lib/utils";
import type { UserInfo, QualityLevel } from "../types";

const VIDEO_STYLE: React.CSSProperties = { transform: "translateZ(0)" };

interface VideoPlayerProps {
  channel: string;
  userInfo: UserInfo | null;
  isFullscreen: boolean;
  setIsFullscreen?: (fullscreen: boolean) => void;
  forceMuted?: boolean;
  compact?: boolean;
  onRequestFocus?: () => void;
}

export function VideoPlayer({
  channel,
  userInfo,
  isFullscreen,
  setIsFullscreen,
  forceMuted = false,
  compact = false,
  onRequestFocus,
}: VideoPlayerProps) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const hlsRef = useRef<Hls | null>(null);
  const listenersCleanupRef = useRef<(() => void) | null>(null);

  const [isPaused, setIsPaused] = useState(false);
  const [isMuted, setIsMuted] = useState(getInitialMuted);
  const [volume, setVolume] = useState(getInitialVolume);
  const [showVolumeSlider, setShowVolumeSlider] = useState(false);
  const [qualities, setQualities] = useState<QualityLevel[]>([]);
  const [currentQuality, setCurrentQuality] = useState<number>(AUTO_QUALITY);
  const [showQualityMenu, setShowQualityMenu] = useState(false);
  const [isLoadingStream, setIsLoadingStream] = useState(true);
  const { isActive, markActive, reset: resetIdle } = useIdleTimer(2500);

  const showOverlay = isActive || isPaused || showQualityMenu;

  useEffect(() => { setIsLoadingStream(true); }, [channel]);
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

    async function loadStream() {
      try {
        const url: string = await invoke("get_stream_url", { login: channel });
        info(`[VideoPlayer] got url len=${url.length}`);

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
        // expectations that never matched the wire. We rely instead on
        // TauriHlsLoader's streaming path (Phase 1) — the transmuxer parses
        // bytes as they arrive on past EXTINF segments too. Phase 2 (playlist
        // rewrite of EXT-X-TWITCH-PREFETCH) brings actual sub-segment latency.
        const hls = new Hls({
          loader: TauriHlsLoader,
          enableWorker: true,
          backBufferLength: 30,
          liveSyncDuration: 4,
          liveMaxLatencyDuration: 15,
          abrEwmaDefaultEstimate: 5_000_000,
        });
        // hls.js' enableStreamingMode() flips progressive back to false when
        // a custom loader is registered (config.ts safety guard). Set it
        // after construction so base-stream-controller passes a progress
        // callback to fragmentLoader.load(), letting our chunks reach the
        // transmuxer instead of being held until onSuccess.
        hls.config.progressive = true;

        let errorLogCount = 0;
        hls.on(Hls.Events.ERROR, (_event, data) => {
          if (errorLogCount < 6) {
            info(`[VideoPlayer] HLS error type=${data.type} details=${data.details} fatal=${data.fatal}`);
            errorLogCount++;
            if (errorLogCount === 6) info(`[VideoPlayer] (further HLS errors suppressed)`);
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
        // MANIFEST_PARSED is too early with the progressive loader: the loader
        // disappears but no frame has been demuxed/decoded yet.
        const onLoadedData = () => setIsLoadingStream(false);
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

        hls.loadSource(url);
        hls.attachMedia(videoRef.current);
        hlsRef.current = hls;
      } catch (err) {
        logError(`[VideoPlayer] Failed to load stream: ${err}`);
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
  }, [channel, userInfo?.stream?.id, setIsLoadingStream]);

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
              <p className="text-muted mt-1">Channel is currently offline</p>
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
        style={VIDEO_STYLE}
        className="w-full h-full object-contain shadow-2xl"
        autoPlay
        playsInline
        onClick={() => {
          onRequestFocus?.();
          applyMuted(desiredMuted());
        }}
        onPlay={() => {
          setIsPaused(false);
          hlsRef.current?.startLoad(-1);
        }}
        onPause={() => {
          setIsPaused(true);
          hlsRef.current?.stopLoad();
        }}
        onVolumeChange={(e) => {
          const video = e.currentTarget;
          setIsMuted(video.muted);
          setVolume(video.volume);
        }}
        onDoubleClick={() => setIsFullscreen?.(!isFullscreen)}
      />

      {/* Loading overlay */}
      {isLoadingStream && (
        <div className="absolute inset-0 flex items-center justify-center bg-black/50">
          <div className="flex flex-col items-center gap-3">
            <Loader2 className="w-10 h-10 text-twitch animate-spin" />
            <span className="text-white text-sm">Loading stream...</span>
          </div>
        </div>
      )}

      {userInfo?.stream && (
        <div className={cn(
          "absolute top-4 left-4 flex items-center gap-2 transition-opacity",
          showOverlay ? "opacity-100" : "opacity-0 pointer-events-none"
        )}>
          <div className="bg-red-600 text-white text-xs font-bold px-2 py-1 rounded flex items-center gap-1">
            <div className="w-2 h-2 bg-white rounded-full animate-pulse" />
            LIVE
          </div>
          <div className="bg-black/70 text-white text-xs px-2 py-1 rounded">
            {formatViewers(userInfo.stream.viewersCount)} viewers
          </div>
        </div>
      )}

      {/* Video Controls Overlay */}
      <div className={cn(
        "absolute bottom-0 left-0 right-0 bg-gradient-to-t from-black/80 to-transparent p-2 transition-opacity",
        showOverlay ? "opacity-100" : "opacity-0 pointer-events-none"
      )}>
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
            onMouseEnter={() => !forceMuted && setShowVolumeSlider(true)}
            onMouseLeave={() => setShowVolumeSlider(false)}
          >
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

            <div
              className={cn(
                "flex items-center overflow-hidden transition-all duration-200",
                showVolumeSlider ? "w-20 opacity-100" : "w-0 opacity-0"
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

          <div className="flex-1" />

          {/* Quality Selector */}
          <div className="relative">
            <button
              onClick={() => setShowQualityMenu(q => !q)}
              className="p-2 hover:bg-white/20 rounded transition-colors flex items-center gap-1"
            >
              <Settings className="w-5 h-5 text-white" />
              <span className="text-white text-xs">
                {currentQuality === AUTO_QUALITY ? "Auto" : qualities.find(q => q.id === currentQuality)?.label || "Auto"}
              </span>
            </button>

            {showQualityMenu && (
              <>
                <div className="fixed inset-0 z-10" onClick={() => setShowQualityMenu(false)} />
                <div className="absolute bottom-full right-0 mb-2 bg-elevated border border-border rounded-lg shadow-xl overflow-hidden min-w-[140px] z-20">
                  <div className="p-2 border-b border-border text-xs text-muted font-semibold">
                    Quality
                  </div>
                  <div className="max-h-64 overflow-y-auto scrollbar-thin">
                    <button
                      onClick={() => changeQuality(AUTO_QUALITY)}
                      className={cn(
                        "w-full px-3 py-2 text-left text-sm hover:bg-hover flex items-center justify-between",
                        currentQuality === AUTO_QUALITY && "text-twitch"
                      )}
                    >
                      Auto
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
