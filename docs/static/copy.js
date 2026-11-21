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
