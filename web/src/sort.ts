// Sortable column headers: click ascending, descending, back to the source order. `Sort` is a $state object that toggleSort mutates.
export type Sort = { key: string; dir: 1 | -1 };

export const toggleSort = (st: Sort, key: string) => {
  if (st.key !== key) { st.key = key; st.dir = 1; } else if (st.dir === 1) st.dir = -1; else st.key = "";
};

export const sorted = <T,>(list: T[], st: Sort, val: (x: T, k: string) => string | number): T[] =>
  !st.key ? list : [...list].sort((a, b) => {
    const x = val(a, st.key), y = val(b, st.key);
    return st.dir * (typeof x === "number" && typeof y === "number" ? x - y : String(x).localeCompare(String(y), undefined, { numeric: true }));
  });
