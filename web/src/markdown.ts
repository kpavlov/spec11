// Small, safe markdown subset for spec files: everything is HTML-escaped first, then a fixed set of patterns is turned back into tags.
// Requirement IDs (ABC-12) become <a data-id> so the viewer can open their trace.
const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

const ID = /\b([A-Z]{2,5}-\d{1,4})\b/g; // the core's ID shape

/** One line of markdown as HTML: code spans, bold, italic, strike, links and requirement-ID links. */
export function inline(s: string): string {
  const codes: string[] = [];
  let t = esc(s).replace(/(`+)\s?(.+?)\s?\1(?!`)/g, (_, __, c) => `\u0000${codes.push(c) - 1}\u0000`); // code spans: no further formatting
  t = t
    .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
    .replace(/~~(.+?)~~/g, "<s>$1</s>")
    .replace(/(^|[^*\w])\*([^*\s][^*]*?)\*(?!\w)/g, "$1<em>$2</em>")
    .replace(/\[([^\]]+)\]\((https?:\/\/[^\s)]+)\)/g, '<a href="$2" target="_blank" rel="noopener noreferrer">$1</a>')
    .replace(ID, '<a class="id" href="/requirements/$1" data-id="$1">$1</a>');
  // RFC 2119 level as a pill; skips tags so hrefs stay intact
  t = t.replace(/(<[^>]+>)|\b(MUST|SHOULD|MAY)\b/g, (m, tag, w) => (tag ? m : `<span class="rfc ${w.toLowerCase()}">${w}</span>`));
  return t.replace(/\u0000(\d+)\u0000/g, (_, i) => `<code>${codes[+i]}</code>`);
}

const cells = (row: string) => row.trim().replace(/^\||\|$/g, "").split("|").map((c) => c.trim());

export function render(md: string): string {
  const ls = md.split("\n");
  const out: string[] = [];
  let i = 0;
  while (i < ls.length) {
    const l = ls[i];
    if (/^```/.test(l)) {
      const code: string[] = [];
      for (i++; i < ls.length && !/^```/.test(ls[i]); i++) code.push(ls[i]);
      i++;
      out.push(`<pre><code>${esc(code.join("\n"))}</code></pre>`);
    } else if (/^#{1,6}\s/.test(l)) {
      const n = l.match(/^#+/)![0].length;
      out.push(`<h${n}>${inline(l.replace(/^#+\s*/, ""))}</h${n}>`);
      i++;
    } else if (/^\s*[-*]\s/.test(l)) {
      const items: string[] = [];
      for (; i < ls.length && (/^\s*[-*]\s/.test(ls[i]) || (/^\s+\S/.test(ls[i]) && items.length)); i++) {
        if (/^\s*[-*]\s/.test(ls[i])) items.push(ls[i].replace(/^\s*[-*]\s/, ""));
        else items[items.length - 1] += " " + ls[i].trim(); // wrapped bullet
      }
      out.push(`<ul>${items.map((x) => `<li>${inline(x)}</li>`).join("")}</ul>`);
    } else if (/^\|.*\|\s*$/.test(l) && /^\|?[\s:|-]+\|[\s:|-]*$/.test(ls[i + 1] ?? "")) {
      const head = cells(l);
      const body: string[][] = [];
      for (i += 2; i < ls.length && /^\|.*\|\s*$/.test(ls[i]); i++) body.push(cells(ls[i]));
      out.push(`<table><thead><tr>${head.map((c) => `<th>${inline(c)}</th>`).join("")}</tr></thead><tbody>${body.map((r) => `<tr>${r.map((c) => `<td>${inline(c)}</td>`).join("")}</tr>`).join("")}</tbody></table>`);
    } else if (!l.trim()) {
      i++;
    } else {
      const p: string[] = [];
      for (; i < ls.length && ls[i].trim() && !/^(#{1,6}\s|```|\s*[-*]\s)/.test(ls[i]); i++) p.push(ls[i].trim());
      out.push(`<p>${inline(p.join(" "))}</p>`);
    }
  }
  return out.join("\n");
}
