<script lang="ts">
  import { m, type Messages } from '#lib/i18n/index.ts';

  type NoteKey = keyof Messages['comparison']['notes'];
  type Mark = '✅' | '❌' | '🟡';
  // A plain mark, a mark with a note, or a price word.
  type Cell = Mark | { mark: Mark; note: NoteKey } | 'free' | 'paid';

  interface Row {
    feature: keyof Messages['comparison']['rows'];
    pincel: Cell;
    aseprite: Cell;
    piskel: Cell;
  }

  const rows: Row[] = [
    { feature: 'format', pincel: '✅', aseprite: { mark: '✅', note: 'native' }, piskel: '❌' },
    { feature: 'tilemaps', pincel: '✅', aseprite: '✅', piskel: '❌' },
    { feature: 'slices', pincel: '✅', aseprite: '✅', piskel: '❌' },
    { feature: 'timeline', pincel: '✅', aseprite: '✅', piskel: { mark: '✅', note: 'basic' } },
    { feature: 'tablet', pincel: '✅', aseprite: '✅', piskel: '❌' },
    { feature: 'browser', pincel: '✅', aseprite: '❌', piskel: '✅' },
    {
      feature: 'photo',
      pincel: { mark: '✅', note: 'imageMode' },
      aseprite: { mark: '❌', note: 'pixelOnly' },
      piskel: '❌',
    },
    {
      feature: 'desktop',
      pincel: { mark: '✅', note: 'tauri' },
      aseprite: '✅',
      piskel: { mark: '✅', note: 'nwjs' },
    },
    { feature: 'library', pincel: '✅', aseprite: '❌', piskel: '❌' },
    { feature: 'scripting', pincel: { mark: '❌', note: 'phase2' }, aseprite: '✅', piskel: '❌' },
    { feature: 'brushes', pincel: { mark: '❌', note: 'phase2' }, aseprite: '✅', piskel: '✅' },
    { feature: 'price', pincel: 'free', aseprite: 'paid', piskel: 'free' },
    {
      feature: 'openSource',
      pincel: { mark: '✅', note: 'mitApache' },
      aseprite: { mark: '🟡', note: 'sourceAvailable' },
      piskel: { mark: '✅', note: 'apache' },
    },
    {
      feature: 'active',
      pincel: '✅',
      aseprite: '✅',
      piskel: { mark: '🟡', note: 'noRelease' },
    },
  ];
</script>

<div class="table-wrap" role="region" aria-label={m().comparison.label}>
  <table>
    <thead>
      <tr>
        <th scope="col">{m().comparison.feature}</th>
        <th scope="col" class="pincel">Pincel</th>
        <th scope="col">Aseprite</th>
        <th scope="col">Piskel</th>
      </tr>
    </thead>
    <tbody>
      {#each rows as row (row.feature)}
        <tr>
          <th scope="row">{m().comparison.rows[row.feature]}</th>
          {#each [row.pincel, row.aseprite, row.piskel] as cell, i (i)}
            <td class:pincel={i === 0}>
              {#if cell === 'free' || cell === 'paid'}
                <span class="mark">{m().comparison[cell]}</span>
              {:else if typeof cell === 'string'}
                <span class="mark">{cell}</span>
              {:else}
                <span class="mark">{cell.mark}</span>
                <span class="note">{m().comparison.notes[cell.note]}</span>
              {/if}
            </td>
          {/each}
        </tr>
      {/each}
    </tbody>
  </table>
</div>
<p class="footnote">{m().comparison.footnote}</p>

<style>
  .table-wrap {
    overflow-x: auto;
    border: 1px solid var(--color-border-subtle);
    background-color: var(--color-bg-card);
  }
  table {
    width: 100%;
    /* Separate (zero-spaced) borders belong to their cells, so a sticky
       cell keeps its row line while it scrolls. */
    border-collapse: separate;
    border-spacing: 0;
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
    /* Opaque (mixed into the card colour, not into transparent) so a sticky
       cell hides what scrolls beneath it on a phone. */
    background-color: color-mix(in srgb, var(--color-bg-elevated) 30%, var(--color-bg-card));
    border-bottom: 2px solid var(--color-border-strong);
  }
  tbody tr:last-child th,
  tbody tr:last-child td {
    border-bottom: none;
  }
  th[scope='row'] {
    color: var(--color-fg-secondary);
    font-weight: 500;
    background-color: var(--color-bg-card);
  }
  td.pincel,
  th.pincel {
    background-color: color-mix(in srgb, var(--color-brand-primary) 8%, var(--color-bg-card));
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
  /* Phones: the table keeps readable columns and scrolls inside its wrapper
     (never the page); the feature labels and the Pincel column stay in view
     while the competitor columns scroll past (website-comparison AC-09). */
  /* Fixed column widths keep every competitor column narrower than the strip
     the two sticky columns leave free at 390 px, so scrolling reveals each
     one completely. */
  @media (max-width: 720px) {
    table {
      width: max-content;
      font-size: 0.875rem;
    }
    th,
    td {
      padding: 0.6rem 0.75rem;
      width: 7.5rem;
      min-width: 7.5rem;
      max-width: 7.5rem;
      /* A long word ("Kostenpflichtig") breaks instead of running into
         the next column. */
      overflow-wrap: anywhere;
      hyphens: auto;
    }
    th[scope='row'],
    thead th:first-child {
      position: sticky;
      left: 0;
      z-index: 1;
    }
    td.pincel,
    th.pincel {
      position: sticky;
      left: 7.5rem;
      z-index: 1;
      width: 6.5rem;
      min-width: 6.5rem;
      max-width: 6.5rem;
    }
  }
</style>
