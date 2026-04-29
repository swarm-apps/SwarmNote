import { type ReactNode, useEffect, useRef, useState } from "react";

import { TOTAL_STEPS } from "@/components/onboarding/OnboardingFlow";
import { cn } from "@/lib/utils";
import { useOnboardingStore } from "@/stores/onboardingStore";

function StepIndicator() {
  const currentStep = useOnboardingStore((s) => s.currentStep);

  return (
    <div className="flex items-center gap-2">
      {Array.from({ length: TOTAL_STEPS }, (_, step) => (
        <div
          // biome-ignore lint/suspicious/noArrayIndexKey: fixed-length indicator dots
          key={`step-${step}`}
          className={cn(
            "h-2 w-2 rounded-full transition-colors duration-300",
            step === currentStep ? "bg-primary" : "bg-muted-foreground/30",
          )}
        />
      ))}
    </div>
  );
}

interface StepTransitionProps {
  stepKey: number;
  children: ReactNode;
}

export function StepTransition({ stepKey, children }: StepTransitionProps) {
  const [visible, setVisible] = useState(false);
  const prevKey = useRef(stepKey);

  useEffect(() => {
    // Reset on step change
    setVisible(false);
    const timer = requestAnimationFrame(() => setVisible(true));
    prevKey.current = stepKey;
    return () => cancelAnimationFrame(timer);
  }, [stepKey]);

  return (
    <div
      className={cn(
        "flex w-full max-w-md flex-col items-center gap-8 px-6 transition-all duration-300",
        visible ? "translate-y-0 opacity-100" : "translate-y-2 opacity-0",
      )}
    >
      {children}
    </div>
  );
}

interface OnboardingLayoutProps {
  children: ReactNode;
}

export function OnboardingLayout({ children }: OnboardingLayoutProps) {
  return (
    <div className="relative flex h-full flex-col items-center justify-center bg-background">
      {children}
      <div className="absolute bottom-8">
        <StepIndicator />
      </div>
    </div>
  );
}
