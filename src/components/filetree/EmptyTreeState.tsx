import { Trans } from "@lingui/react/macro";

/**
 * Minimal empty state for the sidebar file tree. Intentionally visually
 * subordinate — the primary "create your first note" CTA lives in the
 * EditorPane's EmptyState to avoid duplicating affordances.
 */
export function EmptyTreeState() {
  return (
    <div className="flex flex-1 items-center justify-center px-4 py-6 text-center">
      <p className="text-xs text-muted-foreground/60">
        <Trans>暂无笔记</Trans>
      </p>
    </div>
  );
}
