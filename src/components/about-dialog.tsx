import { useEffect } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ArrowUpRight, Bug } from "lucide-react";
import appIconBundle from "@/assets/app-icon-bundle.svg";
import { Button } from "@/components/ui/button";

interface AboutDialogProps {
  version: string;
  onClose: () => void;
}

export function AboutDialog({ version, onClose }: AboutDialogProps) {
  const repoUrl = "https://github.com/suasgn/openburn";
  const issueUrl = "https://github.com/suasgn/openburn/issues/new/choose";

  const openExternal = (url: string) => {
    openUrl(url).catch(console.error);
  };

  // Close on ESC key
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    };
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);

  // Close when panel hides (loses visibility)
  useEffect(() => {
    const handleVisibilityChange = () => {
      if (document.hidden) {
        onClose();
      }
    };
    document.addEventListener("visibilitychange", handleVisibilityChange);
    return () => document.removeEventListener("visibilitychange", handleVisibilityChange);
  }, [onClose]);

  // Close on backdrop click
  const handleBackdropClick = (e: React.MouseEvent) => {
    if (e.target === e.currentTarget) {
      onClose();
    }
  };

  return (
    <div
      className="absolute inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm rounded-xl"
      onClick={handleBackdropClick}
    >
      <div className="bg-card rounded-lg border shadow-xl p-6 max-w-sm w-full mx-4 animate-in fade-in zoom-in-95 duration-200">
        <div className="flex flex-col items-center text-center">
          <img
            src={appIconBundle}
            alt="OpenBurn"
            className="w-16 h-16 rounded-xl mb-3"
          />

          <h2 className="text-xl font-semibold leading-none">OpenBurn</h2>
          <p className="mt-2 text-sm text-muted-foreground">
            Know your AI spend before it surprises you.
          </p>

          <div className="mt-3 flex items-center gap-1.5">
            <span className="inline-flex items-center rounded-full bg-muted px-2 py-0.5 text-xs text-muted-foreground">
              v{version}
            </span>
            <span className="inline-flex items-center rounded-full border border-border px-2 py-0.5 text-xs text-muted-foreground">
              stable
            </span>
          </div>
        </div>

        <div className="mt-4 grid gap-2">
          <Button type="button" size="sm" className="w-full" onClick={() => openExternal(repoUrl)}>
            View on GitHub
            <ArrowUpRight className="size-3.5" />
          </Button>
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="w-full"
            onClick={() => openExternal(issueUrl)}
          >
            Report an issue
            <Bug className="size-3.5" />
          </Button>
        </div>

        <p className="mt-3 text-center text-xs text-muted-foreground">
          MIT License - Contributions welcome
        </p>
      </div>
    </div>
  );
}
