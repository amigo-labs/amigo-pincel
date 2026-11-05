<script lang="ts">
  import { m, type Messages } from '#lib/i18n/index.ts';

  // The two comparison tables of the home page (docs/specs/website-comparison.md,
  // website-spec §4.1.4). Every competitor cell follows pincel.md §18.

  type Copy = Messages['comparison'];
  type NoteKey = keyof Copy['notes'];
  type Mark = '✅' | '❌' | '🟡';
  type Price = 'free' | 'paid';
  // A plain mark, a price word, or either with a note.
  type Cell = Mark | Price | { mark: Mark | Price; note: NoteKey };

  interface Row {
    feature: keyof Copy['rows'];
    cells: readonly [Cell, Cell, Cell, Cell];
  }

  interface Table {
    id: 'pixel' | 'image';
    columns: readonly [string, string, string, string];
    rows: readonly Row[];
    sources: readonly string[];
  }

  const n = (mark: Mark | Price, note: NoteKey): Cell => ({ mark, note });

  const pixel: Table = {
    id: 'pixel',
    columns: ['Pincel', 'Aseprite', 'Pixelorama', 'Piskel'],
    rows: [
      {
        feature: 'format',
        cells: [n('🟡', 'rgbaOnly'), n('✅', 'native'), n('✅', 'importExport'), '❌'],
      },
      { feature: 'tilemaps', cells: ['✅', '✅', '✅', '❌'] },
      { feature: 'slices', cells: ['✅', '✅', '❌', '❌'] },
      { feature: 'timeline', cells: ['✅', '✅', '✅', n('✅', 'basic')] },
      { feature: 'tablet', cells: ['✅', '✅', '✅', '❌'] },
      { feature: 'browser', cells: ['✅', '❌', '✅', '✅'] },
      {
        feature: 'photo',
        cells: [n('✅', 'imageMode'), n('❌', 'pixelOnly'), n('❌', 'pixelOnly'), '❌'],
      },
      { feature: 'desktop', cells: [n('✅', 'tauri'), '✅', '✅', n('✅', 'nwjs')] },
      { feature: 'library', cells: ['✅', '❌', '❌', '❌'] },
      { feature: 'scripting', cells: [n('❌', 'phase2'), '✅', n('✅', 'extensions'), '❌'] },
      { feature: 'brushes', cells: [n('❌', 'phase2'), '✅', '✅', '❌'] },
      { feature: 'pixelPerfect', cells: ['✅', '✅', '✅', '❌'] },
      { feature: 'animatedExport', cells: ['✅', n('🟡', 'gifOnly'), '✅', n('🟡', 'gifOnly')] },
      { feature: 'penPressure', cells: ['✅', '✅', '✅', '❌'] },
      { feature: 'indexed', cells: ['❌', '✅', '✅', '❌'] },
      { feature: 'psd', cells: [n('🟡', 'psdRead'), '❌', n('✅', 'importOnly'), '❌'] },
      { feature: 'price', cells: ['free', 'paid', 'free', 'free'] },
      {
        feature: 'openSource',
        cells: [n('✅', 'mitApache'), n('🟡', 'sourceAvailable'), n('✅', 'mit'), n('✅', 'apache')],
      },
      { feature: 'active', cells: ['✅', '✅', '✅', n('🟡', 'noRelease')] },
    ],
    sources: [
      'aseprite.org (docs, release notes, FAQ), github.com/aseprite/aseprite',
      'github.com/Orama-Interactive/Pixelorama, pixelorama.org',
      'github.com/piskelapp/piskel',
    ],
  };

  const image: Table = {
    id: 'image',
    columns: ['Pincel', 'Paint.NET', 'Pinta', 'Photopea'],
    rows: [
      { feature: 'windows', cells: ['✅', '✅', '✅', n('🟡', 'inBrowser')] },
      { feature: 'mac', cells: ['✅', n('🟡', 'wineBeta'), '✅', n('🟡', 'inBrowser')] },
      { feature: 'linux', cells: ['✅', n('🟡', 'wineBeta'), '✅', n('🟡', 'inBrowser')] },
      { feature: 'web', cells: ['✅', '❌', '❌', '✅'] },
      { feature: 'layersBlend', cells: ['✅', '✅', '✅', '✅'] },
      { feature: 'adjustments', cells: ['✅', '✅', '✅', '✅'] },
      { feature: 'magicWand', cells: ['✅', '✅', '✅', '✅'] },
      { feature: 'text', cells: ['✅', n('🟡', 'simple'), '✅', '✅'] },
      { feature: 'penPressure', cells: ['✅', '✅', '❌', '✅'] },
      { feature: 'psd', cells: [n('🟡', 'psdRead'), n('🟡', 'plugin'), '❌', '✅'] },
      {
        feature: 'priceLicense',
        cells: [
          n('free', 'mitApache'),
          n('free', 'proprietary'),
          n('free', 'mit'),
          n('free', 'adsPremium'),
        ],
      },
      { feature: 'offline', cells: ['✅', '✅', '✅', n('✅', 'onceLoaded')] },
    ],
    sources: [
      'paint.net (features, documentation, roadmap), forums.paint.net (PSD plugin)',
      'github.com/PintaProject/Pinta, pinta-project.com',
      'photopea.com/learn, photopea.com/api',
    ],
  };

  const tables = [pixel, image] as const;

  const markText = (mark: Mark | Price): string =>
    mark === 'free' || mark === 'paid' ? m().comparison[mark] : mark;
</script>

{#each tables as table (table.id)}
  <h3 class="table-title">{m().comparison[table.id].title}</h3>
  <div
    class="table-wrap"
    role="region"
    aria-label={m().comparison[table.id].label}
    data-table={table.id}
  >
    <table>
      <thead>
        <tr>
          <th scope="col">{m().comparison.feature}</th>
          {#each table.columns as column, i (column)}
            <th scope="col" class:pincel={i === 0}>{column}</th>
          {/each}
        </tr>
      </thead>
      <tbody>
        {#each table.rows as row (row.feature)}
          <tr>
            <th scope="row">{m().comparison.rows[row.feature]}</th>
            {#each row.cells as cell, i (i)}
              <td class:pincel={i === 0}>
                {#if typeof cell === 'string'}
                  <span class="mark">{markText(cell)}</span>
                {:else}
                  <span class="mark">{markText(cell.mark)}</span>
                  <span class="note">{m().comparison.notes[cell.note]}</span>
                {/if}
              </td>
            {/each}
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
  <div class="footnotes" data-footnotes={table.id}>
    {#if table.id === 'pixel'}
      <p class="footnote">{m().comparison.footnote}</p>
    {/if}
    <p class="footnote">{m().comparison.snapshot}</p>
    <details class="sources">
      <summary>{m().comparison.sources}</summary>
      <ul>
        {#each table.sources as source (source)}
          <li>{source}</li>
        {/each}
      </ul>
      {#if table.id === 'pixel'}
        <p>{m().comparison.related}</p>
      {/if}
    </details>
  </div>
{/each}

<style>
  .table-title {
    margin: 2.5rem 0 1rem;
    font-size: 1.15rem;
    color: var(--color-fg-primary);
  }
  .table-title:first-child {
    margin-top: 0;
  }
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
  .footnotes {
    margin-top: 1rem;
  }
  .footnote {
    margin: 0.5rem 0 0;
    color: var(--color-fg-muted);
    font-size: 0.9rem;
    max-width: 60ch;
  }
  .sources {
    margin-top: 0.75rem;
    color: var(--color-fg-muted);
    font-size: 0.85rem;
    max-width: 60ch;
  }
  .sources summary {
    cursor: pointer;
  }
  .sources ul {
    margin: 0.5rem 0;
    padding-left: 1.25rem;
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
