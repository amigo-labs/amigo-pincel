# NEXT-PROMPTS — amigo-pincel

Prompts zum Weiterarbeiten in Claude Code. Stand: 20.08.2026, nach den
Aufräum-Commits.

## Entschieden

Pincel wird das Produkt. In zwei Stufen:

1. **Jetzt: eine App, zwei Modi.** amigo-fineliner wird der Paint-Modus von
   Pincel. Die Rust-Cores bleiben getrennt — verschiedene Domänen, und die
   Blend-Mathematik hat unvereinbare Anforderungen (u8/sRGB für Aseprite-Treue
   gegen f32/linear für W3C-Korrektheit).
2. **Danach: die Hülle als geteiltes Package.** `fs`/`idb`/PWA/Tauri werden
   aus der vereinheitlichten App herausgelöst, damit weitere Tools sie
   mitbenutzen können — unter *eigenen* Namen, nicht als Pincel-Modi. Pincel
   bleibt ein Werkzeugname, `amigo-labs` ist das Dach.

Was ausdrücklich **nicht** unter Pincel läuft: amigo-trommel (Game-Audio,
andere Domäne), die Engine (Bibliothek, kein Produkt).

## Stand

Gelandet am 20.08.2026:

- `render/compose.rs` in Module aufgeteilt: `error.rs`, `blend.rs`,
  `image_layer.rs`, `tilemap_layer.rs`, `test_support.rs`. Öffentliche Fläche
  unverändert.
- `codec/png.rs`: `export_frame_png`, `export_atlas_png` mit Manifest, plus
  `Document::exportPng` / `Document::exportAtlas` in `pincel-wasm`.
- `.gitattributes` pinnt LF, CLAUDE.md §6.4 hält die Konvention fest.
- Decision Log für `png 0.18` nachgetragen, STATUS hat die Export-Zeile.

Noch offen im Renderer — `compose.rs` lehnt weiter ab: `UnsupportedColorMode`,
`UnsupportedBlendMode`, `LinkedCelUnsupported`, `OnionSkinUnsupported`,
`OverlaysUnsupported`. `blend.rs` enthält bis jetzt nur `blend_normal_into`.

Test-Baseline: vor diesen Commits 429 (`cargo test --workspace`). Stell die
Zahl einmal neu fest und halte sie fest — nach jedem Merge darf sie nur nach
oben.

---

## Der Renderer — die drei laufen parallel

Das ist der Grund für die `compose.rs`-Aufteilung. Für jeden ein `git worktree
add`. Rechne mit kleinen Konflikten in der Layer-Loop-Dispatch in
`compose.rs`; merge einzeln, nicht alle drei auf einmal.

Warum das vor allem UI-Kram kommt: ein Programm, das die eigenen Dateien nicht
anzeigen kann, braucht keinen Autosave.

### 1 — Blend-Modi

```
compose() lehnt 18 von 19 Blend-Modi ab: render/compose.rs prüft auf
BlendMode::Normal und wirft sonst RenderError::UnsupportedBlendMode. Das
Sprite-Modell kann alle 19 darstellen, der Renderer einen — eine .aseprite mit
einem Multiply-Layer lässt sich lesen und schreiben, aber nicht anzeigen.

Implementier die fehlenden Modi in render/blend.rs. Reihenfolge: erst die 12
separablen (Multiply bis Exclusion), dann Addition/Subtract/Divide, dann die
vier non-separablen (Hue/Saturation/Color/Luminosity).

Zwei harte Vorgaben:
- Gerechnet wird in u8 und sRGB, nach Aseprite. NICHT in linearem Licht mit
  f32. Das Schwesterprojekt amigo-fineliner hat dieselben Formeln in
  f32/linear nach W3C Compositing-1 — die Formeltabelle kannst du als Referenz
  lesen, die Implementierung ist neu. Aseprite-Treue hängt genau an den
  Rundungsdetails der Integer-Mathematik.
- Wir haben KEINE von Aseprite selbst erzeugten Referenz-Fixtures. Du
  implementierst gegen die Aseprite-Formeltabelle und markierst die Tests
  ausdrücklich als vorläufig, mit einem TODO, das auf die fehlende
  Kreuzvalidierung zeigt. Behaupte in keinem Kommentar Bit-Genauigkeit, die
  nicht geprüft ist.
```

### 2 — Indexed und Grayscale

```
compose() lehnt indizierte und Graustufen-Sprites ab (render/compose.rs,
RenderError::UnsupportedColorMode). Indexed existiert heute nur im Codec,
nicht im Renderer — Pincel kann so eine Datei lesen und schreiben, aber nicht
darstellen.

Implementier Indexed und Grayscale im Compose-Pfad mit Palette-Lookup. Das
zieht die im STATUS als offene Frage geführte Entscheidung nach: SetPixel ist
heute RGBA-only und braucht für Indexed entweder ein Payload-Enum oder ein
eigenes Kommando — entscheide das begründet und schreib die Begründung ins
Decision Log, bevor du implementierst.
```

### 3 — Linked cels und Group-Layer

```
Zwei Löcher im Compose-Pfad, die in echten Aseprite-Dateien Standard sind:

1) Linked cels werden abgelehnt (RenderError::LinkedCelUnsupported). Jede
handgezeichnete Animation, die einen Frame wiederverwendet, benutzt sie.
2) Group-Layer werden übersprungen bzw. abgelehnt (LayerKind::Group).

Implementier beides. Bei Gruppen achte auf die Semantik: Sichtbarkeit und
Opazität der Gruppe wirken auf die Kinder, und die Verschachtelung muss
beliebig tief funktionieren. Prüf am .aseprite-Format nach, wie die
Gruppenzugehörigkeit kodiert ist, statt sie zu erraten.
```

### Danach, wenn 1–3 gemergt sind

```
Onion Skin und Overlays werden in compose() noch abgelehnt
(RenderError::OnionSkinUnsupported, OverlaysUnsupported), obwohl
ComposeRequest die Felder schon trägt. Implementier beides jetzt, wo
Blend-Modi, Farbmodi und linked cels da sind.
```

---

## Metropolis entblocken, ohne UI

Der Atlas-Export existiert in `pincel-core`. Ein Build-Pipeline-Konsument
braucht dafür keine Oberfläche — und jede Oberfläche, die jetzt in der alten
Hülle entsteht, wird bei der Zusammenlegung nochmal gebaut.

```
Der PNG-/Atlas-Export existiert in pincel-core (codec::png::export_frame_png
und export_atlas_png mit Manifest), ist aber nur über die wasm-Bindings
erreichbar. Ein Asset-Pipeline-Konsument braucht ihn ohne Browser.

Bau ein kleines pincel-cli als vierte Workspace-Crate: nimmt eine .aseprite,
schreibt PNG oder Atlas + Manifest-JSON. Argumente für Spaltenzahl, Tag-Filter
und Ausgabepfad. Kein Framework-Dependency-Wildwuchs — Argumente von Hand
parsen oder eine minimale, pure-Rust-Lösung, und den Decision-Log-Eintrag
nicht vergessen, falls doch eine Dependency dazukommt (CLAUDE.md §3.3, §9).

Damit kann eine externe Build-Pipeline Atlanten erzeugen, ohne dass die UI
fertig sein muss.
```

---

## Stufe 1 — die gemeinsame Hülle

Erst wenn der Renderer steht. Bewusst ein Design vor Code.

```
Entschieden: Pincel und amigo-fineliner werden eine App mit zwei Modi (Pixel /
Paint), Fineliner wird der Paint-Modus. Die Rust-Cores bleiben getrennt.

Schreib das Design-Dokument, keinen Code. Zu klären:

- Der Modus folgt dem Dokument, nicht einer Nutzereinstellung: .aseprite
  öffnet Pixel-Modus, PNG/JPEG/WebP/BMP/GIF/TIFF öffnet Paint-Modus. Eine
  echte Wahl gibt es nur bei "Neues Dokument". Beschreib die Zustandsmaschine
  und wo die Modusanzeige sitzt. Kein Startup-Dialog — der würde fragen, bevor
  man es wissen kann.
- Quergeladenes: Paint-Modus öffnet eine .aseprite (Frames, Tags, Tilesets,
  Slices fallen weg), Pixel-Modus öffnet ein PNG (Speichern geht dann nach
  .aseprite). Vorschlag für v1: kein impliziter Import, sondern ein explizites
  "Konvertieren zu…" mit Datenverlust-Warnung. Prüf den Vorschlag, statt ihn
  zu übernehmen.
- Welche Hülle die Basis wird, Modul für Modul. Fineliner hat die reichere UI
  (Layer-Panel, Effekt-Dialoge, Adjustments-Menü) und eine funktionierende
  Release-Automation plus Live-Deploy; Pincel hat fs/idb, was Fineliner fehlt.
  Keins ist eine saubere Basis — mach den Schnitt explizit.
- Was einmal statt zweimal gebaut wird: Autosave (M10.3), Service Worker
  (M10.4), Tauri (M11), das Export-Menü. Fineliner hat von den ersten drei
  keines, Pincel noch nicht. Das ist das eigentliche Argument für die
  Zusammenlegung — beziffere es.
- Aufwandsschätzung und die Reihenfolge gegen die restliche Renderer-Arbeit.
```

## Stufe 2 — das Dach herauslösen

Erst wenn Stufe 1 einmal funktioniert. Nicht vorher: eine Extraktion, die noch
nie in Betrieb war, extrahiert Vermutungen.

```
Die vereinheitlichte Hülle läuft. Löse jetzt heraus, was modusneutral und
projektneutral ist, als eigenes Package (Arbeitsname
@amigo-labs/canvas-shell): fs-Adapter, IndexedDB-Substrat, PWA-Registrierung,
Tauri-Wrapper, und was die Inventur sonst als neutral markiert hat.

Ziel ist, dass ein anderes Tool im Haus — konkret ein
Game-Audio-Authoring-Tool mit Svelte 5 und Tauri v2 — dieselbe Hülle benutzen
kann, unter eigenem Namen und mit eigenem Dokumentmodell. Prüf das gegen
dessen Anforderungen, statt eine Hülle zu bauen, die nur zu Bildbearbeitung
passt.

Schnitt-Kriterium: was das Dokumentmodell kennt, bleibt in der App. Was nur
Bytes, Dateien und Browser-APIs kennt, geht ins Package.
```

---

## Angehalten, mit Absicht

**M10.3 Autosave**, **M10.4 Service Worker** und das **Export-Menü in der UI**
sind bewusst nicht gebaut. Alle drei sind Arbeit in einer Hülle, die durch die
Zusammenlegung ersetzt wird. Sie kommen in Stufe 1 — einmal, für beide Modi.

Danach bleibt: **M11 Tauri**, **M12 Perf** (dirty rects, `dirty_hint`
verdrahten, pre-allocated Scratch in `compose()`, per-Command-Dirty-Region —
alle vier stehen gesammelt in den STATUS-„Open questions"), und **M6.7**: eine
handgemachte Datei einmal von Hand durch das echte Aseprite und zurück.

Das Gate von amigo-trommel („Pincel muss zu v1.0 sein") bleibt vorerst stehen
wie es ist. Es ist absehbar zu breit geschnitten — ein Audio-Tool wartet dort
auf Paint-Features, die es nie benutzen wird — aber Trommel ist eingefroren,
also kostet es nichts.
