// Folders of repo-relative file paths: single-child folders merge ("a/b"), a lone top folder is hoisted, folders come first.
export type TreeNode = { name: string; path: string; file?: string; count: number; children: TreeNode[] };

export function buildTree(files: string[]): TreeNode[] {
  const root: TreeNode = { name: "", path: "", count: 0, children: [] };
  for (const file of files) {
    const parts = file.split("/");
    let at = root;
    parts.slice(0, -1).forEach((name, i) => {
      const path = parts.slice(0, i + 1).join("/");
      let dir = at.children.find((c) => !c.file && c.path === path);
      if (!dir) at.children.push((dir = { name, path, count: 0, children: [] }));
      dir.count++;
      at = dir;
    });
    at.children.push({ name: parts[parts.length - 1], path: file, file, count: 1, children: [] });
  }
  const tidy = (n: TreeNode): TreeNode => {
    while (!n.file && n.children.length === 1 && !n.children[0].file) n = { ...n.children[0], name: `${n.name}/${n.children[0].name}` };
    n.children = n.children.map(tidy).sort((a, b) => Number(!!a.file) - Number(!!b.file));
    return n;
  };
  let top = tidy(root).children;
  while (top.length === 1 && !top[0].file) top = top[0].children;
  return top;
}
