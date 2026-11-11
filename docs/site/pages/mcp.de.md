`pincel-mcp` ist ein Server für das [Model Context Protocol](https://modelcontextprotocol.io) ohne Oberfläche. Claude Code, Claude Desktop oder jeder andere MCP-Client kann darüber Dokumente anlegen, zeichnen, als Vorschau ansehen und exportieren: Pixel-Dokumente (`.aseprite`, PNG, Sprite-Sheets) und Bild-Dokumente (PNG, JPEG, WebP, BMP, GIF, TIFF, OpenRaster `.ora`).

Dokumente leben im Arbeitsspeicher. Dateien werden nur in dem Arbeitsordner gelesen und geschrieben, den du angibst.

## Installieren

Lade `pincel-mcp-windows-x64.exe`, `pincel-mcp-macos-universal` oder `pincel-mcp-linux-x64` aus dem [neuesten Release](https://github.com/amigo-labs/amigo-pincel/releases/latest). Benenne die Datei in `pincel-mcp` um (`pincel-mcp.exe` unter Windows), mache sie ausführbar (`chmod +x pincel-mcp`) und lege sie in deinen `PATH`.

Oder baue es aus einem Checkout des Repositorys:

```bash
cargo install --path crates/pincel-mcp
```

## Anmelden

Bei Claude Code, mit dem aktuellen Projekt als Arbeitsordner:

```bash
claude mcp add pincel -- pincel-mcp --workspace .
```

| Option | Bedeutung |
|--------|-----------|
| `--workspace <dir>` | Der Ordner, in dem Dateien gelesen und geschrieben werden dürfen. Standard: der aktuelle Ordner. |
| `--read-only` | Kein Speichern und kein Export (`doc.save`, `export.*`). |

## Was der Agent bekommt

- **Werkzeuge** nach dem Muster `bereich.verb`, zum Beispiel `doc.new`, `draw.pixels`, `doc.preview`, `export.png`. `tools/list` beschreibt jedes einzelne.
- **Ressourcen:** `pincel://guide` (Anleitung zu den Werkzeugen, für das Modell geschrieben), `pincel://effects` (Parameter der Effekte und Korrekturen) und die eingebauten Paletten unter `pincel://palettes/{id}`.
