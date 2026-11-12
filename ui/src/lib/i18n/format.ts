// Message formatting for the UI dictionaries (spec §9.5,
// docs/specs/more-languages.md): `{name}` placeholders and a small ICU
// subset for counts — `{count, plural, one {# file} other {# files}}`, where
// `#` is the count in the locale's number format and the form comes from
// the locale's plural rules (`Intl.PluralRules`), plus locale-aware numbers,
// percentages and dates. Rune-free, so a Node-side spec can test it.

/** A plural form name of `Intl.PluralRules`, or an exact `=n` match. */
type Forms = Map<string, string>;

/** Splits `{a} {b {c}}` style option text into `name {body}` pairs. */
function parseForms(text: string): Forms | null {
  const forms: Forms = new Map();
  let i = 0;
  while (i < text.length) {
    while (i < text.length && /\s/.test(text.charAt(i))) i++;
    if (i >= text.length) break;
    const start = i;
    while (i < text.length && !/[\s{]/.test(text.charAt(i))) i++;
    const name = text.slice(start, i);
    while (i < text.length && /\s/.test(text.charAt(i))) i++;
    if (text.charAt(i) !== '{' || name === '') return null;
    let depth = 0;
    const open = i;
    for (; i < text.length; i++) {
      const ch = text.charAt(i);
      if (ch === '{') depth++;
      else if (ch === '}' && --depth === 0) break;
    }
    if (depth !== 0) return null;
    forms.set(name, text.slice(open + 1, i));
    i++;
  }
  return forms.has('other') ? forms : null;
}

/** The index of the `}` closing the `{` at `open`, or -1. */
function closing(s: string, open: number): number {
  let depth = 0;
  for (let i = open; i < s.length; i++) {
    const ch = s.charAt(i);
    if (ch === '{') depth++;
    else if (ch === '}' && --depth === 0) return i;
  }
  return -1;
}

/**
 * `template` with `{name}` placeholders filled from `params` (unknown ones
 * stay as written) and `{name, plural, …}` blocks resolved for `locale`.
 */
export function formatMessage(
  template: string,
  params: Readonly<Record<string, string | number>> | undefined,
  locale: string,
): string {
  if (!params) return template;
  let out = '';
  let i = 0;
  while (i < template.length) {
    const open = template.indexOf('{', i);
    if (open < 0) {
      out += template.slice(i);
      break;
    }
    out += template.slice(i, open);
    const close = closing(template, open);
    if (close < 0) {
      out += template.slice(open);
      break;
    }
    const inner = template.slice(open + 1, close);
    const plural = /^\s*(\w+)\s*,\s*plural\s*,(.*)$/s.exec(inner);
    if (plural) {
      const name = plural[1] ?? '';
      const forms = parseForms(plural[2] ?? '');
      const value = params[name];
      if (forms !== null && typeof value === 'number') {
        const exact = forms.get(`=${value}`);
        const form =
          exact ??
          forms.get(new Intl.PluralRules(locale).select(value)) ??
          forms.get('other') ??
          '';
        const shown = formatNumber(value, locale);
        out += formatMessage(form.replace(/#/g, shown), params, locale);
      } else {
        out += template.slice(open, close + 1);
      }
    } else if (/^\w+$/.test(inner) && inner in params) {
      out += String(params[inner]);
    } else {
      out += template.slice(open, close + 1);
    }
    i = close + 1;
  }
  return out;
}

/** `value` as the locale writes numbers (decimal separator, grouping). */
export function formatNumber(value: number, locale: string, maxFraction = 2): string {
  return new Intl.NumberFormat(locale, { maximumFractionDigits: maxFraction }).format(value);
}

/** `value` with exactly `digits` decimals, as the locale writes them. */
export function formatFixed(value: number, locale: string, digits: number): string {
  return new Intl.NumberFormat(locale, {
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  }).format(value);
}

/** `percent` (0–100) as the locale writes a percentage ("100 %" in French). */
export function formatPercent(percent: number, locale: string, maxFraction = 0): string {
  return new Intl.NumberFormat(locale, {
    style: 'percent',
    maximumFractionDigits: maxFraction,
    // A zoom readout reaches 6400 %; it reads as one number, never "6,400%".
    useGrouping: false,
  }).format(percent / 100);
}

/** `time` (ms since the epoch) as the locale writes a date and time. */
export function formatDateTime(time: number, locale: string): string {
  return new Intl.DateTimeFormat(locale, { dateStyle: 'medium', timeStyle: 'short' }).format(
    new Date(time),
  );
}

/** The `{name}` placeholder names of `template`, plural blocks included. */
export function placeholders(template: string): string[] {
  const names = new Set<string>();
  for (const m of template.matchAll(/\{\s*(\w+)\s*(?:,\s*plural\s*,|\})/g)) {
    if (m[1]) names.add(m[1]);
  }
  return [...names].sort();
}
