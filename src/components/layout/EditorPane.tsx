import { NoteEditor } from "@/components/editor/NoteEditor";
import { EmptyState } from "@/components/layout/EmptyState";
import { StatusBar } from "@/components/layout/StatusBar";
import { useEditorStore } from "@/stores/editorStore";

export function EditorPane() {
  const currentDocId = useEditorStore((s) => s.currentDocId);

  return (
    <main className="flex min-w-0 flex-1 flex-col bg-background">
      <div className="flex min-h-0 flex-1 flex-col">
        {currentDocId ? <NoteEditor key={currentDocId} /> : <EmptyState />}
      </div>
      <StatusBar />
    </main>
  );
}
