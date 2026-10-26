// Deutsche Texte der Website (website spec §11). Typisiert gegen `en.ts`:
// ein fehlender oder falsch geschachtelter Schlüssel ist ein Fehler in
// `pnpm check`. Markup wie in `en.ts` (siehe `Rich.svelte`).

import type { Messages } from './en';

export const de: Messages = {
  meta: { locale: 'de_DE' },
  nav: {
    home: 'Start',
    features: 'Funktionen',
    devs: 'Für Entwickler',
    about: 'Über',
    primary: 'Hauptnavigation',
    openEditor: 'Editor öffnen',
    download: 'Download',
    language: 'Sprache',
  },
  logo: { home: 'Pincel – Startseite' },
  footer: {
    line: 'Pincel – von [amigo-labs](https://amigo-labs.dev). MIT oder Apache-2.0.',
    social: 'Soziale Netzwerke und Feeds',
    drawn: 'Bald in Pincel selbst gezeichnet.',
    crt: 'CRT-Effekt:',
    on: 'an',
    off: 'aus',
  },
  hero: {
    caption: 'Pixel-Art- & Bildeditor · v{version}',
    title: 'Ein Pixel-Art-\nund Bildeditor\nfürs Web.',
    tagline:
      'Open Source. Kostenlos. Zwei Editoren in einem: Erstelle Sprites, Tilemaps und Animationen, speichere sie Aseprite-kompatibel als `.aseprite` und ab damit ins Spiel. Bearbeite Fotos, Screenshots und Grafiken mit Ebenen, Pinseln, Auswahlen und Effekten – PNG, JPEG, WebP, TIFF und mehr. Kein Konto, kein Upload, kein Haken.',
    github: 'Auf GitHub ansehen',
    scene: 'Eine kleine blaue Figur in einem Steinraum, beleuchtet von einer flackernden Fackel.',
  },
  grid: {
    aseprite: {
      title: 'Aseprite-kompatibel',
      description: '.aseprite-Dateien lesen und schreiben. Passt direkt in deine Spiele-Pipeline.',
    },
    tilemaps: {
      title: 'Tilemaps',
      description:
        'Vollwertige Tilemap-Ebenen mit Tilesets. Stempeln, tauschen, direkt bearbeiten.',
    },
    slices: {
      title: 'Slices',
      description: 'Benannte Bereiche, 9-Patch, Drehpunkte – für Hitboxen, UI und Spiellogik.',
    },
    animation: {
      title: 'Animation',
      description: 'Frames, Tags, Zwiebelhaut, Ping-Pong. Zustände taggen, Sheets exportieren.',
    },
    palette: {
      title: 'Paletten',
      description:
        'Vorlagen für DB32, PICO-8, NES und Game Boy, Import und Export im GIMP-Format .gpl.',
    },
    tablet: {
      title: 'Tablet & Stift',
      description:
        'Pinch-Zoom, Stiftdruck, Umschalt und Alt auf dem Bildschirm. Gemacht fürs iPad.',
    },
    image: {
      title: 'Bild-Modus',
      description:
        'Ebenen, Pinsel, Auswahlen, Effekte und Korrekturen für Fotos, Screenshots und Grafiken. Speichert .ora.',
    },
    offline: {
      title: 'Offline-PWA',
      description:
        'Installieren und auch ohne Verbindung arbeiten. Deine Dateien bleiben auf deinem Gerät.',
    },
    embed: {
      title: 'Einbettbar',
      description: 'Als npm-Paket. In dein Tool einbauen. Derselbe Editor, in deiner App.',
    },
    open: {
      title: 'Open Source',
      description: 'MIT / Apache 2.0. Forkbar, prüfbar, deins.',
    },
  },
  how: {
    open: { title: '1. Öffnen', text: 'Eine `.aseprite`-Datei hineinziehen oder neu anfangen.' },
    edit: { title: '2. Bearbeiten', text: 'Malen, animieren, Tilemaps bauen, Slices festlegen.' },
    save: {
      title: '3. Speichern',
      text: 'Zurück als `.aseprite` speichern. Deine Engine lädt neu.',
    },
  },
  finalCta: {
    desktop: 'Oder als Desktop-App für Windows, macOS und Linux: [herunterladen](/download) →',
    sub: 'Oder: `npm install @amigo-labs/pincel`, um ihn [einzubetten](/embed) →',
  },
  featureSection: {
    inProgress: 'in Arbeit',
    planned: 'geplant',
    shortcuts: 'Tastenkürzel',
    screenshot: 'Screenshot',
  },
  comparison: {
    label: 'Funktionsvergleich',
    feature: 'Funktion',
    free: 'Kostenlos',
    paid: 'Kostenpflichtig',
    footnote:
      'Pincel ist neu, Aseprite hat 14 Jahre an Funktionen. Wir holen dort auf, wo es für Spieleentwicklung zählt. Wo nicht, da nicht.',
    rows: {
      format: 'Aseprite-Dateiformat (Lesen + Schreiben)',
      tilemaps: 'Tilemaps',
      slices: 'Slices (9-Patch, Drehpunkt)',
      timeline: 'Animations-Zeitleiste + Tags',
      tablet: 'Tablet- / Stift-Unterstützung',
      browser: 'Läuft im Browser',
      photo: 'Bildbearbeitung mit Ebenen (Fotos, Screenshots)',
      desktop: 'Native Desktop-App',
      library: 'Als Bibliothek einbettbar',
      scripting: 'Lua-Skripte / Erweiterungen',
      brushes: 'Eigene Pinsel',
      price: 'Preis',
      openSource: 'Open Source',
      active: 'Aktive Entwicklung',
    },
    notes: {
      native: 'nativ',
      basic: 'einfach',
      imageMode: 'Bild-Modus',
      pixelOnly: 'nur Pixel-Art',
      tauri: 'Tauri',
      nwjs: 'über NW.js, veraltet',
      phase2: 'Phase 2',
      mitApache: 'MIT/Apache',
      eula: 'EULA-beschränkt',
      apache: 'Apache 2.0',
      modernizing: 'Modernisierung läuft',
    },
  },
  home: {
    seoTitle: 'Pincel – ein Pixel-Art- und Bildeditor fürs Web',
    seoDescription:
      'Pixel-Art- und Bildeditor im Browser: Aseprite-kompatible Sprites, Tilemaps, Slices und Animation, dazu Bildbearbeitung mit Ebenen, Pinseln, Auswahlen und Effekten für Fotos und Screenshots. Open Source, kostenlos, mit Tablet-Unterstützung.',
    jsonLd:
      'Ein Pixel-Art- und Bildeditor fürs Web. Aseprite-kompatible Sprites, Tilemaps und Animationen und Bildbearbeitung mit Ebenen für PNG, JPEG, WebP und TIFF. Open Source, kostenlos.',
    pitch:
      'Pincel sind zwei Editoren in einem Browser-Tab, und die Datei entscheidet, welcher sich öffnet. Der Pixel-Modus ist für Spieleentwickler und Pixel-Künstler gemacht: Er liest und schreibt das Aseprite-Format, sodass deine Arbeit in bestehende Pipelines passt, mit Tilemaps, Slices, Animations-Tags, Paletten und indizierten Farben – das, was man wirklich braucht. Der Bild-Modus ist ein vollwertiger Editor im Stil von paint.net für alles andere – Fotos, Screenshots, UI-Entwürfe, Grafiken – mit Ebenen, Mischmodi, Pinseln, Auswahlen, Transformationen, Effekten und Korrekturen; er öffnet PNG, JPEG, WebP, BMP, GIF und TIFF und speichert OpenRaster, das auch GIMP und Krita öffnen. Beide laufen offline als PWA oder als Desktop-App, sprechen Deutsch und Englisch und funktionieren auf dem iPad. Pincel ist Open Source unter MIT oder Apache 2.0, und das bleibt so.',
    whatEyebrow: 'Was es kann',
    whatTitle: 'Vom Spiele-Asset bis zum Alltagsbild.',
    compareEyebrow: 'Ehrlicher Vergleich',
    compareTitle: 'Wo es heute steht.',
    howEyebrow: 'In drei Schritten',
    howTitle: 'So funktioniert es.',
  },
  features: {
    seoTitle: 'Funktionen – Pincel',
    seoDescription:
      'Alle Funktionen von Pincel im Detail: Arbeitsfläche, Werkzeuge, Ebenen, Animation, Tilemaps, Slices, Palette, Dateiformat, Bild-Modus, Tablet-Unterstützung, PWA und was als Nächstes kommt.',
    caption: 'Funktionen im Detail',
    title: 'Alles, was Pincel heute kann.',
    lede: 'Ausführlich, ehrlich, mit Screenshots. Wo eine Funktion noch in Arbeit ist, sagen wir das. Wo sie geplant, aber noch nicht gebaut ist, auch.',
    sections: {
      canvas: {
        heading: 'Die Arbeitsfläche',
        description:
          'Flüssiges Zoomen und Verschieben bis 64×, pixelgenau dargestellt. Mausrad-Zoom am Cursor, automatisches Einpassen beim Öffnen und WebGPU-Rendering mit Canvas2D als Rückfallebene.',
        shortcuts: ['Zoom', 'Verschieben', 'Einpassen'],
      },
      tools: {
        heading: 'Werkzeuge',
        description:
          'Stift, Radierer, Füllen, Linie, Rechtecke, Ellipsen, abgerundete Rechtecke, Polygone, Pipette, Verschieben, Text sowie Rechteck-, Ellipsen-, Lasso-, Polygon- und Zauberstab-Auswahl. Jedes Werkzeug ist klein, fokussiert und auf pixelgenaue Eingabe gebaut.',
        shortcuts: ['Stift', 'Radierer', 'Füllen', 'Pipette', 'Verschieben', 'Auswahl', 'Formen'],
      },
      layers: {
        heading: 'Ebenen',
        description:
          'Bildebenen, Gruppen, Mischmodi, Deckkraft, Sichtbarkeit und Sperre, mit Vorschaubildern und Umsortieren per Ziehen. Das Ebenenmodell von Aseprite – verlustfrei hin und zurück zwischen Pincel und Aseprite.',
        shortcuts: [],
      },
      animation: {
        heading: 'Frames & Animation',
        description:
          'Eine Zeitleiste mit Dauer pro Frame, benannte Tags mit Vorwärts-, Rückwärts- und Ping-Pong-Wiedergabe, Zwiebelhaut und Sprite-Sheet-Export mit JSON im Aseprite-Format. Alles bleibt in der .aseprite-Datei erhalten.',
        shortcuts: ['vorheriger/nächster Frame', 'Abspielen / Pause'],
      },
      tilemaps: {
        heading: 'Tilemaps',
        description:
          'Vollwertige Tilemap-Ebenen. Ein Tileset anlegen, Kacheln auf die Fläche stempeln, einzelne Kacheln spiegeln und drehen und eine Kachel direkt bearbeiten, damit sie sich überall ändert.',
        shortcuts: [],
      },
      slices: {
        heading: 'Slices',
        description:
          'Benannte rechteckige Bereiche festlegen, wahlweise mit 9-Patch-Rändern und Drehpunkten. Schlüssel pro Frame für animierte Bereiche. Deine Engine liest sie direkt aus der .aseprite-Datei.',
        shortcuts: [],
      },
      palette: {
        heading: 'Palette',
        description:
          'Neue Sprites starten mit DB32. Mit einem Klick zu DB16, PICO-8, Game Boy oder NES wechseln, GIMP-Paletten (.gpl) importieren und exportieren, Einträge umfärben, hinzufügen, löschen und per Ziehen umsortieren; benannte Einträge bleiben in der .aseprite-Datei erhalten.',
        shortcuts: [],
      },
      'file-format': {
        heading: 'Dateiformat',
        description:
          'Lesen und Schreiben von Aseprite sind die Grundlage – Ebenen-Flags, Benutzerdaten und Rastereinstellungen bleiben erhalten. PNG-Export einzelner Frames und Sprite-Sheets mit Aseprite-kompatiblem JSON. Der Bild-Modus speichert OpenRaster (.ora). Nie ein proprietäres Format.',
        shortcuts: [],
      },
      tablet: {
        heading: 'Tablet & Stift',
        description:
          'Durchgehend Pointer-Events, Verschieben und Pinch-Zoom mit zwei Fingern in beiden Modi. Umschalt- und Alt-Schalter auf dem Bildschirm ersetzen die Tastatur, und der Pinsel im Bild-Modus folgt Druck und Neigung des Stifts.',
        shortcuts: [],
      },
      'image-mode': {
        heading: 'Bild-Modus',
        description:
          'Ein Editor im Stil von paint.net in derselben App für alles, was keine Pixel-Art ist: PNG, JPEG, WebP, BMP, GIF und TIFF öffnen mit Ebenen, Mischmodi, Pinseln, Formen, Text, Auswahlen, Transformationen und allen Effekten und Korrekturen – bei aktiver Auswahl nur innerhalb der Auswahl. Große Bilder werden nur im sichtbaren Ausschnitt gerendert. Als OpenRaster speichern, um die Ebenen zu behalten, oder flach exportieren.',
        shortcuts: [],
      },
      desktop: {
        heading: 'Desktop-App',
        description:
          'Native Builds für Windows, macOS und Linux über Tauri: derselbe Editor mit den Dateidialogen des Systems, Dateizuordnungen und „Öffnen mit“ aus Explorer, Finder oder Dateimanager. Er installiert sich nur für dich, ohne Administratorrechte, und hält sich im Hintergrund aktuell – jedes Update mit geprüfter Signatur.',
        shortcuts: [],
      },
      pwa: {
        heading: 'PWA',
        description:
          'Pincel aus dem Browser installieren. Es funktioniert offline, speichert automatisch in IndexedDB und behält deine Dateien auf deinem Gerät. Wo verfügbar, nutzt es die File System Access API, sonst den Download. Der Editor spricht Deutsch und Englisch.',
        shortcuts: [],
      },
      roadmap: {
        heading: 'Was als Nächstes kommt',
        description:
          'Lua-Skripte. Eigene Pinsel. Eine Node-API ohne Oberfläche für Asset-Pipelines. Ein reiner Betrachter zum Einbetten. Wir liefern in kleinen, ehrlichen Schritten.',
        shortcuts: [],
      },
    },
  },
  download: {
    seoTitle: 'Download – Pincel',
    seoDescription:
      'Pincel, den Pixel-Art- und Bildeditor, für Windows, macOS und Linux herunterladen. Kostenlos und Open Source; installiert sich ohne Administratorrechte und aktualisiert sich selbst.',
    caption: 'Download',
    title: 'Pincel auf deinem Desktop.',
    lede: 'Version {version} für Windows, macOS und Linux. Derselbe Editor wie im Browser, als native App: Er öffnet deine Dateien direkt aus Explorer, Finder oder Dateimanager und hält sich selbst aktuell.',
    for: {
      windows: 'Für Windows herunterladen',
      macos: 'Für macOS herunterladen',
      linux: 'Für Linux herunterladen',
    },
    platforms: 'Plattformen',
    requires: 'Voraussetzung',
    windows: {
      heading: 'Windows',
      requires: 'Windows 10 oder 11, x64 (auf ARM über die eingebaute x64-Emulation).',
      setup: 'Installer (.exe)',
      msi: 'MSI-Paket',
      install:
        'Der Installer richtet Pincel nur für dich ein, in `%LOCALAPPDATA%\\Pincel`, ohne Administratorrechte und mit einem Eintrag im Startmenü. Er meldet Pincel für `.aseprite`, PNG, JPEG, WebP und die übrigen Formate an, die es öffnet; die Standard-App wählst unter Windows nur du selbst, deshalb öffnet die erste Installation Einstellungen → Standard-Apps auf der Seite von Pincel. Deinstallieren: Einstellungen → Apps → Installierte Apps. Das MSI-Paket installiert für alle Benutzer und braucht Administratorrechte.',
      firstStart:
        '**Erster Start:** Der Installer ist nicht mit einem Code-Signing-Zertifikat signiert, deshalb zeigt SmartScreen „Der Computer wurde durch Windows geschützt“. Klicke auf **Weitere Informationen**, dann auf **Trotzdem ausführen**.',
    },
    macos: {
      heading: 'macOS',
      requires: 'macOS 10.13 oder neuer; eine App für Apple Silicon und Intel.',
      dmg: 'Disk-Image (.dmg)',
      install:
        'Öffne das Disk-Image und ziehe Pincel in „Programme“. Der Finder bietet es dann unter „Öffnen mit“ für die Formate an, die es öffnet; zur Standard-App wird es über „Informationen“ → „Öffnen mit“ → „Alle ändern“.',
      firstStart:
        '**Erster Start:** Die App ist nicht von Apple notarisiert, deshalb öffnet macOS sie zunächst nicht. Klicke sie in „Programme“ mit gedrückter Ctrl-Taste an, wähle **Öffnen** und bestätige; ab macOS 15 öffnest du sie einmal und gehst dann zu Systemeinstellungen → Datenschutz & Sicherheit → **Dennoch öffnen**. Oder im Terminal: `xattr -dr com.apple.quarantine /Applications/Pincel.app`.',
    },
    linux: {
      heading: 'Linux',
      requires: 'x64 mit glibc 2.35 oder neuer: Ubuntu 22.04, Debian 12, Fedora 36 und später.',
      appimage: 'AppImage',
      deb: '.deb (Debian, Ubuntu)',
      rpm: '.rpm (Fedora, openSUSE)',
      install:
        'Das AppImage läuft so, wie es heruntergeladen wurde, sobald es ausführbar ist. Um es für dich zu installieren, ohne root – mit einem Befehl `pincel`, einem Eintrag im Anwendungsmenü und „Öffnen mit“ für seine Formate:',
      after:
        'Das kopiert es nach `~/.local/share/pincel` und verlinkt `~/.local/bin/pincel`; deinstallieren mit `pincel --uninstall`. Ab Ubuntu 24.04 brauchen AppImages `libfuse2t64` (`sudo apt install libfuse2t64`). Die Pakete .deb und .rpm installieren stattdessen über den Paketmanager und werden mit ihm aktualisiert.',
    },
    verifyTitle: 'Prüfen',
    verify:
      'Die Prüfsummen aller Dateien stehen in [SHA256SUMS]({sums}). Unter Windows: `Get-FileHash pincel-windows-x64-setup.exe`, unter macOS und Linux: `shasum -a 256` bzw. `sha256sum`.',
    updatesTitle: 'Updates',
    updates:
      'Die Desktop-App fragt höchstens einmal am Tag bei GitHub nach einer neuen Version, lädt sie im Hintergrund und prüft vor der Installation ihre Signatur; aktiv wird sie beim nächsten Start (unter Windows, sobald du Pincel beendest). Hilfe → **Nach Updates suchen…** prüft sofort, Hilfe → **Updates automatisch installieren** schaltet es ab. Die Pakete .deb und .rpm sowie ein AppImage, das nicht mit `--install` installiert wurde, überlassen Updates dir.',
    releases: 'Alle Releases und Versionshinweise',
    webTitle: 'Oder einfach im Browser',
    web: 'Nichts zu installieren: Der Editor läuft in jedem aktuellen Browser, funktioniert offline und lässt sich von dort als App (PWA) installieren. Deine Dateien bleiben so oder so auf deinem Gerät.',
  },
  about: {
    seoTitle: 'Über – Pincel',
    seoDescription:
      'Pincel gehört zu amigo-labs, einem Open-Source-Ökosystem von Werkzeugen für die Spieleentwicklung in Rust. Kostenlos, MIT oder Apache 2.0, keine Finanzierungsrunde, kein Abo.',
    caption: 'Über',
    title: 'Ein Pixel-Art-Editor – und ein Bildeditor – mit langem Atem.',
    p1: 'Pincel gehört zu [amigo-labs](https://amigo-labs.dev), einem Open-Source-Ökosystem von Werkzeugen für die Spieleentwicklung in Rust. Das Flaggschiff ist amigo-engine – eine deterministische, ECS-basierte 2D-Engine mit eingebautem Level-Editor, KI-Asset-Pipelines und Werkzeugen für Chiptune-Musik. Pincel füllt die Editor-Lücke in diesem Ökosystem – für Pixel-Art und für jedes andere Bild – und steht auch für sich allein. Sein Bild-Modus begann als Schwesterprojekt Fineliner, ein Editor im Stil von paint.net; heute teilen sich beide eine App, eine Effekt-Engine und ein Release.',
    p2: 'Das Projekt ist Open Source unter MIT oder Apache 2.0 – du hast die Wahl. Wir nutzen das Format von Aseprite, das dokumentiert ist und sich nachbauen lässt. Wir sind nicht mit dem Aseprite-Projekt verbunden und konkurrieren nicht mit ihm: Aseprite hat 14 Jahre Feinschliff und einen Funktionsumfang, den wir nicht über Nacht einholen. Pincel ist für alle da, die eine kostenlose, webbasierte und einbettbare Alternative wollen.',
    p3: 'Keine Finanzierungsrunde. Kein Abo in Sicht. Kein „Open Core“-Haken. Der Plan: das Werkzeug gut und den Code sauber halten und es pflegen, solange es Menschen benutzen.',
    builtTitle: 'Mit Pincel selbst gebaut',
    built:
      'Jedes Pixel-Art-Element dieser Website – das Logo, die Szene oben, die Symbole im Funktionsraster – entstand in Pincel. Die Website ist der Beweis. Wenn wir im Editor etwas ändern, merken wir hier als Erstes, ob etwas kaputt ist.',
    teamTitle: 'Das Team',
    team: 'Pincel wird derzeit von **amigo-labs** gepflegt. Mitwirkende sind willkommen – offene Issues und den Leitfaden findest du im [GitHub-Repository](https://github.com/amigo-labs/amigo-pincel).',
    contribute: 'Auf GitHub mitmachen',
  },
  embed: {
    seoTitle: 'Pincel einbetten – Für Entwickler',
    seoDescription:
      '@amigo-labs/pincel ist ein Framework-unabhängiges npm-Paket. Mit einem einzigen Aufruf einen vollständigen Pixel-Art-Editor in dein Tool einbauen. Aseprite-Dateien lesen und schreiben.',
    caption: 'Für Tool-Entwickler',
    title: 'Pincel in dein Tool einbetten.',
    lede: 'Pincel gibt es als `@amigo-labs/pincel`, ein Framework-unabhängiges npm-Paket. In jedes DOM-Element einhängen. Aseprite-Dateien lesen und schreiben. Auf Änderungen hören. Gebaut auf `pincel-core` (Rust → WASM). Derselbe Editor, in deiner App, ohne iframe.',
    quickStart: 'Schnellstart',
    demoTitle: 'Live-Demo',
    demoLabel: 'Eingebettete Pincel-Demo',
    demoSoon: 'Die Live-Einbettung erscheint hier zum Start.',
    demoFollow:
      'Der Editor ist noch nicht veröffentlicht – verfolge den Stand auf [GitHub](https://github.com/amigo-labs/amigo-pincel).',
    viewSource: 'Quelltext anzeigen',
    getTitle: 'Was du bekommst',
    get: [
      'Vollständige Editor-Oberfläche, gestaltbar über dokumentierte CSS-Variablen.',
      'Öffentliche Events: `change`, `save`, `selection`, `tool-change`, `palette-change`.',
      'Imperative API: `loadFile`, `saveAseprite`, `exportPng`, `setActiveLayer` und mehr.',
      'TypeScript-Typen inklusive.',
      'Gemessene Bundle-Größe zu jedem Release.',
      'Funktioniert in jedem Framework. Adapter für React und Svelte, oder reines JS.',
    ],
    costTitle: 'Was es kostet',
    cost: 'Kostenlos, MIT oder Apache 2.0. Keine Telemetrie, keine Lizenzen, keine Gebühren pro Platz. Wenn du ein Produkt mit Pincel auslieferst, schuldest du uns nichts. Eine Erwähnung freut uns trotzdem.',
    limitsTitle: 'Ehrliche Grenzen',
    limits: [
      'Das Bundle ist nicht winzig – es ist ein Editor. Für kleine Einbettungen kommt ein reiner Betrachter (Phase 2).',
      'Keine Node-API ohne Oberfläche in Phase 1 (nur Browser). Phase 2 bringt Node für Pipelines.',
      'Lua-Skripte von Aseprite werden nicht unterstützt und sind nicht geplant.',
    ],
    source: 'Quelltext auf GitHub',
    coming: 'Was als Nächstes kommt',
  },
};
