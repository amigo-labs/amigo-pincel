<script lang="ts">
  type Cell = '✅' | '❌' | { mark: '✅' | '❌' | '🟡'; note: string };

  interface Row {
    feature: string;
    pincel: Cell;
    aseprite: Cell;
    piskel: Cell;
  }

  const rows: Row[] = [
    {
      feature: 'Aseprite file format (read+write)',
      pincel: '✅',
      aseprite: { mark: '✅', note: 'native' },
      piskel: '❌',
    },
    { feature: 'Tilemaps', pincel: '✅', aseprite: '✅', piskel: '❌' },
    { feature: 'Slices (9-patch, pivot)', pincel: '✅', aseprite: '✅', piskel: '❌' },
    {
      feature: 'Animation timeline + tags',
      pincel: '✅',
      aseprite: '✅',
      piskel: { mark: '✅', note: 'basic' },
    },
    { feature: 'Tablet / pen support', pincel: '✅', aseprite: '✅', piskel: '❌' },
    { feature: 'Runs in browser', pincel: '✅', aseprite: '❌', piskel: '✅' },
    {
      feature: 'Native desktop app',
      pincel: { mark: '✅', note: 'Tauri' },
      aseprite: '✅',
      piskel: { mark: '✅', note: 'via NW.js, ageing' },
    },
    { feature: 'Embeddable as a library', pincel: '✅', aseprite: '❌', piskel: '❌' },
    {
      feature: 'Lua scripting / extensions',
      pincel: { mark: '❌', note: 'Phase 2' },
      aseprite: '✅',
      piskel: '❌',
    },
    {
      feature: 'Custom brushes',
      pincel: { mark: '❌', note: 'Phase 2' },
      aseprite: '✅',
      piskel: '✅',
    },
    { feature: 'Price', pincel: 'Free' as unknown as Cell, aseprite: 'Paid' as unknown as Cell, piskel: 'Free' as unknown as Cell },
    {
      feature: 'Open source',
      pincel: { mark: '✅', note: 'MIT/Apache' },
      aseprite: { mark: '✅', note: 'EULA-restricted' },
      piskel: { mark: '✅', note: 'Apache 2.0' },
    },
    {
      feature: 'Active development',
      pincel: '✅',
      aseprite: '✅',
      piskel: { mark: '🟡', note: 'modernization in progress' },
    },
  ];

  function isPlain(c: Cell): c is '✅' | '❌' {
    return typeof c === 'string';
  }
</script>

<div class="table-wrap" role="region" aria-label="Feature comparison table">
  <table>
    <thead>
      <tr>
        <th scope="col">Feature</th>
        <th scope="col" class="pincel">Pincel</th>
        <th scope="col">Aseprite</th>
        <th scope="col">Piskel</th>
      </tr>
    </thead>
    <tbody>
      {#each rows as row (row.feature)}
        <tr>
          <th scope="row">{row.feature}</th>
          {#each [row.pincel, row.aseprite, row.piskel] as cell, i (i)}
            <td class:pincel={i === 0}>
              {#if isPlain(cell)}
                <span class="mark">{cell}</span>
              {:else}
                <span class="mark">{cell.mark}</span>
                <span class="note">{cell.note}</span>
              {/if}
            </td>
          {/each}
        </tr>
      {/each}
    </tbody>
  </table>
</div>
<p class="footnote">
  Pincel is new and Aseprite has 14 years of features. We're catching up where it matters for
  game-dev workflows. Where it doesn't, we're not.
</p>

<style>
  .table-wrap {
    overflow-x: auto;
    border: 1px solid var(--color-border-subtle);
    background-color: var(--color-bg-card);
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.95rem;
  }
  th,
  td {
    text-align: left;
    padding: 0.75rem 1rem;
    border-bottom: 1px solid var(--color-border-subtle);
    vertical-align: top;
  }
  thead th {
    font-family: var(--font-body);
    font-weight: 600;
    color: var(--color-fg-primary);
    background-color: color-mix(in srgb, var(--color-bg-elevated) 30%, transparent);
    border-bottom: 2px solid var(--color-border-strong);
  }
  tbody tr:last-child th,
  tbody tr:last-child td {
    border-bottom: none;
  }
  th[scope='row'] {
    color: var(--color-fg-secondary);
    font-weight: 500;
  }
  td.pincel,
  th.pincel {
    background-color: color-mix(in srgb, var(--color-brand-primary) 8%, transparent);
  }
  .mark {
    font-size: 1.05rem;
    display: inline-block;
    margin-right: 0.4rem;
  }
  .note {
    font-size: 0.8rem;
    color: var(--color-fg-muted);
  }
  .footnote {
    margin: 1.5rem 0 0;
    color: var(--color-fg-muted);
    font-size: 0.9rem;
    max-width: 60ch;
  }
</style>
