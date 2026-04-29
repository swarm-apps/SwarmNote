import { createFileRoute } from "@tanstack/react-router";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, X } from "lucide-react";

import { OnboardingFlow } from "@/components/onboarding/OnboardingFlow";
import { isMac } from "@/lib/utils";

function OnboardingPage() {
  const appWindow = getCurrentWindow();

  return (
    <div className="flex h-screen flex-col">
      {/* Title Bar — drag region + close only */}
      <header data-tauri-drag-region className="flex h-10 shrink-0 items-center justify-end px-4">
        {!isMac && (
          <div className="flex items-center gap-1">
            <button
              type="button"
              onClick={() => appWindow.minimize()}
              className="flex h-7 w-9 items-center justify-center text-muted-foreground hover:bg-accent"
            >
              <Minus className="h-3.5 w-3.5" />
            </button>
            <button
              type="button"
              onClick={() => appWindow.close()}
              className="flex h-7 w-9 items-center justify-center text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
            >
              <X className="h-3.5 w-3.5" />
            </button>
          </div>
        )}
      </header>

      {/* Onboarding content */}
      <div className="flex min-h-0 flex-1 items-center justify-center">
        <OnboardingFlow />
      </div>
    </div>
  );
}

export const Route = createFileRoute("/onboarding")({
  component: OnboardingPage,
});
