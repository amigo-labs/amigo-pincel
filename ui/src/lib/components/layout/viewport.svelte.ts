// Layout size class of the window (spec §9.2 "Responsive layout"). The
// editor frame restructures at these widths — tool rail vs bottom strip,
// docked sidebar vs drawer vs bottom sheet — which CSS breakpoints alone
// cannot do without mounting panels twice. Thresholds match Tailwind's
// `sm` (640) and `lg` (1024) so markup can still use those prefixes.

/** `wide` ≥ 1024 px, `medium` 640–1023 px, `narrow` < 640 px. */
export type LayoutSize = 'wide' | 'medium' | 'narrow';

/** Reactive window size class; updated by `watchViewport`. */
export const viewport = $state({ size: 'wide' as LayoutSize });

function classify(): LayoutSize {
  if (typeof window === 'undefined') return 'wide';
  if (window.matchMedia('(min-width: 1024px)').matches) return 'wide';
  if (window.matchMedia('(min-width: 640px)').matches) return 'medium';
  return 'narrow';
}

/** Start tracking the window; returns the cleanup. */
export function watchViewport(): () => void {
  viewport.size = classify();
  const queries = ['(min-width: 1024px)', '(min-width: 640px)'].map((q) =>
    window.matchMedia(q),
  );
  const update = () => (viewport.size = classify());
  for (const q of queries) q.addEventListener('change', update);
  return () => {
    for (const q of queries) q.removeEventListener('change', update);
  };
}
