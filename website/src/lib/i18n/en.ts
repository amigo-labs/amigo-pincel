// English site copy — the shape every other language follows (website
// spec §11). Inline markup in strings: `code`, [text](url), **bold**,
// and "\n" for a line break; see `Rich.svelte`. Internal links are
// written unprefixed (`/embed`) and get the page's language prefix.

export const en = {
  meta: { locale: 'en_US' },
  nav: {
    home: 'Home',
    features: 'Features',
    devs: 'For Devs',
    about: 'About',
    primary: 'Primary',
    openEditor: 'Open Editor',
    language: 'Language',
  },
  logo: { home: 'Pincel — home' },
  footer: {
    line: 'Pincel — by [amigo-labs](https://amigo-labs.dev). MIT or Apache-2.0.',
    social: 'Social and feeds',
    drawn: 'Soon to be drawn in Pincel itself.',
    crt: 'CRT effect:',
    on: 'on',
    off: 'off',
  },
  hero: {
    caption: 'Pixel-art & image editor · v0.1',
    title: 'A pixel-art editor\nfor the web.',
    tagline:
      'Aseprite-compatible. Open source. Free. Make sprites, tilemaps, and animations in your browser. Save as `.aseprite`. Drop into your game. Need to touch up a screenshot or a photo? The Image mode opens PNG, JPEG and WebP with layers and effects. No account, no upload, no catch.',
    github: 'View on GitHub',
    scene: 'A small blue character standing in a stone room lit by a flickering torch.',
  },
  grid: {
    aseprite: {
      title: 'Aseprite-Compatible',
      description: 'Read and write .aseprite files. Drop into your game pipeline.',
    },
    tilemaps: {
      title: 'Tilemaps',
      description: 'First-class tilemap layers with tilesets. Stamp, swap, edit in place.',
    },
    slices: {
      title: 'Slices',
      description: 'Named regions, 9-patch, pivot points — for hitboxes, UI, and game logic.',
    },
    animation: {
      title: 'Animation',
      description: 'Frames, tags, onion skin, ping-pong. Tag your states; export sheets.',
    },
    palette: {
      title: 'Palette Workflow',
      description: 'DB32, PICO-8, NES and Game Boy presets, GIMP .gpl import and export.',
    },
    tablet: {
      title: 'Tablet & Pen',
      description: 'Pinch-to-zoom, pen pressure, on-screen Shift and Alt. iPad-friendly.',
    },
    image: {
      title: 'Image Mode',
      description:
        'Layers, blend modes, selections, effects for screenshots and photos. Saves .ora.',
    },
    offline: {
      title: 'Offline PWA',
      description: 'Install it. Works without a connection. Your files stay on your device.',
    },
    embed: {
      title: 'Embeddable',
      description: 'npm package. Drop into your tool. Same editor, in your app.',
    },
    open: {
      title: 'Open Source',
      description: 'MIT / Apache 2.0. Forkable, auditable, yours.',
    },
  },
  how: {
    open: { title: '1. Open', text: 'Drag in a `.aseprite` file or start fresh.' },
    edit: { title: '2. Edit', text: 'Paint, animate, build tilemaps, define slices.' },
    save: { title: '3. Save', text: 'Save back to `.aseprite`. Your engine hot-reloads.' },
  },
  finalCta: { sub: 'Or: `npm install @amigo-labs/pincel` to [embed it](/embed) →' },
  featureSection: {
    inProgress: 'in progress',
    planned: 'planned',
    shortcuts: 'Keyboard shortcuts',
    screenshot: 'screenshot',
  },
  comparison: {
    label: 'Feature comparison table',
    feature: 'Feature',
    free: 'Free',
    paid: 'Paid',
    footnote:
      "Pincel is new and Aseprite has 14 years of features. We're catching up where it matters for game-dev workflows. Where it doesn't, we're not.",
    rows: {
      format: 'Aseprite file format (read+write)',
      tilemaps: 'Tilemaps',
      slices: 'Slices (9-patch, pivot)',
      timeline: 'Animation timeline + tags',
      tablet: 'Tablet / pen support',
      browser: 'Runs in browser',
      photo: 'Layered photo / screenshot editing',
      desktop: 'Native desktop app',
      library: 'Embeddable as a library',
      scripting: 'Lua scripting / extensions',
      brushes: 'Custom brushes',
      price: 'Price',
      openSource: 'Open source',
      active: 'Active development',
    },
    notes: {
      native: 'native',
      basic: 'basic',
      imageMode: 'Image mode',
      pixelOnly: 'pixel art only',
      tauri: 'Tauri',
      nwjs: 'via NW.js, ageing',
      phase2: 'Phase 2',
      mitApache: 'MIT/Apache',
      eula: 'EULA-restricted',
      apache: 'Apache 2.0',
      modernizing: 'modernization in progress',
    },
  },
  home: {
    seoTitle: 'Pincel — a pixel-art editor for the web',
    seoDescription:
      'Aseprite-compatible pixel-art editor that runs in your browser, plus a layered image mode for screenshots and photos. Open source, free, with tilemaps, slices, animation, and tablet support.',
    jsonLd:
      'A pixel-art editor for the web, with a layered image mode for everyday editing. Aseprite-compatible, open source, free. Make sprites, tilemaps, and animations in your browser.',
    pitch:
      "Pincel is built for game developers and pixel artists who want a serious tool that runs in a browser tab. It reads and writes the Aseprite file format, so your work fits into existing pipelines. It supports tilemaps, slices, animation tags, palettes, and indexed color — the things you actually need. Next to it sits an Image mode for everything that isn't pixel art — screenshots, photos, UI mockups — with layers, blend modes, selections, effects and adjustments, saved as OpenRaster so GIMP and Krita open it too. It runs offline as a PWA, speaks English and German, and you can install it on iPad. It's open source under MIT or Apache 2.0, and it will stay that way.",
    whatEyebrow: 'What it does',
    whatTitle: 'Built for game-dev workflows.',
    compareEyebrow: 'Honest comparison',
    compareTitle: 'Where it stands today.',
    howEyebrow: 'In three steps',
    howTitle: 'How it works.',
  },
  features: {
    seoTitle: 'Features — Pincel',
    seoDescription:
      "Every feature in Pincel, deep-dive style: canvas, tools, layers, animation, tilemaps, slices, palette, file format, tablet support, PWA, and what's coming next.",
    caption: 'Feature deep-dive',
    title: 'Everything Pincel does today.',
    lede: "Dense, honest, screenshot-led. Where a feature is still in progress, we say so. Where it's planned but not built, we say that too.",
    sections: {
      canvas: {
        heading: 'The Canvas',
        description:
          'Smooth zoom and pan up to 64×, with pixel-perfect rendering. Cursor-anchored wheel zoom, auto-fit on open, and WebGPU rendering with a Canvas2D fallback.',
        shortcuts: ['zoom', 'pan', 'fit to view'],
      },
      tools: {
        heading: 'Tools',
        description:
          'Pencil, eraser, bucket, line, rectangles, ellipses, rounded rectangles, polygons, eyedropper, move, text, and rectangle / ellipse / lasso / polygon / magic-wand selections. Each tool is small, focused, and built around pixel-perfect input.',
        shortcuts: ['pencil', 'eraser', 'bucket', 'eyedropper', 'move', 'selection', 'shapes'],
      },
      layers: {
        heading: 'Layers',
        description:
          'Image layers, group layers, blend modes, opacity, visibility and locking, with thumbnails and drag-to-reorder. The Aseprite layer model — round-trips through Pincel and back into Aseprite without loss.',
        shortcuts: [],
      },
      animation: {
        heading: 'Frames & Animation',
        description:
          'A timeline with per-frame durations, named tags with forward / reverse / ping-pong playback, onion skin, and sprite-sheet export with Aseprite-format JSON. Everything round-trips through the .aseprite file.',
        shortcuts: ['prev/next frame', 'play / pause'],
      },
      tilemaps: {
        heading: 'Tilemaps',
        description:
          'First-class tilemap layers. Build a tileset, stamp tiles on the canvas, flip and rotate per-tile, and edit a tile in place to update everywhere it appears.',
        shortcuts: [],
      },
      slices: {
        heading: 'Slices',
        description:
          'Define named rectangular regions, optionally with 9-patch borders and pivot points. Per-frame keys for animated regions. Your engine reads them straight out of the .aseprite file.',
        shortcuts: [],
      },
      palette: {
        heading: 'Palette',
        description:
          'New sprites start with DB32. Switch to DB16, PICO-8, Game Boy or NES in one click, import and export GIMP .gpl palettes, and recolor, add, delete and drag-reorder entries; named entries survive the .aseprite round-trip.',
        shortcuts: [],
      },
      'file-format': {
        heading: 'File Format',
        description:
          'Aseprite read and write are the source of truth — layer flags, user data and grid settings survive the trip. PNG frame export and sprite sheets with Aseprite-compatible JSON. Image mode saves OpenRaster (.ora). No proprietary format lock-in, ever.',
        shortcuts: [],
      },
      tablet: {
        heading: 'Tablet & Pen',
        description:
          'Pointer events throughout, two-finger pan and pinch-to-zoom in both modes. On-screen Shift and Alt toggles stand in for the keyboard, and the Image-mode brush follows pen pressure and tilt.',
        shortcuts: [],
      },
      'image-mode': {
        heading: 'Image Mode',
        description:
          'A paint.net-style editor in the same app for everything that is not pixel art: PNG, JPEG, WebP, BMP, GIF and TIFF open with layers, blend modes, brushes, shapes, text, selections, transforms, and the full effects and adjustments set — limited to the selection when there is one. Large canvases render viewport-first. Save as OpenRaster to keep the layers, export flat images.',
        shortcuts: [],
      },
      pwa: {
        heading: 'PWA',
        description:
          'Install Pincel from your browser. It works offline, autosaves to IndexedDB, and keeps your files on your device. Use the File System Access API where available; fall back to download otherwise. The editor speaks English and German.',
        shortcuts: [],
      },
      roadmap: {
        heading: "What's Coming",
        description:
          'Native desktop builds via Tauri. Lua scripting. Custom brushes. Headless Node API for asset pipelines. A read-only viewer for embeds. We ship in small, honest increments.',
        shortcuts: [],
      },
    },
  },
  about: {
    seoTitle: 'About — Pincel',
    seoDescription:
      'Pincel is part of amigo-labs, an open-source ecosystem of game-development tools built in Rust. Free, MIT or Apache 2.0, no funding round, no subscription tier.',
    caption: 'About',
    title: 'A pixel-art editor — and an image editor — with a long view.',
    p1: 'Pincel is part of [amigo-labs](https://amigo-labs.dev), an open-source ecosystem of game-development tools built in Rust. The flagship is amigo-engine — a deterministic ECS-based 2D engine with a built-in level editor, AI asset pipelines, and chiptune music tooling. Pincel fills the pixel-art-editor gap in that ecosystem, and stands alone for anyone outside it. Its Image mode began as the sister project Fineliner, a paint.net-style editor; the two now share one app, one effects engine and one release.',
    p2: "The project is open source under MIT or Apache 2.0 — your choice. The format we use is Aseprite's, which is documented and reimplementable. We are not affiliated with the Aseprite project, and we don't compete with it: Aseprite has 14 years of refinement and a deep feature set we won't catch up to overnight. Pincel exists for people who want a free, web-based, embeddable alternative.",
    p3: 'No funding round. No subscription tier coming. No "open core" gotcha. The plan is to keep the tool good and the codebase clean, and to maintain it for as long as people use it.',
    builtTitle: 'Built with Pincel itself',
    built:
      'Every pixel-art element on this site — the logo, the hero scene, the icons in the feature grid — was made in Pincel. The site is dogfood proof. When we change something in the editor, this site is one of the first places we notice if it broke.',
    teamTitle: 'The team',
    team: 'Pincel is currently maintained by **amigo-labs**. Contributors are welcome — see the [GitHub repo](https://github.com/amigo-labs/amigo-pincel) for open issues and the contributing guide.',
    contribute: 'Contribute on GitHub',
  },
  embed: {
    seoTitle: 'Embed Pincel — For developers',
    seoDescription:
      '@amigo-labs/pincel is a framework-agnostic npm package. Drop a full pixel-art editor into your tool with one mount call. Read and write Aseprite files.',
    caption: 'For tool makers',
    title: 'Embed Pincel in your tool.',
    lede: 'Pincel ships as `@amigo-labs/pincel`, a framework-agnostic npm package. Mount it in any DOM element. Read and write Aseprite files. Listen for changes. Built on `pincel-core` (Rust → WASM). Same editor, in your app, no iframe.',
    quickStart: 'Quick start',
    demoTitle: 'Live demo',
    demoLabel: 'Embedded Pincel demo',
    demoSoon: 'Live embed loads here at launch.',
    demoFollow:
      "The editor itself isn't published yet — follow along on [GitHub](https://github.com/amigo-labs/amigo-pincel).",
    viewSource: 'View source',
    getTitle: 'What you get',
    get: [
      'Full editor UI, themeable via documented CSS variables.',
      'Public events: `change`, `save`, `selection`, `tool-change`, `palette-change`.',
      'Imperative API: `loadFile`, `saveAseprite`, `exportPng`, `setActiveLayer`, and more.',
      'TypeScript types shipped.',
      'Measured bundle size published per release.',
      'Works in any framework. React adapter, Svelte adapter, plain JS.',
    ],
    costTitle: 'What it costs',
    cost: "Free, MIT or Apache 2.0. No telemetry, no licensing, no per-seat fees. If you ship a product using Pincel, you don't owe us anything. If you want to credit us, that's nice.",
    limitsTitle: 'Honest limits',
    limits: [
      "Bundle is not tiny — it's an editor. For small embeds, consider a read-only viewer (Phase 2).",
      'No headless / Node API in Phase 1 (browser-only). Phase 2 adds Node for pipelines.',
      'Aseprite Lua scripts are not supported and not planned.',
    ],
    source: 'Source on GitHub',
    coming: "See what's coming",
  },
};

/** The site's copy, in one language. */
export type Messages = typeof en;
