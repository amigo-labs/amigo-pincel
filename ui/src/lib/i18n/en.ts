// English UI strings — the source of every message key (spec §9.5).
// Keys are `area.name`; `{name}` marks an interpolated parameter.

export const en = {
  // Shell
  'shell.discardPrompt': 'Discard unsaved changes to the open document?',
  'shell.unsavedChanges': 'Unsaved changes',
  'shell.discard': 'Discard changes',
  'shell.unsupported':
    '{name}: not a supported file (sprite, OpenRaster or PNG / JPEG / WebP / BMP / GIF / TIFF)',
  'shell.openFailed': 'Open failed: {error}',
  'shell.permissionDenied': '{name}: permission denied',
  'shell.noHandle': '{name}: no handle / path',
  'shell.recentFailed': 'Opening {name} failed: {error}',
  'shell.dropFailed': 'Drop failed: {error}',
  'shell.loadingEditor': 'Loading the {mode} editor…',
  'shell.editorFailed': 'Failed to load the {mode} editor: {error}',
  'shell.dismiss': 'Dismiss',

  // Generic buttons
  'common.cancel': 'Cancel',
  'common.ok': 'OK',
  'common.apply': 'Apply',
  'common.create': 'Create',
  'common.close': 'Close',
  'common.width': 'Width',
  'common.height': 'Height',

  // Modes
  'mode.pixel': 'Pixel',
  'mode.image': 'Image',

  // Start screen
  'start.tagline': 'Pixel-art sprites and everyday image editing, one app. Drop a file here to open it.',
  'start.newPixel': 'New pixel sprite',
  'start.newPixelHint': 'Frames, palettes, tilemaps, slices. Saves as .aseprite.',
  'start.newImage': 'New image',
  'start.newImageHint': 'Layers, soft brushes, selections, effects. Saves as .ora; exports PNG / JPEG / WebP.',
  'start.open': 'Open…',
  'start.openHint': '.aseprite opens in Pixel mode; .ora, PNG, JPEG, WebP, BMP, GIF and TIFF open in Image mode.',
  'start.recent': 'Recent',
  'start.recentFiles': 'Recent files',
  'start.language': 'Language',

  // New document dialog
  'newDoc.title': 'New document',
  'newDoc.type': 'Document type',
  'newDoc.pixel': 'Pixel sprite',
  'newDoc.image': 'Image',
  'newDoc.w': 'W',
  'newDoc.h': 'H',

  // Recovery dialog
  'recovery.title': 'Recover unsaved work?',
  'recovery.count': '{count} snapshot(s)',
  'recovery.body':
    'Pincel found autosaved snapshots from a previous session. Recover a snapshot to continue editing, or discard it to drop it permanently.',
  'recovery.recover': 'Recover',
  'recovery.discard': 'Discard',
  'recovery.notNow': 'Not now',

  // File association dialog
  'fileAssoc.title': 'Open sprites and images with Pincel',
  'fileAssoc.body':
    'Pincel edits Aseprite-format sprites (Pixel mode) and PNG, JPEG, WebP, BMP, GIF and TIFF images (Image mode). To make it the default handler for .aseprite / .ase — and, if you like, for the image types — on your system:',
  'fileAssoc.mac1': 'Right-click an .aseprite file in Finder → Get Info.',
  'fileAssoc.mac2': 'Under “Open With”, pick Pincel.',
  'fileAssoc.mac3': 'Click “Change All…” to apply to every .aseprite file.',
  'fileAssoc.win1': 'Open Settings → Apps → Default apps.',
  'fileAssoc.win2': 'Search for “.aseprite” and pick Pincel from the list.',
  'fileAssoc.win3': 'Repeat for “.ase” and any image types you want Pincel to open.',
  'fileAssoc.linux1': 'Run {command}.',
  'fileAssoc.linux2': 'If your distro doesn’t pick up new MIME types automatically, run {command} afterward.',
  'fileAssoc.other': 'Steps depend on your OS — consult its documentation for setting default file handlers.',
  'fileAssoc.dontShow': 'Don’t show this again',
  'fileAssoc.gotIt': 'Got it',

  // Editor frame
  'frame.unsaved': '{title} — unsaved changes',
  'frame.toolOptions': 'Tool options',
  'frame.panels': 'Panels',
  'frame.tools': 'Tools',
  'frame.menu': 'Menu',
  'frame.closeMenu': 'Close menu',
  'frame.variant': '{group} variant',
  'frame.togglePanels': 'Toggle panels',
  'frame.closePanels': 'Close panels',

  // Zoom
  'zoom.label': 'Zoom',
  'zoom.fit': 'Fit to window',
  'zoom.actual': 'Actual size',
  'zoom.in': 'Zoom in',
  'zoom.out': 'Zoom out',

  // Colors
  'color.fg': 'Foreground color',
  'color.bg': 'Background color',
  'color.swap': 'Swap colors',
  'color.swapLabel': 'Swap foreground and background colors',
  'color.reset': 'Default colors',
  'color.resetLabel': 'Reset to default colors',
  'color.hex': 'Hex',
  'color.hexLabel': 'Foreground color hex value',
  'color.alpha': 'Foreground alpha',

  // Language menu
  'menu.language': 'Language',

  // Layers
  'layers.add': 'Add layer',
  'layers.duplicate': 'Duplicate layer',
  'layers.mergeDown': 'Merge down',
  'layers.delete': 'Delete layer',
  'layers.none': 'No layers.',
  'layers.list': 'Layer list',
  'layers.move': 'Drag to reorder {name}',
  'layers.moveHint': 'Drag, or use the arrow keys, to reorder',
  'layers.hide': 'Hide {name}',
  'layers.show': 'Show {name}',
  'layers.lock': 'Lock {name}',
  'layers.unlock': 'Unlock {name}',
  'layers.lockedByGroup': 'Locked by its group',
  'layers.renameTitle': 'Rename layer',
  'layers.name': 'Name',
  'menu.renameLayer': 'Rename…',
  'layers.rename': 'Rename',
  'layers.renameLabel': 'Rename {name}',
  'layers.properties': 'Layer properties',
  'layers.opacity': 'Opacity',
  'layers.opacityLabel': 'Layer opacity',
  'layers.blend': 'Blend',
  'layers.blendLabel': 'Layer blend mode',
  'layers.mergeVisible': 'Merge visible',
  'layers.flatten': 'Flatten',
  'layers.kind.tilemap': 'tilemap',
  'layers.kind.group': 'group',
  'layers.locked': 'Layer “{name}” is locked',
  'blend.normal': 'Normal',
  'blend.multiply': 'Multiply',
  'blend.screen': 'Screen',
  'blend.overlay': 'Overlay',
  'blend.darken': 'Darken',
  'blend.lighten': 'Lighten',
  'blend.color_dodge': 'Color Dodge',
  'blend.color_burn': 'Color Burn',
  'blend.hard_light': 'Hard Light',
  'blend.soft_light': 'Soft Light',
  'blend.difference': 'Difference',
  'blend.exclusion': 'Exclusion',
  'blend.hue': 'Hue',
  'blend.saturation': 'Saturation',
  'blend.color': 'Color',
  'blend.luminosity': 'Luminosity',
  'blend.addition': 'Addition',
  'blend.subtract': 'Subtract',
  'blend.divide': 'Divide',

  // On-screen modifiers
  'mods.label': 'Modifier keys',
  'mods.shift': 'Shift',
  'mods.alt': 'Alt',
  'mods.shiftHint': 'Shift: square shapes, 45° lines, add to selection',
  'mods.altHint': 'Alt: subtract from selection (with Shift: intersect)',

  // Pen pressure
  'pressure.label': 'Pen pressure',
  'pressure.size': 'Size',
  'pressure.opacity': 'Opacity',
  'pressure.hint': 'What a stylus’s pressure varies (mouse and touch paint at full pressure)',
} as const;

/** A UI message key. */
export type MessageKey = keyof typeof en;
