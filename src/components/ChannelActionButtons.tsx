import { Plus, ExternalLink, X } from "lucide-react";
import { cn } from "../lib/utils";

type Variant = "row" | "card" | "overlay";

interface ChannelActionButtonsProps {
  variant: Variant;
  canAddToGrid: boolean;
  isInGrid?: boolean;
  onAddToGrid?: () => void;
  onOpenPopout: () => void;
  onRemove?: () => void;
  className?: string;
}

interface VariantStyle {
  wrapper: string;
  button: string;
  iconSize: string;
  addAccent: string;
  popoutAccent: string;
}

const STYLES: Record<Variant, VariantStyle> = {
  row: {
    wrapper: "bg-surface/80 backdrop-blur-sm rounded",
    button: "p-1 hover:bg-hover rounded",
    iconSize: "w-3.5 h-3.5",
    addAccent: "text-twitch",
    popoutAccent: "text-muted hover:text-white",
  },
  card: {
    wrapper: "",
    button: "p-1.5 bg-black/80 hover:bg-black rounded text-white transition-colors",
    iconSize: "w-4 h-4",
    addAccent: "hover:bg-twitch",
    popoutAccent: "",
  },
  overlay: {
    wrapper: "",
    button: "p-1.5 bg-black/70 hover:bg-black/90 rounded text-white transition-colors",
    iconSize: "w-3.5 h-3.5",
    addAccent: "",
    popoutAccent: "",
  },
};

export function ChannelActionButtons({
  variant,
  canAddToGrid,
  isInGrid = false,
  onAddToGrid,
  onOpenPopout,
  onRemove,
  className,
}: ChannelActionButtonsProps) {
  const styles = STYLES[variant];
  const showAdd = !!onAddToGrid && canAddToGrid && !isInGrid;

  const stop = (fn: () => void) => (e: React.MouseEvent) => {
    e.stopPropagation();
    fn();
  };

  return (
    <div className={cn("flex items-center gap-1", styles.wrapper, className)}>
      {showAdd && (
        <button
          onClick={stop(onAddToGrid!)}
          className={cn(styles.button, styles.addAccent)}
          title="Add to grid"
        >
          <Plus className={styles.iconSize} />
        </button>
      )}
      <button
        onClick={stop(onOpenPopout)}
        className={cn(styles.button, styles.popoutAccent)}
        title="Open in new window"
      >
        <ExternalLink className={styles.iconSize} />
      </button>
      {onRemove && (
        <button
          onClick={stop(onRemove)}
          className={cn(styles.button, "hover:bg-red-600 text-white")}
          title="Remove tile"
        >
          <X className={styles.iconSize} />
        </button>
      )}
    </div>
  );
}
