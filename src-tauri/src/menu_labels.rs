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
    pub quit: &'static str,
    pub edit: &'static str,
    pub cut: &'static str,
    pub copy: &'static str,
    pub paste: &'static str,
    /// App menu ▸ Settings… (docs/specs/settings-dialog.md, macos-native-menu AC-07).
    pub settings: &'static str,
    pub help: &'static str,
    pub shortcuts: &'static str,
    pub tour: &'static str,
    pub about: &'static str,
    pub check_updates: &'static str,
    /// App menu ▸ Services (macOS, docs/specs/macos-native-menu.md AC-07).
    pub services: &'static str,
    /// App menu ▸ Hide Pincel.
    pub hide: &'static str,
    /// App menu ▸ Hide Others.
    pub hide_others: &'static str,
    /// App menu ▸ Show All.
    pub show_all: &'static str,
    /// The start screen's Edit ▸ Select All (AC-09).
    pub select_all: &'static str,
}

pub const LABELS_EN: Labels = Labels {
    file: "File",
    new: "New",
    open: "Open\u{2026}",
    open_recent: "Open Recent",
    no_recent: "(no recent files)",
    quit: "Quit Pincel",
    edit: "Edit",
    cut: "Cut",
    copy: "Copy",
    paste: "Paste",
    settings: "Settings\u{2026}",
    help: "Help",
    shortcuts: "Keyboard Shortcuts",
    tour: "Tour",
    about: "About Pincel",
    check_updates: "Check for Updates\u{2026}",
    services: "Services",
    hide: "Hide Pincel",
    hide_others: "Hide Others",
    show_all: "Show All",
    select_all: "Select All",
};

pub const LABELS_DE: Labels = Labels {
    file: "Datei",
    new: "Neu",
    open: "\u{d6}ffnen\u{2026}",
    open_recent: "Zuletzt ge\u{f6}ffnet",
    no_recent: "(keine zuletzt ge\u{f6}ffneten Dateien)",
    quit: "Pincel beenden",
    edit: "Bearbeiten",
    cut: "Ausschneiden",
    copy: "Kopieren",
    paste: "Einf\u{fc}gen",
    settings: "Einstellungen\u{2026}",
    help: "Hilfe",
    shortcuts: "Tastenk\u{fc}rzel",
    tour: "Rundgang",
    about: "\u{dc}ber Pincel",
    check_updates: "Nach Updates suchen\u{2026}",
    services: "Dienste",
    hide: "Pincel ausblenden",
    hide_others: "Andere ausblenden",
    show_all: "Alle einblenden",
    select_all: "Alles ausw\u{e4}hlen",
};

pub const LABELS_ES: Labels = Labels {
    file: "Archivo",
    new: "Nuevo",
    open: "Abrir\u{2026}",
    open_recent: "Abrir recientes",
    no_recent: "(no hay archivos recientes)",
    quit: "Salir de Pincel",
    edit: "Edici\u{f3}n",
    cut: "Cortar",
    copy: "Copiar",
    paste: "Pegar",
    settings: "Ajustes\u{2026}",
    help: "Ayuda",
    shortcuts: "Atajos de teclado",
    tour: "Recorrido",
    about: "Acerca de Pincel",
    check_updates: "Buscar actualizaciones\u{2026}",
    services: "Servicios",
    hide: "Ocultar Pincel",
    hide_others: "Ocultar otros",
    show_all: "Mostrar todo",
    select_all: "Seleccionar todo",
};

pub const LABELS_JA: Labels = Labels {
    file: "\u{30d5}\u{30a1}\u{30a4}\u{30eb}",
    new: "\u{65b0}\u{898f}",
    open: "\u{958b}\u{304f}\u{2026}",
    open_recent: "\u{6700}\u{8fd1}\u{4f7f}\u{3063}\u{305f}\u{30d5}\u{30a1}\u{30a4}\u{30eb}",
    no_recent: "\u{ff08}\u{6700}\u{8fd1}\u{306e}\u{30d5}\u{30a1}\u{30a4}\u{30eb}\u{306f}\u{3042}\u{308a}\u{307e}\u{305b}\u{3093}\u{ff09}",
    quit: "Pincel\u{3092}\u{7d42}\u{4e86}",
    edit: "\u{7de8}\u{96c6}",
    cut: "\u{5207}\u{308a}\u{53d6}\u{308a}",
    copy: "\u{30b3}\u{30d4}\u{30fc}",
    paste: "\u{8cbc}\u{308a}\u{4ed8}\u{3051}",
    settings: "\u{8a2d}\u{5b9a}\u{2026}",
    help: "\u{30d8}\u{30eb}\u{30d7}",
    shortcuts: "\u{30ad}\u{30fc}\u{30dc}\u{30fc}\u{30c9}\u{30b7}\u{30e7}\u{30fc}\u{30c8}\u{30ab}\u{30c3}\u{30c8}",
    tour: "\u{30c4}\u{30a2}\u{30fc}",
    about: "Pincel\u{306b}\u{3064}\u{3044}\u{3066}",
    check_updates: "\u{30a2}\u{30c3}\u{30d7}\u{30c7}\u{30fc}\u{30c8}\u{3092}\u{78ba}\u{8a8d}\u{2026}",
    services: "\u{30b5}\u{30fc}\u{30d3}\u{30b9}",
    hide: "Pincel\u{3092}\u{96a0}\u{3059}",
    hide_others: "\u{307b}\u{304b}\u{3092}\u{96a0}\u{3059}",
    show_all: "\u{3059}\u{3079}\u{3066}\u{3092}\u{8868}\u{793a}",
    select_all: "\u{3059}\u{3079}\u{3066}\u{3092}\u{9078}\u{629e}",
};

pub const LABELS_FR: Labels = Labels {
    file: "Fichier",
    new: "Nouveau",
    open: "Ouvrir\u{2026}",
    open_recent: "Ouvrir un fichier r\u{e9}cent",
    no_recent: "(aucun fichier r\u{e9}cent)",
    quit: "Quitter Pincel",
    edit: "\u{c9}dition",
    cut: "Couper",
    copy: "Copier",
    paste: "Coller",
    settings: "R\u{e9}glages\u{2026}",
    help: "Aide",
    shortcuts: "Raccourcis clavier",
    tour: "Visite guid\u{e9}e",
    about: "\u{c0} propos de Pincel",
    check_updates: "Rechercher des mises \u{e0} jour\u{2026}",
    services: "Services",
    hide: "Masquer Pincel",
    hide_others: "Masquer les autres",
    show_all: "Tout afficher",
    select_all: "Tout s\u{e9}lectionner",
};

pub const LABELS_PT_BR: Labels = Labels {
    file: "Arquivo",
    new: "Novo",
    open: "Abrir\u{2026}",
    open_recent: "Abrir recentes",
    no_recent: "(nenhum arquivo recente)",
    quit: "Encerrar Pincel",
    edit: "Editar",
    cut: "Recortar",
    copy: "Copiar",
    paste: "Colar",
    settings: "Configura\u{e7}\u{f5}es\u{2026}",
    help: "Ajuda",
    shortcuts: "Atalhos de teclado",
    tour: "Tour",
    about: "Sobre o Pincel",
    check_updates: "Verificar atualiza\u{e7}\u{f5}es\u{2026}",
    services: "Servi\u{e7}os",
    hide: "Ocultar Pincel",
    hide_others: "Ocultar Outros",
    show_all: "Mostrar Tudo",
    select_all: "Selecionar Tudo",
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
        assert_eq!(current().quit, "Quitter Pincel");
        set_language("xx");
        assert_eq!(current().quit, "Quit Pincel");
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
                l.quit,
                l.edit,
                l.cut,
                l.copy,
                l.paste,
                l.settings,
                l.help,
                l.shortcuts,
                l.tour,
                l.about,
                l.check_updates,
                l.services,
                l.hide,
                l.hide_others,
                l.show_all,
                l.select_all,
            ] {
                assert!(!v.is_empty(), "{code}");
            }
        }
    }
}
