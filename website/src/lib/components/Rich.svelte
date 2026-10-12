<script lang="ts">
  // Renders a copy string with the small inline markup the dictionaries
  // use: `code`, [text](url), **bold**, and "\n" as a line break.
  // Internal links (`/…`) get the page's language prefix.
  import { localized } from '$lib/i18n';

  interface Props {
    text: string;
  }
  const { text }: Props = $props();

  type Part =
    | { kind: 'text'; value: string }
    | { kind: 'code'; value: string }
    | { kind: 'bold'; value: string }
    | { kind: 'link'; value: string; href: string }
    | { kind: 'br' };

  const TOKEN = /`([^`]+)`|\[([^\]]+)\]\(([^)\s]+)\)|\*\*([^*]+)\*\*|\n/g;

  const parts = $derived.by<Part[]>(() => {
    const out: Part[] = [];
    let last = 0;
    for (const match of text.matchAll(TOKEN)) {
      const at = match.index ?? 0;
      if (at > last) out.push({ kind: 'text', value: text.slice(last, at) });
      const [whole, code, linkText, href, bold] = match;
      if (code !== undefined) out.push({ kind: 'code', value: code });
      else if (linkText !== undefined && href !== undefined) {
        out.push({
          kind: 'link',
          value: linkText,
          href: href.startsWith('/') ? localized(href) : href,
        });
      } else if (bold !== undefined) out.push({ kind: 'bold', value: bold });
      else out.push({ kind: 'br' });
      last = at + whole.length;
    }
    if (last < text.length) out.push({ kind: 'text', value: text.slice(last) });
    return out;
  });
</script>

{#each parts as part, i (i)}{#if part.kind === 'text'}{part.value}{:else if part.kind === 'code'}<code
      >{part.value}</code
    >{:else if part.kind === 'bold'}<strong>{part.value}</strong>{:else if part.kind === 'link'}<a
      href={part.href}
      rel={part.href.startsWith('http') ? 'external' : undefined}>{part.value}</a
    >{:else}<br />{/if}{/each}
