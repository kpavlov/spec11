// Highlight the sidebar TOC entry for the section being read: the last heading whose top has passed the sticky header.
// A short last section can never scroll that far, so at the bottom of the page the entry just jumped to (or the last one) wins.
{
  const id = a => decodeURIComponent(a.hash.slice(1));
  const links = [...document.querySelectorAll('#TableOfContents a')];
  const heads = links.map(a => document.getElementById(id(a))).filter(Boolean);
  let pinned = decodeURIComponent(location.hash.slice(1));
  const mark = () => {
    const bottom = innerHeight + scrollY >= document.documentElement.scrollHeight - 2;
    let cur = null;
    for (const h of heads) if (h.getBoundingClientRect().top <= 80) cur = h.id;
    if (bottom) cur = heads.some(h => h.id === pinned) ? pinned : heads.at(-1).id;
    for (const a of links) a.classList.toggle('active', id(a) === cur);
  };
  if (heads.length) {
    addEventListener('scroll', () => { if (innerHeight + scrollY < document.documentElement.scrollHeight - 2) pinned = ''; mark(); }, { passive: true });
    addEventListener('hashchange', () => { pinned = decodeURIComponent(location.hash.slice(1)); mark(); });
    mark();
  }
}
