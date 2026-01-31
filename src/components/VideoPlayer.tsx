import { useRef, useState, useEffect, useCallback } from "react";
import Hls from "hls.js";
import { invoke } from "@tauri-apps/api/core";
import { debug, error as logError } from "@tauri-apps/plugin-log";
import { Play, Pause, Volume2, VolumeX, Settings, Maximize, Minimize, Loader2 } from "lucide-react";
import { TauriHlsLoader } from "../TauriHlsLoader";
import { cn, formatViewers } from "../lib/utils";
import type { UserInfo, QualityLevel } from "../types";

interface VideoPlayerProps {
  channel: string;
  userInfo: UserInfo | null;
  isLoadingStream: boolean;
  setIsLoadingStream: (loading: boolean) => void;
  isFullscreen: boolean;
  setIsFullscreen: (fullscreen: boolean) => void;
}

export function VideoPlayer({
  channel,
  userInfo,
  isLoadingStream,
  setIsLoadingStream,
  isFullscreen,
  setIsFullscreen,
}: VideoPlayerProps) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const hlsRef = useRef<Hls | null>(null);
  
  const [isPaused, setIsPaused] = useState(false);
  const [isMuted, setIsMuted] = useState(false);
  const [volume, setVolume] = useState(1);
  const [showVolumeSlider, setShowVolumeSlider] = useState(false);
  const [qualities, setQualities] = useState<QualityLevel[]>([]);
  const [currentQuality, setCurrentQuality] = useState<number>(-1);
  const [showQualityMenu, setShowQualityMenu] = useState(false);

  // Load stream when channel changes
  useEffect(() => {
    if (!channel || !userInfo?.stream) return;

    async function loadStream() {
      try {
        const url: string = await invoke("get_stream_url", { login: channel });
        
        if (!videoRef.current) return;
        
        if (hlsRef.current) {
          hlsRef.current.destroy();
        }

        const hls = new Hls({
          lowLatencyMode: true,
          loader: TauriHlsLoader,
          enableWorker: true,
          backBufferLength: 60,
        });

        hls.on(Hls.Events.ERROR, (_event, data) => {
          if (data.fatal) {
            switch (data.type) {
              case Hls.ErrorTypes.NETWORK_ERROR:
                hls.startLoad();
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
          setIsLoadingStream(false);
          
          const levels = hls.levels.map((level, index) => ({
            id: index,
            label: level.name || `${level.height}p${level.frameRate ? Math.round(level.frameRate) : ""}`,
            height: level.height,
          }));
          levels.sort((a, b) => b.height - a.height);
          setQualities(levels);
          setCurrentQuality(-1);
          debug(`[VideoPlayer] Available qualities: ${JSON.stringify(levels)}`);

          if (videoRef.current) {
            videoRef.current.muted = false;
            videoRef.current.play().catch(() => {
              if (videoRef.current) {
                videoRef.current.muted = true;
                videoRef.current.play().catch(e => logError(`[VideoPlayer] Playback failed: ${e}`));
              }
            });
          }
        });

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
      if (hlsRef.current) {
        hlsRef.current.destroy();
        hlsRef.current = null;
      }
    };
  }, [channel, userInfo?.stream?.id, setIsLoadingStream]);

  const changeQuality = useCallback((levelId: number) => {
    const hls = hlsRef.current;
    if (!hls) return;

    if (levelId === -1) {
      hls.currentLevel = -1;
      debug("[VideoPlayer] Quality set to auto");
    } else {
      const quality = qualities.find(q => q.id === levelId);
      if (quality) {
        hls.currentLevel = levelId;
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
    if (video) {
      video.muted = !video.muted;
    }
  }, []);

  const handleVolumeChange = useCallback((newVolume: number) => {
    const video = videoRef.current;
    if (video) {
      video.volume = newVolume;
      video.muted = newVolume === 0;
    }
  }, []);

  // Show offline screen if channel exists but is not live
  if (!userInfo?.stream && !isLoadingStream) {
    return (
      <div className="flex-1 relative bg-black">
        <div className="absolute inset-0 flex items-center justify-center bg-[#18181b]">
          <div className="flex flex-col items-center gap-4 text-center">
            {userInfo?.profileImageURL && (
              <img
                src={userInfo.profileImageURL}
                alt={userInfo.displayName}
                className="w-24 h-24 rounded-full border-4 border-[#3f3f46]"
              />
            )}
            <div>
              <h2 className="text-xl font-bold text-white">{userInfo?.displayName || channel}</h2>
              <p className="text-[#adadb8] mt-1">Channel is currently offline</p>
            </div>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div
      className={cn(
        "flex-1 relative bg-black group",
        isFullscreen && "fixed inset-0 z-50"
      )}
    >
      <video
        ref={videoRef}
        className="w-full h-full object-contain shadow-2xl"
        autoPlay
        playsInline
        onPlay={() => setIsPaused(false)}
        onPause={() => setIsPaused(true)}
        onVolumeChange={(e) => {
          const video = e.currentTarget;
          setIsMuted(video.muted);
          setVolume(video.volume);
        }}
        onDoubleClick={() => setIsFullscreen(!isFullscreen)}
      />

      {/* Loading overlay */}
      {isLoadingStream && (
        <div className="absolute inset-0 flex items-center justify-center bg-black/50">
          <div className="flex flex-col items-center gap-3">
            <Loader2 className="w-10 h-10 text-[#9146ff] animate-spin" />
            <span className="text-white text-sm">Loading stream...</span>
          </div>
        </div>
      )}

      {/* Live indicator */}
      {userInfo?.stream && (
        <div className="absolute top-4 left-4 flex items-center gap-2">
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
      <div className="absolute bottom-0 left-0 right-0 bg-gradient-to-t from-black/80 to-transparent p-2 opacity-0 group-hover:opacity-100 transition-opacity">
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
            onMouseEnter={() => setShowVolumeSlider(true)}
            onMouseLeave={() => setShowVolumeSlider(false)}
          >
            <button onClick={toggleMute} className="p-2 hover:bg-white/20 rounded transition-colors">
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
                onChange={(e) => handleVolumeChange(parseFloat(e.target.value))}
                className="w-full h-1 bg-white/30 rounded-full appearance-none cursor-pointer [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:w-3 [&::-webkit-slider-thumb]:h-3 [&::-webkit-slider-thumb]:bg-white [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:cursor-pointer"
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
                {currentQuality === -1 ? "Auto" : qualities.find(q => q.id === currentQuality)?.label || "Auto"}
              </span>
            </button>

            {showQualityMenu && (
              <>
                <div className="fixed inset-0 z-10" onClick={() => setShowQualityMenu(false)} />
                <div className="absolute bottom-full right-0 mb-2 bg-[#18181b] border border-[#3f3f46] rounded-lg shadow-xl overflow-hidden min-w-[140px] z-20">
                  <div className="p-2 border-b border-[#3f3f46] text-xs text-[#adadb8] font-semibold">
                    Quality
                  </div>
                  <div className="max-h-64 overflow-y-auto">
                    <button
                      onClick={() => changeQuality(-1)}
                      className={cn(
                        "w-full px-3 py-2 text-left text-sm hover:bg-[#2f2f35] flex items-center justify-between",
                        currentQuality === -1 && "text-[#9146ff]"
                      )}
                    >
                      Auto
                      {currentQuality === -1 && <span className="text-xs">✓</span>}
                    </button>
                    {qualities.map((q) => (
                      <button
                        key={q.id}
                        onClick={() => changeQuality(q.id)}
                        className={cn(
                          "w-full px-3 py-2 text-left text-sm hover:bg-[#2f2f35] flex items-center justify-between",
                          currentQuality === q.id && "text-[#9146ff]"
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

          {/* Fullscreen Button */}
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
        </div>
      </div>
    </div>
  );
}
