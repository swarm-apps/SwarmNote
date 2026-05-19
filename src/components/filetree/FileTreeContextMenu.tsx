import { Trans } from "@lingui/react/macro";
import { FilePlus, FolderPlus, Pencil, Trash2 } from "lucide-react";
import type { NodeApi } from "react-arborist";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import type { FileTreeNode_Serialize as FileTreeNode } from "@/lib/bindings";

interface FileTreeContextMenuProps {
  node: NodeApi<FileTreeNode> | null;
  children: React.ReactNode;
  onCreateFile: (parentRel: string) => void;
  onCreateDir: (parentRel: string) => void;
  onDelete: (node: NodeApi<FileTreeNode>) => void;
  onRename: (node: NodeApi<FileTreeNode>) => void;
}

export function FileTreeContextMenu({
  node,
  children,
  onCreateFile,
  onCreateDir,
  onDelete,
  onRename,
}: FileTreeContextMenuProps) {
  const isFolder = node?.isInternal ?? true;
  const parentRel = node ? (isFolder ? node.data.id : getParentRel(node.data.id)) : "";

  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>{children}</ContextMenuTrigger>
      <ContextMenuContent className="w-48">
        <ContextMenuItem onClick={() => onCreateFile(parentRel)}>
          <FilePlus className="h-4 w-4" />
          <Trans>新建笔记</Trans>
        </ContextMenuItem>
        <ContextMenuItem onClick={() => onCreateDir(parentRel)}>
          <FolderPlus className="h-4 w-4" />
          <Trans>新建文件夹</Trans>
        </ContextMenuItem>
        {node && (
          <>
            <ContextMenuSeparator />
            <ContextMenuItem onClick={() => onRename(node)}>
              <Pencil className="h-4 w-4" />
              <Trans>重命名</Trans>
            </ContextMenuItem>
            <ContextMenuItem
              className="text-destructive focus:text-destructive"
              onClick={() => onDelete(node)}
            >
              <Trash2 className="h-4 w-4" />
              <Trans>删除</Trans>
            </ContextMenuItem>
          </>
        )}
      </ContextMenuContent>
    </ContextMenu>
  );
}

function getParentRel(id: string): string {
  const lastSlash = id.lastIndexOf("/");
  return lastSlash === -1 ? "" : id.substring(0, lastSlash);
}
