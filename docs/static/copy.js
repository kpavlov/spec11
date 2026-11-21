// "Copy" button on every code block: wraps the <pre> so the button stays put while a wide block scrolls.
for (const pre of document.querySelectorAll('main pre')) {
  const box = document.createElement('div');
  box.className = 'codebox';
  pre.replaceWith(box);
  box.append(pre);
  const btn = document.createElement('button');
  btn.type = 'button';
  btn.className = 'copy';
  btn.textContent = 'Copy';
  btn.onclick = async () => {
    try { await navigator.clipboard.writeText(pre.innerText.replace(/\n$/, '')); btn.textContent = 'Copied'; }
    catch { btn.textContent = 'Failed'; }
    setTimeout(() => { btn.textContent = 'Copy'; }, 1500);
  };
  box.append(btn);
}

// "Copy page": the page's Markdown twin goes to the clipboard. Needs JS, so the buttons start hidden;
// the "View as Markdown" link works without it. Safari only allows the write within the click, hence the pending Blob.
const pageMenu = document.querySelector('.page-actions details');
for (const b of document.querySelectorAll('.copy-page')) {
  b.hidden = false;
  b.onclick = async () => {
    const label = document.querySelector('.page-actions > .copy-page');
    const idle = label.textContent;
    const text = fetch(b.dataset.markdown)
      .then((r) => (r.ok ? r.blob() : Promise.reject(r.status)))
      .then((blob) => new Blob([blob], { type: 'text/plain' }));
    try { await navigator.clipboard.write([new ClipboardItem({ 'text/plain': text })]); label.textContent = 'Copied'; }
    catch { label.textContent = 'Failed'; }
    pageMenu.open = false;
    setTimeout(() => { label.textContent = idle; }, 1500);
  };
}
document.addEventListener('click', (e) => { if (pageMenu?.open && !e.target.closest('.page-actions')) pageMenu.open = false; });
