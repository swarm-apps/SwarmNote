import { ChevronDown, ChevronRight, FileText, Folder, FolderOpen } from "lucide-react";
import type { NodeRendererProps } from "react-arborist";
import type { FileTreeNode as FileTreeNodeData } from "@/commands/fs";
import { cn } from "@/lib/utils";

function RenameInput({ node }: { node: NodeRendererProps<FileTreeNodeData>["node"] }) {
  return (
    <input
      // biome-ignore lint/a11y/noAutofocus: inline rename requires immediate focus
      autoFocus
      type="text"
      defaultValue={node.data.name}
      className="h-5 w-full rounded border border-primary bg-background px-1 text-[13px] outline-none"
      onFocus={(e) => e.currentTarget.select()}
      onBlur={() => node.reset()}
      onKeyDown={(e) => {
        if (e.key === "Escape") node.reset();
        if (e.key === "Enter") node.submit(e.currentTarget.value);
      }}
    />
  );
}

export function FileTreeNodeRenderer({
  node,
  style,
  dragHandle,
}: NodeRendererProps<FileTreeNodeData>) {
  const isSelected = node.isSelected;

  // Note-app convention (Obsidian/Notion/Bear): single-click on a file
  // opens it immediately — there are no tabs to accidentally switch, and
  // an intermediate "selected but not opened" state isn't useful here.
  // Folders toggle expand/collapse on click. Enter/Space mirror click.
  const handleClick = (e: React.MouseEvent) => {
    if (node.isInternal) {
      node.toggle();
    } else {
      node.select();
      node.activate();
    }
    e.stopPropagation();
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      if (node.isInternal) {
        node.toggle();
      } else {
        node.activate();
      }
    }
  };

  return (
    <div
      ref={dragHandle}
      style={{
        ...style,
        // Add base left padding on top of react-arborist's indent
        paddingLeft: `${Number.parseInt(String(style.paddingLeft || "0"), 10) + 8}px`,
      }}
      role="treeitem"
      tabIndex={node.isFocused ? 0 : -1}
      aria-selected={isSelected}
      aria-expanded={node.isInternal ? node.isOpen : undefined}
      className={cn(
        "mx-2 flex items-center gap-1 rounded-md pr-3 py-1.25 text-[13px] cursor-default",
        isSelected
          ? "bg-sidebar-accent text-sidebar-accent-foreground"
          : "text-sidebar-foreground hover:bg-sidebar-accent/50",
      )}
      onClick={handleClick}
      onKeyDown={handleKeyDown}
    >
      {node.isInternal ? (
        node.isOpen ? (
          <ChevronDown className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
        ) : (
          <ChevronRight className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
        )
      ) : (
        <span className="w-3.5 shrink-0" />
      )}

      {node.isInternal ? (
        node.isOpen ? (
          <FolderOpen className="h-3.5 w-3.5 shrink-0 text-primary" />
        ) : (
          <Folder className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
        )
      ) : (
        <FileText
          className={cn(
            "h-3.5 w-3.5 shrink-0",
            isSelected ? "text-primary" : "text-muted-foreground",
          )}
        />
      )}

      {node.isEditing ? (
        <RenameInput node={node} />
      ) : (
        <span className="truncate">{node.data.name}</span>
      )}
    </div>
  );
}
