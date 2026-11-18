//! The native menu bar on macOS (docs/specs/macos-native-menu.md,
//! spec §11.3, Decision Log 2026-11-18).
//!
//! The bar is described as data first ([`Node`]) and turned into a Tauri
//! menu last ([`materialize`]), so what it lists can be tested without an
//! app. Two shapes:
//!
//! - **start screen** ([`start_menus`]): App menu, File (New, Open…, Open
//!   Recent), Edit (the system's text-editing items), Help.
//! - **editor** ([`editor_menus`]): App menu, then the menus the renderer
//!   sends with `set_editor_menu` — the active editor's own menus, Help
//!   included — with every chord the App menu uses taken off them.

use serde::Deserialize;
use tauri::menu::{
    CheckMenuItemBuilder, IsMenuItem, Menu, MenuItemBuilder, MenuItemKind, PredefinedMenuItem,
    Submenu, SubmenuBuilder,
};
use tauri::{AppHandle, Wry};

use crate::menu_labels::Labels;
use crate::updater;

/// Id of the start screen's File ▸ Open Recent submenu.
pub const RECENT_MENU_ID: &str = "open-recent";

/// Id of the App menu's Quit entry; the renderer asks about unsaved
/// changes before it quits (unsaved-changes-prompt AC-09).
pub const MENU_QUIT: &str = "menu:quit";

/// Id of the App menu's Settings… entry.
pub const MENU_SETTINGS: &str = "menu:settings";

/// Default accelerator of the App menu's Settings… entry.
pub const SETTINGS_DEFAULT: &str = "CmdOrCtrl+,";

/// The App menu's fixed chords besides Settings…: Hide, Hide Others, Quit.
const APP_CHORDS: [&str; 3] = ["CmdOrCtrl+H", "CmdOrCtrl+Alt+H", "CmdOrCtrl+Q"];

/// One entry of the native menu bar.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Node {
    /// A clickable entry; picking it emits `id` as a `menu` event.
    Item {
        id: String,
        label: String,
        #[serde(default = "enabled_default")]
        enabled: bool,
        /// `Some` draws a check mark column (a toggle).
        #[serde(default)]
        checked: Option<bool>,
        /// A Tauri accelerator (`CmdOrCtrl+Shift+S`).
        #[serde(default)]
        accelerator: Option<String>,
    },
    /// A nested menu; a top-level node of the bar is one too.
    Submenu {
        #[serde(default)]
        id: Option<String>,
        label: String,
        #[serde(default = "enabled_default")]
        enabled: bool,
        items: Vec<Node>,
    },
    /// A divider line.
    Separator,
    /// An item the operating system implements (AppKit's predefined items).
    System {
        role: Role,
        #[serde(default)]
        label: Option<String>,
    },
}

fn enabled_default() -> bool {
    true
}

/// The predefined items the bar uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Role {
    Cut,
    Copy,
    Paste,
    SelectAll,
    Services,
    Hide,
    HideOthers,
    ShowAll,
}

/// One entry of the start screen's Open Recent submenu.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RecentEntry {
    pub id: String,
    pub name: String,
}

fn item(id: &str, label: &str, accelerator: Option<String>) -> Node {
    Node::Item {
        id: id.to_string(),
        label: label.to_string(),
        enabled: true,
        checked: None,
        accelerator,
    }
}

fn system(role: Role, label: &str) -> Node {
    Node::System {
        role,
        label: Some(label.to_string()),
    }
}

fn submenu(id: Option<&str>, label: &str, items: Vec<Node>) -> Node {
    Node::Submenu {
        id: id.map(str::to_string),
        label: label.to_string(),
        enabled: true,
        items,
    }
}

/// The App menu (AC-07): About, Check for Updates… where the updater can
/// install, Settings…, Services, Hide / Hide Others / Show All, Quit.
pub fn app_menu(l: &Labels, updatable: bool, settings: Option<String>) -> Node {
    let mut items = vec![item(updater::MENU_ABOUT, l.about, None)];
    if updatable {
        items.push(item(updater::MENU_CHECK, l.check_updates, None));
    }
    items.extend([
        Node::Separator,
        item(MENU_SETTINGS, l.settings, settings),
        Node::Separator,
        system(Role::Services, l.services),
        Node::Separator,
        system(Role::Hide, l.hide),
        system(Role::HideOthers, l.hide_others),
        system(Role::ShowAll, l.show_all),
        Node::Separator,
        item(MENU_QUIT, l.quit, Some(APP_CHORDS[2].to_string())),
    ]);
    submenu(None, "Pincel", items)
}

/// The start screen's bar (AC-09). `accelerator(id, default)` gives an
/// entry's chord after the user's rebinding.
pub fn start_menus(
    l: &Labels,
    updatable: bool,
    recents: &[RecentEntry],
    accelerator: &dyn Fn(&str, &str) -> Option<String>,
) -> Vec<Node> {
    let recent_items = if recents.is_empty() {
        vec![Node::Item {
            id: "recent:_empty".to_string(),
            label: l.no_recent.to_string(),
            enabled: false,
            checked: None,
            accelerator: None,
        }]
    } else {
        recents
            .iter()
            .map(|r| item(&format!("recent:{}", r.id), &r.name, None))
            .collect()
    };
    vec![
        app_menu(l, updatable, accelerator(MENU_SETTINGS, SETTINGS_DEFAULT)),
        submenu(
            None,
            l.file,
            vec![
                item("menu:new", l.new, Some("CmdOrCtrl+N".to_string())),
                item("menu:open", l.open, accelerator("menu:open", "CmdOrCtrl+O")),
                submenu(Some(RECENT_MENU_ID), l.open_recent, recent_items),
            ],
        ),
        submenu(
            None,
            l.edit,
            vec![
                system(Role::Cut, l.cut),
                system(Role::Copy, l.copy),
                system(Role::Paste, l.paste),
                system(Role::SelectAll, l.select_all),
            ],
        ),
        submenu(
            None,
            l.help,
            vec![
                item(
                    "menu:shortcuts",
                    l.shortcuts,
                    accelerator("menu:shortcuts", "CmdOrCtrl+/"),
                ),
                item("menu:tour", l.tour, None),
            ],
        ),
    ]
}

/// The bar while an editor tab shows (AC-03, AC-07, AC-18): the App menu,
/// then `menus` as the renderer sent them, minus any chord the App menu
/// uses — the App menu keeps it.
pub fn editor_menus(
    l: &Labels,
    updatable: bool,
    settings: Option<String>,
    menus: &[Node],
) -> Vec<Node> {
    let mut reserved: Vec<String> = APP_CHORDS.iter().map(|c| c.to_string()).collect();
    reserved.extend(settings.iter().cloned());
    let mut out = vec![app_menu(l, updatable, settings)];
    out.extend(menus.iter().map(|m| without_chords(m, &reserved)));
    out
}

fn without_chords(node: &Node, reserved: &[String]) -> Node {
    match node {
        Node::Item {
            id,
            label,
            enabled,
            checked,
            accelerator,
        } => Node::Item {
            id: id.clone(),
            label: label.clone(),
            enabled: *enabled,
            checked: *checked,
            accelerator: accelerator
                .clone()
                .filter(|a| !reserved.iter().any(|r| same_accelerator(a, r))),
        },
        Node::Submenu {
            id,
            label,
            enabled,
            items,
        } => Node::Submenu {
            id: id.clone(),
            label: label.clone(),
            enabled: *enabled,
            items: items.iter().map(|n| without_chords(n, reserved)).collect(),
        },
        other => other.clone(),
    }
}

/// Whether two accelerators name the same chord, whatever the modifier
/// order or spelling (`Alt+CmdOrCtrl+h` = `CmdOrCtrl+Alt+H`).
pub fn same_accelerator(a: &str, b: &str) -> bool {
    normalize(a) == normalize(b)
}

fn normalize(accelerator: &str) -> (Vec<String>, String) {
    let mut parts: Vec<String> = accelerator
        .split('+')
        .map(|p| match p.trim().to_ascii_lowercase().as_str() {
            "cmd" | "command" | "commandorcontrol" | "cmdorctrl" | "super" => "cmd".to_string(),
            "ctrl" | "control" => "ctrl".to_string(),
            "alt" | "option" => "alt".to_string(),
            other => other.to_string(),
        })
        .collect();
    let key = parts.pop().unwrap_or_default();
    parts.sort();
    (parts, key)
}

/// Builds the Tauri menu for `nodes` (the top-level submenus of the bar).
pub fn materialize(app: &AppHandle, nodes: &[Node]) -> tauri::Result<Menu<Wry>> {
    let menu = Menu::new(app)?;
    for node in nodes {
        match build(app, node)? {
            Some(kind) => menu.append(&kind)?,
            None => continue,
        }
    }
    Ok(menu)
}

fn build(app: &AppHandle, node: &Node) -> tauri::Result<Option<MenuItemKind<Wry>>> {
    Ok(Some(match node {
        Node::Item {
            id,
            label,
            enabled,
            checked: Some(checked),
            accelerator,
        } => {
            let builder = CheckMenuItemBuilder::with_id(id.as_str(), label)
                .enabled(*enabled)
                .checked(*checked);
            match accelerator {
                Some(a) => builder.accelerator(a),
                None => builder,
            }
            .build(app)?
            .kind()
        }
        Node::Item {
            id,
            label,
            enabled,
            checked: None,
            accelerator,
        } => {
            let builder = MenuItemBuilder::with_id(id.as_str(), label).enabled(*enabled);
            match accelerator {
                Some(a) => builder.accelerator(a),
                None => builder,
            }
            .build(app)?
            .kind()
        }
        Node::Submenu {
            id,
            label,
            enabled,
            items,
        } => {
            let sub: Submenu<Wry> = match id {
                Some(id) => SubmenuBuilder::with_id(app, id.as_str(), label),
                None => SubmenuBuilder::new(app, label),
            }
            .enabled(*enabled)
            .build()?;
            for child in items {
                if let Some(kind) = build(app, child)? {
                    sub.append(&kind)?;
                }
            }
            sub.kind()
        }
        Node::Separator => PredefinedMenuItem::separator(app)?.kind(),
        Node::System { role, label } => {
            let text = label.as_deref();
            match role {
                Role::Cut => PredefinedMenuItem::cut(app, text)?,
                Role::Copy => PredefinedMenuItem::copy(app, text)?,
                Role::Paste => PredefinedMenuItem::paste(app, text)?,
                Role::SelectAll => PredefinedMenuItem::select_all(app, text)?,
                Role::Services => PredefinedMenuItem::services(app, text)?,
                Role::Hide => PredefinedMenuItem::hide(app, text)?,
                Role::HideOthers => PredefinedMenuItem::hide_others(app, text)?,
                Role::ShowAll => PredefinedMenuItem::show_all(app, text)?,
            }
            .kind()
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu_labels::{LABELS_DE, LABELS_EN};

    fn labels(node: &Node) -> Vec<String> {
        let Node::Submenu { items, .. } = node else {
            panic!("not a submenu: {node:?}");
        };
        items
            .iter()
            .map(|n| match n {
                Node::Item { label, .. } | Node::Submenu { label, .. } => label.clone(),
                Node::Separator => "---".to_string(),
                Node::System { role, .. } => format!("<{role:?}>"),
            })
            .collect()
    }

    fn find<'a>(nodes: &'a [Node], id: &str) -> Option<&'a Node> {
        nodes.iter().find_map(|n| match n {
            Node::Item { id: i, .. } if i == id => Some(n),
            Node::Submenu { items, .. } => find(items, id),
            _ => None,
        })
    }

    fn chord(node: Option<&Node>) -> Option<String> {
        match node {
            Some(Node::Item { accelerator, .. }) => accelerator.clone(),
            _ => None,
        }
    }

    fn defaults(_: &str, default: &str) -> Option<String> {
        Some(default.to_string())
    }

    // AC-07: the macOS standard App menu, in order.
    #[test]
    fn app_menu_updatable_lists_the_standard_entries_in_order() {
        let menu = app_menu(&LABELS_EN, true, Some(SETTINGS_DEFAULT.to_string()));
        assert_eq!(
            labels(&menu),
            [
                "About Pincel",
                "Check for Updates\u{2026}",
                "---",
                "Settings\u{2026}",
                "---",
                "<Services>",
                "---",
                "<Hide>",
                "<HideOthers>",
                "<ShowAll>",
                "---",
                "Quit Pincel",
            ]
        );
        let items = [menu];
        assert_eq!(
            chord(find(&items, MENU_SETTINGS)).as_deref(),
            Some("CmdOrCtrl+,")
        );
        assert_eq!(
            chord(find(&items, MENU_QUIT)).as_deref(),
            Some("CmdOrCtrl+Q")
        );
        assert_eq!(chord(find(&items, updater::MENU_ABOUT)), None);
    }

    // AC-07: no Check for Updates… where the updater can't install.
    #[test]
    fn app_menu_not_updatable_has_no_update_entry() {
        let menu = app_menu(&LABELS_EN, false, None);
        let items = [menu];
        assert!(find(&items, updater::MENU_CHECK).is_none());
        assert!(find(&items, updater::MENU_ABOUT).is_some());
        // A Settings… the user unbound shows no chord (AC-11).
        assert_eq!(chord(find(&items, MENU_SETTINGS)), None);
    }

    // AC-07: the App menu speaks the menu language.
    #[test]
    fn app_menu_german_labels() {
        let menu = app_menu(&LABELS_DE, false, None);
        let l = labels(&menu);
        assert_eq!(l[0], "\u{dc}ber Pincel");
        assert_eq!(l.last().map(String::as_str), Some("Pincel beenden"));
    }

    // AC-09: App, File, Edit, Help — nothing that needs a document.
    #[test]
    fn start_menus_lists_app_file_edit_help() {
        let bar = start_menus(&LABELS_EN, false, &[], &defaults);
        assert_eq!(bar.len(), 4);
        assert_eq!(labels(&bar[1]), ["New", "Open\u{2026}", "Open Recent"]);
        assert_eq!(
            labels(&bar[2]),
            ["<Cut>", "<Copy>", "<Paste>", "<SelectAll>"]
        );
        assert_eq!(labels(&bar[3]), ["Keyboard Shortcuts", "Tour"]);
        assert_eq!(
            chord(find(&bar, "menu:new")).as_deref(),
            Some("CmdOrCtrl+N")
        );
        assert_eq!(
            chord(find(&bar, "menu:open")).as_deref(),
            Some("CmdOrCtrl+O")
        );
        assert_eq!(
            chord(find(&bar, "menu:shortcuts")).as_deref(),
            Some("CmdOrCtrl+/")
        );
        for gone in ["menu:save", "menu:undo", "menu:zoomIn", "menu:close"] {
            assert!(find(&bar, gone).is_none(), "{gone}");
        }
    }

    // AC-09: Open Recent lists the pushed recents, or a dimmed placeholder.
    #[test]
    fn start_menus_open_recent_lists_recents_or_placeholder() {
        let empty = start_menus(&LABELS_EN, false, &[], &defaults);
        match find(&empty, "recent:_empty") {
            Some(Node::Item { enabled, .. }) => assert!(!enabled),
            other => panic!("no placeholder: {other:?}"),
        }
        let recents = [RecentEntry {
            id: "abc".to_string(),
            name: "cat.aseprite".to_string(),
        }];
        let bar = start_menus(&LABELS_EN, false, &recents, &defaults);
        assert!(find(&bar, "recent:_empty").is_none());
        match find(&bar, "recent:abc") {
            Some(Node::Item { label, .. }) => assert_eq!(label, "cat.aseprite"),
            other => panic!("no recent entry: {other:?}"),
        }
    }

    // AC-11: rebound chords reach the start screen's entries.
    #[test]
    fn start_menus_rebound_open_shows_new_chord() {
        let rebound = |id: &str, default: &str| {
            Some(
                if id == "menu:open" {
                    "CmdOrCtrl+Shift+O"
                } else {
                    default
                }
                .to_string(),
            )
        };
        let bar = start_menus(&LABELS_EN, false, &[], &rebound);
        assert_eq!(
            chord(find(&bar, "menu:open")).as_deref(),
            Some("CmdOrCtrl+Shift+O")
        );
    }

    fn editor_item(id: &str, accelerator: &str) -> Node {
        Node::Item {
            id: id.to_string(),
            label: id.to_string(),
            enabled: true,
            checked: None,
            accelerator: Some(accelerator.to_string()),
        }
    }

    // AC-03: the App menu leads, then the renderer's menus as sent.
    #[test]
    fn editor_menus_app_menu_first_then_sent_menus() {
        let sent = vec![
            submenu(None, "File", vec![editor_item("ed:save", "CmdOrCtrl+S")]),
            submenu(None, "Help", vec![]),
        ];
        let bar = editor_menus(&LABELS_EN, false, Some(SETTINGS_DEFAULT.to_string()), &sent);
        assert_eq!(bar.len(), 3);
        assert_eq!(bar[1], sent[0]);
        assert_eq!(bar[2], sent[1]);
    }

    // AC-18: the App menu keeps ⌘, ⌘H ⌥⌘H ⌘Q; an editor entry bound to one
    // shows no chord, whatever the spelling.
    #[test]
    fn editor_menus_entry_on_app_chord_loses_it() {
        let sent = vec![submenu(
            None,
            "Edit",
            vec![
                editor_item("ed:a", "CmdOrCtrl+Q"),
                editor_item("ed:b", "Alt+CmdOrCtrl+h"),
                editor_item("ed:c", "CmdOrCtrl+,"),
                editor_item("ed:d", "CmdOrCtrl+D"),
            ],
        )];
        let bar = editor_menus(&LABELS_EN, false, Some(SETTINGS_DEFAULT.to_string()), &sent);
        assert_eq!(chord(find(&bar, "ed:a")), None);
        assert_eq!(chord(find(&bar, "ed:b")), None);
        assert_eq!(chord(find(&bar, "ed:c")), None);
        assert_eq!(chord(find(&bar, "ed:d")).as_deref(), Some("CmdOrCtrl+D"));
        // Quit keeps ⌘Q.
        assert_eq!(chord(find(&bar, MENU_QUIT)).as_deref(), Some("CmdOrCtrl+Q"));
    }

    // AC-18: a Settings… rebound to ⌘U takes ⌘U away from an editor entry.
    #[test]
    fn editor_menus_rebound_settings_chord_is_reserved() {
        let sent = vec![submenu(
            None,
            "Edit",
            vec![editor_item("ed:u", "CmdOrCtrl+U")],
        )];
        let bar = editor_menus(&LABELS_EN, false, Some("CmdOrCtrl+U".to_string()), &sent);
        assert_eq!(chord(find(&bar, "ed:u")), None);
    }

    #[test]
    fn same_accelerator_ignores_order_and_spelling() {
        assert!(same_accelerator(
            "CmdOrCtrl+Alt+H",
            "Alt+CommandOrControl+h"
        ));
        assert!(!same_accelerator("CmdOrCtrl+H", "Ctrl+H"));
        assert!(!same_accelerator("CmdOrCtrl+H", "CmdOrCtrl+Shift+H"));
    }

    // The renderer's payload: the shapes `set_editor_menu` receives.
    #[test]
    fn node_deserializes_renderer_payload() {
        let json = r#"[{"kind":"submenu","label":"Edit","enabled":true,"items":[
            {"kind":"item","id":"ed:edit/0|Undo","label":"Undo","enabled":false,"accelerator":"CmdOrCtrl+Z"},
            {"kind":"separator"},
            {"kind":"item","id":"ed:edit/2|Grid","label":"Grid","checked":true},
            {"kind":"system","role":"selectAll","label":"Select All"}
        ]}]"#;
        let nodes: Vec<Node> = serde_json::from_str(json).expect("valid payload");
        let Node::Submenu { items, .. } = &nodes[0] else {
            panic!("not a submenu");
        };
        assert_eq!(items.len(), 4);
        assert_eq!(
            items[3],
            Node::System {
                role: Role::SelectAll,
                label: Some("Select All".to_string())
            }
        );
        match &items[2] {
            Node::Item {
                checked, enabled, ..
            } => {
                assert_eq!(*checked, Some(true));
                assert!(enabled);
            }
            other => panic!("{other:?}"),
        }
    }
}
