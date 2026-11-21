<script lang="ts">
  import { tick } from "svelte";
  import Pill from "./Pill.svelte";
  import type { Tone } from "$lib/tone";

  let { label, title, tone }: { label: string; title: string; tone?: Tone } = $props();
  const id = $props.id();
  let tip: HTMLElement;
  let trigger: HTMLButtonElement;
  let pinned = false;
  let open = $state(false);

  function close() {
    open = false;
    pinned = false;
    tip.hidePopover();
  }
  $effect(() => {
    if (!open) return;
    const escape = (e: KeyboardEvent) => { if (e.key === "Escape") close(); };
    window.addEventListener("keydown", escape);
    window.addEventListener("scroll", close, true);
    window.addEventListener("resize", close);
    return () => {
      window.removeEventListener("keydown", escape);
      window.removeEventListener("scroll", close, true);
      window.removeEventListener("resize", close);
    };
  });

  async function show() {
    open = true;
    tip.showPopover();
    await tick();
    const box = trigger.getBoundingClientRect();
    tip.style.left = Math.max(8, Math.min(box.left, innerWidth - tip.offsetWidth - 8)) + "px";
    tip.style.top = (box.bottom + tip.offsetHeight + 4 <= innerHeight ? box.bottom : Math.max(4, box.top - tip.offsetHeight)) + "px";
  }
</script>

<button bind:this={trigger} type="button" aria-label={title.split(" — ")[0]} aria-describedby={id} onfocus={show} onblur={close}
  onpointerleave={() => { if (!pinned && document.activeElement !== trigger) close(); }}
  onpointerenter={(e) => { if (e.pointerType !== "touch") show(); }}
  onclick={() => { if (pinned) close(); else { pinned = true; show(); } }}>
  <Pill {tone}>{label}</Pill>
</button>
<span bind:this={tip} {id} popover="manual" role="tooltip">{title}</span>

<style>
  button { display: inline-flex; align-items: center; line-height: 1.25; padding: 0; border: 0; background: transparent; font-family: system-ui, sans-serif; cursor: help; }
  button > :global(span) { margin-right: 0; }
  button:hover:not(:disabled) { background: transparent; }
  [popover] { position: fixed; inset: auto; margin: 0; max-width: min(20rem, calc(100vw - 16px)); white-space: normal; padding: 6px 10px; background: var(--tooltip-bg); color: var(--fg); border: 1px solid var(--muted); border-radius: var(--radius); font: var(--text-sm)/1.5 system-ui, sans-serif; }
</style>
