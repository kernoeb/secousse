import { useRef, useState } from "react";
import { useI18n } from "../i18n";
import { formatUptime } from "../lib/utils";

interface SeekBarProps {
  /** Seconds from stream start to now. */
  duration: number;
  /** Seconds from stream start; equal to `duration` while live. */
  position: number;
  onSeek: (seconds: number) => void;
}

export function formatBehind(seconds: number): string {
  return `-${formatUptime(0, Math.max(0, seconds) * 1000)}`;
}

/** Timeline from stream start to now. The seek fires on release, not while dragging. */
export function SeekBar({ duration, position, onSeek }: SeekBarProps) {
  const { t } = useI18n();
  const barRef = useRef<HTMLDivElement>(null);
  const [hover, setHover] = useState<number | null>(null);
  const [drag, setDrag] = useState<number | null>(null);

  const fractionAt = (clientX: number) => {
    const rect = barRef.current?.getBoundingClientRect();
    if (!rect || rect.width === 0) return 1;
    return Math.min(1, Math.max(0, (clientX - rect.left) / rect.width));
  };

  const fraction = drag ?? (duration > 0 ? Math.min(1, position / duration) : 1);
  const tip = drag ?? hover;

  return (
    <div
      ref={barRef}
      role="slider"
      aria-label={t("player.timeline")}
      aria-valuemin={0}
      aria-valuemax={Math.round(duration)}
      aria-valuenow={Math.round(position)}
      className="group relative h-4 flex items-center cursor-pointer touch-none"
      onPointerDown={(e) => {
        e.currentTarget.setPointerCapture(e.pointerId);
        setDrag(fractionAt(e.clientX));
      }}
      onPointerMove={(e) => {
        const f = fractionAt(e.clientX);
        setHover(f);
        if (drag !== null) setDrag(f);
      }}
      onPointerUp={() => {
        if (drag !== null) onSeek(drag * duration);
        setDrag(null);
      }}
      onPointerCancel={() => setDrag(null)}
      onPointerLeave={() => setHover(null)}
    >
      <div className="relative w-full h-1 group-hover:h-1.5 bg-white/30 rounded-full transition-all">
        <div className="absolute inset-y-0 left-0 bg-twitch rounded-full" style={{ width: `${fraction * 100}%` }} />
        <div
          className="absolute top-1/2 w-3 h-3 -translate-x-1/2 -translate-y-1/2 bg-white rounded-full opacity-0 group-hover:opacity-100 transition-opacity"
          style={{ left: `${fraction * 100}%` }}
        />
      </div>
      {tip !== null && (
        <div
          className="absolute bottom-full mb-1 -translate-x-1/2 px-2 py-1 rounded bg-elevated border border-border text-white text-xs whitespace-nowrap shadow-lg pointer-events-none"
          style={{ left: `clamp(2.5rem, ${tip * 100}%, calc(100% - 2.5rem))` }}
        >
          {formatBehind(duration * (1 - tip))}
        </div>
      )}
    </div>
  );
}
