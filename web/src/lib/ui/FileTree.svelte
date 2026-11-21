<script lang="ts">
  import { SvelteSet } from "svelte/reactivity";
  import { buildTree, type TreeNode } from "$lib/tree";

  // a collapsible folder tree of files; `mark` returns a change word for a file with working-tree changes
  let { files, selected, href, mark }: { files: string[]; selected?: string; href: (file: string) => string; mark?: (file: string) => string } = $props();
  let q = $state("");
  const tree = $derived(buildTree(files));
  const found = $derived(files.filter((f) => f.toLowerCase().includes(q.trim().toLowerCase())));
  // the folders the reader opened; whatever holds the selection opens too
  const opened = new SvelteSet<string>();
  $effect(() => {
    const parts = selected?.split("/").slice(0, -1) ?? [];
    parts.forEach((_, i) => opened.add(parts.slice(0, i + 1).join("/")));
  });
</script>

{#snippet leaf(file: string, label: string)}
  <a href={href(file)} class:on={file === selected} aria-current={file === selected ? "page" : undefined} title={file}>{label.replace(/\.md$/, "")}</a>{#if mark?.(file)}<span class="chg" title="{mark(file)} in the working tree">✎</span>{/if}
{/snippet}

{#snippet item(n: TreeNode)}
  <li>
    {#if n.file}{@render leaf(n.file, n.name)}
    {:else}
      <details open={opened.has(n.path)} ontoggle={(e) => (e.currentTarget.open ? opened.add(n.path) : opened.delete(n.path))}>
        <summary>{n.name} <span class="muted n">{n.count}</span></summary>
        <ul>{#each n.children as c (c.path)}{@render item(c)}{/each}</ul>
      </details>
    {/if}
  </li>
{/snippet}

<nav aria-label="Documents">
  <input type="search" aria-label="Filter documents" placeholder="Filter documents" bind:value={q} />
  <ul class="root">
    {#if q.trim()}
      {#each found as f (f)}<li>{@render leaf(f, f)}</li>{:else}<li class="muted">No document matches.</li>{/each}
    {:else}
      {#each tree as n (n.path)}{@render item(n)}{/each}
    {/if}
  </ul>
</nav>

<style>
  nav { display: flex; flex-direction: column; gap: 8px; min-height: 0; height: 100%; }
  input { width: 100%; min-width: 0; }
  ul { list-style: none; margin: 0; padding: 0; }
  ul ul { padding-left: 12px; margin-left: 6px; border-left: 1px solid var(--line); }
  .root { overflow: auto; min-height: 0; font-size: var(--text-sm); }
  li { margin: 1px 0; }
  summary { cursor: pointer; padding: 3px 6px; border-radius: var(--radius); color: var(--muted); font-weight: var(--weight-semibold); overflow-wrap: anywhere; }
  summary:hover { background: var(--tint-btn); }
  a { display: inline-block; max-width: calc(100% - 1.4em); padding: 3px 6px; border-radius: var(--radius); color: var(--fg); text-decoration: none; overflow-wrap: anywhere; }
  a:hover { background: var(--tint-btn); color: var(--accent); }
  a.on { background: var(--tint-on); color: var(--accent); font-weight: var(--weight-semibold); }
  .chg { color: var(--accent); margin-left: 2px; }
  .n { font-weight: normal; }
</style>
