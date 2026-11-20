// Local search over index.json (built by Hugo, see layouts/home.json): every term must match the title or text.
const q = document.getElementById('q'), out = document.getElementById('results');
let index;
q.hidden = false;

const clear = () => { out.hidden = true; out.replaceChildren(); };
const el = (tag, text, attrs = {}) => Object.assign(document.createElement(tag), { textContent: text }, attrs);
const snippet = (text, term) => {
  const at = Math.max(0, text.toLowerCase().indexOf(term) - 40);
  return (at ? '…' : '') + text.slice(at, at + 120).trim() + '…';
};

q.addEventListener('input', async () => {
  if (!q.value.trim()) return clear();
  index ??= fetch(q.dataset.index).then(r => r.json());
  const docs = await index;
  const terms = q.value.toLowerCase().split(/\s+/).filter(Boolean); // after the await: the latest input wins
  if (!terms.length) return clear();
  const hits = docs.flatMap(d => {
    const title = d.title.toLowerCase(), text = d.text.toLowerCase();
    if (!terms.every(w => title.includes(w) || text.includes(w))) return [];
    return [{ d, score: terms.reduce((n, w) => n + (title.includes(w) ? 20 : 0) + text.split(w).length - 1, 0) }];
  }).sort((a, b) => b.score - a.score);
  out.replaceChildren(...(hits.length
    ? hits.map(({ d }) => {
        const li = el('li'), a = el('a', d.title, { href: d.url });
        li.append(a, el('small', snippet(d.text, terms.find(w => d.text.toLowerCase().includes(w)) ?? '')));
        return li;
      })
    : [el('li', 'No matches')]));
  out.hidden = false;
});

q.addEventListener('keydown', e => { if (e.key === 'Enter') out.querySelector('a')?.click(); });
document.addEventListener('click', e => { if (!e.target.closest('.search')) clear(); });
document.addEventListener('keydown', e => {
  if (e.key === 'Escape') { q.value = ''; clear(); q.blur(); }
  else if (e.key === '/' && !e.metaKey && !e.ctrlKey && !/^(INPUT|TEXTAREA)$/.test(document.activeElement.tagName)) {
    e.preventDefault();
    q.focus();
  }
});
