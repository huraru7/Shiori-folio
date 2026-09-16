import { getCategoryMeta } from "../../lib/library";
import type { TreeFolder } from "../../lib/library";
import "./FolderTree.css";

interface FolderTreeProps {
  root: TreeFolder;
  selectedPath: string;
  expandedPaths: Set<string>;
  onSelect: (path: string) => void;
  onToggle: (path: string) => void;
}

// 全件閲覧のエクスプローラー風UI(2026-09-16追加)左ペイン。library/直下の
// トップレベルフォルダ(00-inbox等)から再帰的に展開できるツリー表示。
// Windowsエクスプローラーの左ペインを参考にしつつ、詩織の色分けトークン
// (getCategoryMeta)はトップレベルの行にだけ残し、識別しやすくしている。
function FolderTree({ root, selectedPath, expandedPaths, onSelect, onToggle }: FolderTreeProps) {
  const topFolders = [...root.folders.values()].sort((a, b) => a.name.localeCompare(b.name));
  return (
    <div className="folder-tree">
      {topFolders.map((folder) => (
        <FolderTreeNode
          key={folder.path}
          folder={folder}
          depth={0}
          selectedPath={selectedPath}
          expandedPaths={expandedPaths}
          onSelect={onSelect}
          onToggle={onToggle}
        />
      ))}
    </div>
  );
}

interface FolderTreeNodeProps {
  folder: TreeFolder;
  depth: number;
  selectedPath: string;
  expandedPaths: Set<string>;
  onSelect: (path: string) => void;
  onToggle: (path: string) => void;
}

function FolderTreeNode({
  folder,
  depth,
  selectedPath,
  expandedPaths,
  onSelect,
  onToggle,
}: FolderTreeNodeProps) {
  const isExpanded = expandedPaths.has(folder.path);
  const isSelected = selectedPath === folder.path;
  const children = [...folder.folders.values()].sort((a, b) => a.name.localeCompare(b.name));
  const hasChildren = children.length > 0;
  const isTop = depth === 0;
  const meta = isTop ? getCategoryMeta(folder.name) : null;

  return (
    <div>
      <div
        className={`folder-tree__row${isSelected ? " folder-tree__row--selected" : ""}`}
        style={{ paddingLeft: `${depth * 16 + 6}px` }}
        onClick={() => onSelect(folder.path)}
      >
        <button
          type="button"
          className={`folder-tree__toggle${isExpanded ? " folder-tree__toggle--expanded" : ""}`}
          aria-hidden={!hasChildren}
          tabIndex={-1}
          onClick={(e) => {
            e.stopPropagation();
            if (hasChildren) onToggle(folder.path);
          }}
        >
          {hasChildren ? "▸" : ""}
        </button>
        {meta && (
          <span className="folder-tree__swatch" style={{ background: meta.hex }} />
        )}
        <span className="folder-tree__name">{meta ? meta.label : folder.name}</span>
      </div>
      {isExpanded && hasChildren && (
        <div>
          {children.map((child) => (
            <FolderTreeNode
              key={child.path}
              folder={child}
              depth={depth + 1}
              selectedPath={selectedPath}
              expandedPaths={expandedPaths}
              onSelect={onSelect}
              onToggle={onToggle}
            />
          ))}
        </div>
      )}
    </div>
  );
}

export default FolderTree;
