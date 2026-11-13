//! Native menu labels in the six UI languages (spec §9.5,
//! docs/specs/more-languages.md AC-11). The renderer owns the language
//! choice and pushes it through `set_menu_language`; an unknown language
//! code gives English labels.

use std::sync::atomic::{AtomicUsize, Ordering};

/// Native menu labels in one UI language (spec §9.5). The renderer owns
/// the language choice and pushes it through `set_menu_language`.
pub struct Labels {
    pub file: &'static str,
    pub new: &'static str,
    pub open: &'static str,
    pub open_recent: &'static str,
    pub no_recent: &'static str,
    pub save: &'static str,
    pub save_as: &'static str,
    pub close: &'static str,
    pub export_png: &'static str,
    pub export_sheet: &'static str,
    pub export_animation: &'static str,
    pub export_svg: &'static str,
    pub print: &'static str,
    pub quit: &'static str,
    pub edit: &'static str,
    pub undo: &'static str,
    pub redo: &'static str,
    pub cut: &'static str,
    pub copy: &'static str,
    pub paste: &'static str,
    /// Edit ▸ Settings… (docs/specs/settings-dialog.md).
    pub settings: &'static str,
    pub view: &'static str,
    pub zoom_in: &'static str,
    pub zoom_out: &'static str,
    pub reset_zoom: &'static str,
    pub help: &'static str,
    pub shortcuts: &'static str,
    pub tour: &'static str,
    pub about: &'static str,
    pub check_updates: &'static str,
}

pub const LABELS_EN: Labels = Labels {
    file: "File",
    new: "New",
    open: "Open\u{2026}",
    open_recent: "Open Recent",
    no_recent: "(no recent files)",
    save: "Save",
    save_as: "Save As\u{2026}",
    close: "Close",
    export_png: "Export as PNG\u{2026}",
    export_sheet: "Export Sprite Sheet\u{2026}",
    export_animation: "Export Animation\u{2026}",
    export_svg: "Export as SVG\u{2026}",
    print: "Print\u{2026}",
    quit: "Quit",
    edit: "Edit",
    undo: "Undo",
    redo: "Redo",
    cut: "Cut",
    copy: "Copy",
    paste: "Paste",
    settings: "Settings\u{2026}",
    view: "View",
    zoom_in: "Zoom In",
    zoom_out: "Zoom Out",
    reset_zoom: "Reset Zoom",
    help: "Help",
    shortcuts: "Keyboard Shortcuts",
    tour: "Tour",
    about: "About Pincel",
    check_updates: "Check for Updates\u{2026}",
};

pub const LABELS_DE: Labels = Labels {
    file: "Datei",
    new: "Neu",
    open: "\u{d6}ffnen\u{2026}",
    open_recent: "Zuletzt ge\u{f6}ffnet",
    no_recent: "(keine zuletzt ge\u{f6}ffneten Dateien)",
    save: "Speichern",
    save_as: "Speichern unter\u{2026}",
    close: "Schlie\u{df}en",
    export_png: "Als PNG exportieren\u{2026}",
    export_sheet: "Sprite-Sheet exportieren\u{2026}",
    export_animation: "Animation exportieren\u{2026}",
    export_svg: "Als SVG exportieren\u{2026}",
    print: "Drucken\u{2026}",
    quit: "Beenden",
    edit: "Bearbeiten",
    undo: "R\u{fc}ckg\u{e4}ngig",
    redo: "Wiederholen",
    cut: "Ausschneiden",
    copy: "Kopieren",
    paste: "Einf\u{fc}gen",
    settings: "Einstellungen\u{2026}",
    view: "Ansicht",
    zoom_in: "Vergr\u{f6}\u{df}ern",
    zoom_out: "Verkleinern",
    reset_zoom: "Zoom zur\u{fc}cksetzen",
    help: "Hilfe",
    shortcuts: "Tastenk\u{fc}rzel",
    tour: "Rundgang",
    about: "\u{dc}ber Pincel",
    check_updates: "Nach Updates suchen\u{2026}",
};

pub const LABELS_ES: Labels = Labels {
    file: "Archivo",
    new: "Nuevo",
    open: "Abrir\u{2026}",
    open_recent: "Abrir recientes",
    no_recent: "(no hay archivos recientes)",
    save: "Guardar",
    save_as: "Guardar como\u{2026}",
    close: "Cerrar",
    export_png: "Exportar como PNG\u{2026}",
    export_sheet: "Exportar hoja de sprites\u{2026}",
    export_animation: "Exportar animaci\u{f3}n\u{2026}",
    export_svg: "Exportar como SVG\u{2026}",
    print: "Imprimir\u{2026}",
    quit: "Salir",
    edit: "Edici\u{f3}n",
    undo: "Deshacer",
    redo: "Rehacer",
    cut: "Cortar",
    copy: "Copiar",
    paste: "Pegar",
    settings: "Ajustes\u{2026}",
    view: "Ver",
    zoom_in: "Acercar",
    zoom_out: "Alejar",
    reset_zoom: "Restablecer zoom",
    help: "Ayuda",
    shortcuts: "Atajos de teclado",
    tour: "Recorrido",
    about: "Acerca de Pincel",
    check_updates: "Buscar actualizaciones\u{2026}",
};

pub const LABELS_JA: Labels = Labels {
    file: "\u{30d5}\u{30a1}\u{30a4}\u{30eb}",
    new: "\u{65b0}\u{898f}",
    open: "\u{958b}\u{304f}\u{2026}",
    open_recent: "\u{6700}\u{8fd1}\u{4f7f}\u{3063}\u{305f}\u{30d5}\u{30a1}\u{30a4}\u{30eb}",
    no_recent: "\u{ff08}\u{6700}\u{8fd1}\u{306e}\u{30d5}\u{30a1}\u{30a4}\u{30eb}\u{306f}\u{3042}\u{308a}\u{307e}\u{305b}\u{3093}\u{ff09}",
    save: "\u{4fdd}\u{5b58}",
    save_as: "\u{540d}\u{524d}\u{3092}\u{4ed8}\u{3051}\u{3066}\u{4fdd}\u{5b58}\u{2026}",
    close: "\u{9589}\u{3058}\u{308b}",
    export_png: "PNG\u{3068}\u{3057}\u{3066}\u{66f8}\u{304d}\u{51fa}\u{3057}\u{2026}",
    export_sheet: "\u{30b9}\u{30d7}\u{30e9}\u{30a4}\u{30c8}\u{30b7}\u{30fc}\u{30c8}\u{3092}\u{66f8}\u{304d}\u{51fa}\u{3057}\u{2026}",
    export_animation: "\u{30a2}\u{30cb}\u{30e1}\u{30fc}\u{30b7}\u{30e7}\u{30f3}\u{3092}\u{66f8}\u{304d}\u{51fa}\u{3057}\u{2026}",
    export_svg: "SVG\u{3068}\u{3057}\u{3066}\u{66f8}\u{304d}\u{51fa}\u{3057}\u{2026}",
    print: "\u{5370}\u{5237}\u{2026}",
    quit: "\u{7d42}\u{4e86}",
    edit: "\u{7de8}\u{96c6}",
    undo: "\u{53d6}\u{308a}\u{6d88}\u{3057}",
    redo: "\u{3084}\u{308a}\u{76f4}\u{3057}",
    cut: "\u{5207}\u{308a}\u{53d6}\u{308a}",
    copy: "\u{30b3}\u{30d4}\u{30fc}",
    paste: "\u{8cbc}\u{308a}\u{4ed8}\u{3051}",
    settings: "\u{8a2d}\u{5b9a}\u{2026}",
    view: "\u{8868}\u{793a}",
    zoom_in: "\u{30ba}\u{30fc}\u{30e0}\u{30a4}\u{30f3}",
    zoom_out: "\u{30ba}\u{30fc}\u{30e0}\u{30a2}\u{30a6}\u{30c8}",
    reset_zoom: "\u{30ba}\u{30fc}\u{30e0}\u{3092}\u{30ea}\u{30bb}\u{30c3}\u{30c8}",
    help: "\u{30d8}\u{30eb}\u{30d7}",
    shortcuts: "\u{30ad}\u{30fc}\u{30dc}\u{30fc}\u{30c9}\u{30b7}\u{30e7}\u{30fc}\u{30c8}\u{30ab}\u{30c3}\u{30c8}",
    tour: "\u{30c4}\u{30a2}\u{30fc}",
    about: "Pincel\u{306b}\u{3064}\u{3044}\u{3066}",
    check_updates: "\u{30a2}\u{30c3}\u{30d7}\u{30c7}\u{30fc}\u{30c8}\u{3092}\u{78ba}\u{8a8d}\u{2026}",
};

pub const LABELS_FR: Labels = Labels {
    file: "Fichier",
    new: "Nouveau",
    open: "Ouvrir\u{2026}",
    open_recent: "Ouvrir un fichier r\u{e9}cent",
    no_recent: "(aucun fichier r\u{e9}cent)",
    save: "Enregistrer",
    save_as: "Enregistrer sous\u{2026}",
    close: "Fermer",
    export_png: "Exporter en PNG\u{2026}",
    export_sheet: "Exporter la planche de sprites\u{2026}",
    export_animation: "Exporter l\u{2019}animation\u{2026}",
    export_svg: "Exporter en SVG\u{2026}",
    print: "Imprimer\u{2026}",
    quit: "Quitter",
    edit: "\u{c9}dition",
    undo: "Annuler",
    redo: "R\u{e9}tablir",
    cut: "Couper",
    copy: "Copier",
    paste: "Coller",
    settings: "R\u{e9}glages\u{2026}",
    view: "Affichage",
    zoom_in: "Zoom avant",
    zoom_out: "Zoom arri\u{e8}re",
    reset_zoom: "R\u{e9}initialiser le zoom",
    help: "Aide",
    shortcuts: "Raccourcis clavier",
    tour: "Visite guid\u{e9}e",
    about: "\u{c0} propos de Pincel",
    check_updates: "Rechercher des mises \u{e0} jour\u{2026}",
};

pub const LABELS_PT_BR: Labels = Labels {
    file: "Arquivo",
    new: "Novo",
    open: "Abrir\u{2026}",
    open_recent: "Abrir recentes",
    no_recent: "(nenhum arquivo recente)",
    save: "Salvar",
    save_as: "Salvar como\u{2026}",
    close: "Fechar",
    export_png: "Exportar como PNG\u{2026}",
    export_sheet: "Exportar folha de sprites\u{2026}",
    export_animation: "Exportar anima\u{e7}\u{e3}o\u{2026}",
    export_svg: "Exportar como SVG\u{2026}",
    print: "Imprimir\u{2026}",
    quit: "Sair",
    edit: "Editar",
    undo: "Desfazer",
    redo: "Refazer",
    cut: "Recortar",
    copy: "Copiar",
    paste: "Colar",
    settings: "Configura\u{e7}\u{f5}es\u{2026}",
    view: "Exibir",
    zoom_in: "Ampliar",
    zoom_out: "Reduzir",
    reset_zoom: "Redefinir zoom",
    help: "Ajuda",
    shortcuts: "Atalhos de teclado",
    tour: "Tour",
    about: "Sobre o Pincel",
    check_updates: "Verificar atualiza\u{e7}\u{f5}es\u{2026}",
};

/// The label sets by language: English first, the fallback.
const ALL: [(&str, &Labels); 6] = [
    ("en", &LABELS_EN),
    ("de", &LABELS_DE),
    ("es", &LABELS_ES),
    ("ja", &LABELS_JA),
    ("fr", &LABELS_FR),
    ("pt-BR", &LABELS_PT_BR),
];

/// The label set of a renderer language code; English for any other.
#[cfg(test)]
fn labels_for(lang: &str) -> &'static Labels {
    ALL.iter()
        .find(|(code, _)| *code == lang)
        .map_or(&LABELS_EN, |(_, labels)| labels)
}

/// The language the native menu shows (an index into `ALL`).
static CURRENT: AtomicUsize = AtomicUsize::new(0);

/// Makes `lang` the native menu's language.
pub fn set_language(lang: &str) {
    let index = ALL.iter().position(|(code, _)| *code == lang).unwrap_or(0);
    CURRENT.store(index, Ordering::Relaxed);
}

/// The labels of the native menu's language.
pub fn current() -> &'static Labels {
    ALL.get(CURRENT.load(Ordering::Relaxed))
        .map_or(&LABELS_EN, |(_, labels)| labels)
}

#[cfg(test)]
mod tests {
    use super::*;

    // AC-11: each of the six languages has its own labels; anything else
    // is English.
    #[test]
    fn labels_for_each_language_with_english_fallback() {
        assert_eq!(labels_for("en").file, "File");
        assert_eq!(labels_for("de").file, "Datei");
        assert_eq!(labels_for("es").file, "Archivo");
        assert_eq!(labels_for("ja").file, "\u{30d5}\u{30a1}\u{30a4}\u{30eb}");
        assert_eq!(labels_for("fr").file, "Fichier");
        assert_eq!(labels_for("pt-BR").file, "Arquivo");
        assert_eq!(labels_for("it").file, "File");
        assert_eq!(labels_for("").file, "File");
    }

    #[test]
    fn set_language_switches_the_current_labels() {
        set_language("fr");
        assert_eq!(current().quit, "Quitter");
        set_language("xx");
        assert_eq!(current().quit, "Quit");
    }

    // No label set leaves an entry empty.
    #[test]
    fn every_label_set_is_complete() {
        for (code, l) in ALL {
            for v in [
                l.file,
                l.new,
                l.open,
                l.open_recent,
                l.no_recent,
                l.save,
                l.save_as,
                l.close,
                l.export_png,
                l.export_sheet,
                l.export_animation,
                l.export_svg,
                l.print,
                l.quit,
                l.edit,
                l.undo,
                l.redo,
                l.cut,
                l.copy,
                l.paste,
                l.settings,
                l.view,
                l.zoom_in,
                l.zoom_out,
                l.reset_zoom,
                l.help,
                l.shortcuts,
                l.tour,
                l.about,
                l.check_updates,
            ] {
                assert!(!v.is_empty(), "{code}");
            }
        }
    }
}
