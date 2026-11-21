// Theme: system | light | dark, remembered per browser. data-theme on <html> drives tokens.css; dark.css holds the dark syntax colours; its media attribute switches it:
// the OS preference for system, always for dark, never for light. Runs in <head> so the page does not flash.
{ // a block, so these names do not collide with search.js
  const dark = document.getElementById('dark'), key = 'spec17.theme';
  const themes = [['system', '◐', '(prefers-color-scheme: dark)'], ['light', '☀', 'not all'], ['dark', '☾', 'all']];
  let i = 0;
  try { i = Math.max(0, themes.findIndex(t => t[0] === localStorage.getItem(key))); } catch { /* storage blocked */ }
  const apply = () => {
    dark.media = themes[i][2];
    i ? document.documentElement.dataset.theme = themes[i][0] : document.documentElement.removeAttribute('data-theme'); // tokens.css
    const btn = document.getElementById('theme');
    if (!btn) return;
    const [name, icon] = themes[i], next = themes[(i + 1) % themes.length][0];
    btn.textContent = icon;
    btn.title = `Theme: ${name} (click for ${next})`;
    btn.setAttribute('aria-label', `Theme: ${name}, switch to ${next}`);
  };
  apply();
  addEventListener('DOMContentLoaded', () => {
    const btn = document.getElementById('theme');
    btn.hidden = false;
    btn.onclick = () => {
      i = (i + 1) % themes.length;
      try { i ? localStorage.setItem(key, themes[i][0]) : localStorage.removeItem(key); } catch { /* storage blocked */ }
      apply();
    };
    apply();
  });
}
