// Deutsche UI-Texte (spec §9.5). Typisiert gegen `en.ts`: ein fehlender
// Schlüssel ist ein Fehler in `pnpm check`.

import type { MessageKey } from './en';

export const de: Record<MessageKey, string> = {
  // Shell
  'shell.discardPrompt': 'Ungespeicherte Änderungen am offenen Dokument verwerfen?',
  'shell.unsavedChanges': 'Ungespeicherte Änderungen',
  'shell.discard': 'Änderungen verwerfen',
  'shell.unsupported':
    '{name}: kein unterstütztes Format (Sprite, OpenRaster oder PNG / JPEG / WebP / BMP / GIF / TIFF)',
  'shell.openFailed': 'Öffnen fehlgeschlagen: {error}',
  'shell.permissionDenied': '{name}: Zugriff verweigert',
  'shell.noHandle': '{name}: kein Datei-Handle / Pfad',
  'shell.recentFailed': '{name} konnte nicht geöffnet werden: {error}',
  'shell.dropFailed': 'Ablegen fehlgeschlagen: {error}',
  'shell.loadingEditor': '{mode}-Editor wird geladen…',
  'shell.editorFailed': 'Der {mode}-Editor konnte nicht geladen werden: {error}',
  'shell.dismiss': 'Schließen',

  // Generic buttons
  'common.cancel': 'Abbrechen',
  'common.ok': 'OK',
  'common.apply': 'Anwenden',
  'common.create': 'Erstellen',
  'common.close': 'Schließen',
  'common.width': 'Breite',
  'common.height': 'Höhe',

  // Modes
  'mode.pixel': 'Pixel',
  'mode.image': 'Bild',

  // Start screen
  'start.tagline':
    'Pixel-Art-Sprites und alltägliche Bildbearbeitung in einer App. Datei hierher ziehen, um sie zu öffnen.',
  'start.newPixel': 'Neues Pixel-Sprite',
  'start.newPixelHint': 'Frames, Paletten, Tilemaps, Slices. Speichert als .aseprite.',
  'start.newImage': 'Neues Bild',
  'start.newImageHint':
    'Ebenen, weiche Pinsel, Auswahlen, Effekte. Speichert als .ora; exportiert PNG / JPEG / WebP.',
  'start.open': 'Öffnen…',
  'start.openHint':
    '.aseprite öffnet im Pixel-Modus; .ora, PNG, JPEG, WebP, BMP, GIF und TIFF öffnen im Bild-Modus.',
  'start.recent': 'Zuletzt geöffnet',
  'start.recentFiles': 'Zuletzt geöffnete Dateien',
  'start.language': 'Sprache',

  // New document dialog
  'newDoc.title': 'Neues Dokument',
  'newDoc.type': 'Dokumenttyp',
  'newDoc.pixel': 'Pixel-Sprite',
  'newDoc.image': 'Bild',
  'newDoc.w': 'B',
  'newDoc.h': 'H',

  // Recovery dialog
  'recovery.title': 'Ungespeicherte Arbeit wiederherstellen?',
  'recovery.count': '{count} Sicherung(en)',
  'recovery.body':
    'Pincel hat automatische Sicherungen aus einer früheren Sitzung gefunden. Stelle eine Sicherung wieder her, um weiterzuarbeiten, oder verwirf sie endgültig.',
  'recovery.recover': 'Wiederherstellen',
  'recovery.discard': 'Verwerfen',
  'recovery.notNow': 'Später',

  // File association dialog
  'fileAssoc.title': 'Sprites und Bilder mit Pincel öffnen',
  'fileAssoc.body':
    'Pincel bearbeitet Sprites im Aseprite-Format (Pixel-Modus) sowie PNG-, JPEG-, WebP-, BMP-, GIF- und TIFF-Bilder (Bild-Modus). So wird Pincel Standard-App für .aseprite / .ase – und auf Wunsch für die Bildformate:',
  'fileAssoc.mac1': 'Im Finder auf eine .aseprite-Datei rechtsklicken → Informationen.',
  'fileAssoc.mac2': 'Unter „Öffnen mit“ Pincel wählen.',
  'fileAssoc.mac3': '„Alle ändern…“ klicken, um es für alle .aseprite-Dateien zu übernehmen.',
  'fileAssoc.win1': 'Einstellungen → Apps → Standard-Apps öffnen.',
  'fileAssoc.win2': 'Nach „.aseprite“ suchen und Pincel aus der Liste wählen.',
  'fileAssoc.win3': 'Für „.ase“ und gewünschte Bildformate wiederholen.',
  'fileAssoc.linux1': '{command} ausführen.',
  'fileAssoc.linux2':
    'Falls deine Distribution neue MIME-Typen nicht automatisch erkennt, danach {command} ausführen.',
  'fileAssoc.other':
    'Die Schritte hängen vom Betriebssystem ab – siehe dessen Dokumentation zu Standard-Apps.',
  'fileAssoc.dontShow': 'Nicht mehr anzeigen',
  'fileAssoc.gotIt': 'Verstanden',

  // Editor frame
  'frame.unsaved': '{title} – ungespeicherte Änderungen',
  'frame.toolOptions': 'Werkzeugoptionen',
  'frame.panels': 'Bedienfelder',
  'frame.tools': 'Werkzeuge',
  'frame.menu': 'Menü',
  'frame.closeMenu': 'Menü schließen',
  'frame.variant': '{group}-Variante',
  'frame.togglePanels': 'Bedienfelder ein/aus',
  'frame.closePanels': 'Bedienfelder schließen',

  // Zoom
  'zoom.label': 'Zoom',
  'zoom.fit': 'An Fenster anpassen',
  'zoom.actual': 'Originalgröße',
  'zoom.in': 'Vergrößern',
  'zoom.out': 'Verkleinern',

  // Colors
  'color.fg': 'Vordergrundfarbe',
  'color.bg': 'Hintergrundfarbe',
  'color.swap': 'Farben tauschen',
  'color.swapLabel': 'Vorder- und Hintergrundfarbe tauschen',
  'color.reset': 'Standardfarben',
  'color.resetLabel': 'Auf Standardfarben zurücksetzen',
  'color.hex': 'Hex',
  'color.hexLabel': 'Hex-Wert der Vordergrundfarbe',
  'color.alpha': 'Deckkraft der Vordergrundfarbe',

  // Language menu
  'menu.language': 'Sprache',

  // Layers
  'layers.add': 'Ebene hinzufügen',
  'layers.duplicate': 'Ebene duplizieren',
  'layers.mergeDown': 'Nach unten vereinen',
  'layers.delete': 'Ebene löschen',
  'layers.none': 'Keine Ebenen.',
  'layers.list': 'Ebenenliste',
  'layers.move': '{name} zum Umsortieren ziehen',
  'layers.moveHint': 'Ziehen oder Pfeiltasten zum Umsortieren',
  'layers.hide': '{name} ausblenden',
  'layers.show': '{name} einblenden',
  'layers.lock': '{name} sperren',
  'layers.unlock': '{name} entsperren',
  'layers.lockedByGroup': 'Durch die Gruppe gesperrt',
  'layers.renameTitle': 'Ebene umbenennen',
  'layers.name': 'Name',
  'menu.renameLayer': 'Umbenennen…',
  'layers.rename': 'Umbenennen',
  'layers.renameLabel': '{name} umbenennen',
  'layers.properties': 'Ebeneneigenschaften',
  'layers.opacity': 'Deckkraft',
  'layers.opacityLabel': 'Deckkraft der Ebene',
  'layers.blend': 'Modus',
  'layers.blendLabel': 'Mischmodus der Ebene',
  'layers.mergeVisible': 'Sichtbare vereinen',
  'layers.flatten': 'Auf eine Ebene reduzieren',
  'layers.kind.tilemap': 'Tilemap',
  'layers.kind.group': 'Gruppe',
  'layers.locked': 'Ebene „{name}“ ist gesperrt',
  'blend.normal': 'Normal',
  'blend.multiply': 'Multiplizieren',
  'blend.screen': 'Negativ multiplizieren',
  'blend.overlay': 'Ineinanderkopieren',
  'blend.darken': 'Abdunkeln',
  'blend.lighten': 'Aufhellen',
  'blend.color_dodge': 'Farbig abwedeln',
  'blend.color_burn': 'Farbig nachbelichten',
  'blend.hard_light': 'Hartes Licht',
  'blend.soft_light': 'Weiches Licht',
  'blend.difference': 'Differenz',
  'blend.exclusion': 'Ausschluss',
  'blend.hue': 'Farbton',
  'blend.saturation': 'Sättigung',
  'blend.color': 'Farbe',
  'blend.luminosity': 'Luminanz',
  'blend.addition': 'Addieren',
  'blend.subtract': 'Subtrahieren',
  'blend.divide': 'Dividieren',

  // On-screen modifiers
  'mods.label': 'Sondertasten',
  'mods.shift': 'Umschalt',
  'mods.alt': 'Alt',
  'mods.shiftHint': 'Umschalt: Quadrat/Kreis, 45°-Linien, zur Auswahl hinzufügen',
  'mods.altHint': 'Alt: von der Auswahl abziehen (mit Umschalt: Schnittmenge)',
};
