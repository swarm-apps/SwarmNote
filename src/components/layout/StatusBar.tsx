import { Trans } from "@lingui/react/macro";
import { CircleCheck, Pencil } from "lucide-react";
import { useEditorStore } from "@/stores/editorStore";

export function StatusBar() {
  const currentDocId = useEditorStore((s) => s.currentDocId);
  const charCount = useEditorStore((s) => s.charCount);
  const isDirty = useEditorStore((s) => s.isDirty);
  const lastSavedAt = useEditorStore((s) => s.lastSavedAt);

  if (!currentDocId) {
    return (
      <footer className="flex h-7 shrink-0 items-center border-t border-border bg-card px-4" />
    );
  }

  const timeStr = lastSavedAt
    ? lastSavedAt.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" })
    : null;

  return (
    <footer className="flex h-7 shrink-0 items-center justify-between border-t border-border bg-card px-4">
      <div className="flex items-center gap-3">
        <span className="text-xs text-muted-foreground">
          {charCount.toLocaleString()} <Trans>字符</Trans>
        </span>
      </div>
      <div className="flex items-center gap-2" role="status" aria-live="polite" aria-atomic="true">
        {isDirty ? (
          <>
            <Pencil className="h-3 w-3 text-muted-foreground" />
            <span className="text-xs text-muted-foreground">
              <Trans>未保存</Trans>
            </span>
          </>
        ) : timeStr ? (
          <>
            <CircleCheck className="h-3 w-3 text-green-500" />
            <span className="text-xs text-muted-foreground">
              <Trans>已保存</Trans> {timeStr}
            </span>
          </>
        ) : null}
      </div>
    </footer>
  );
}
