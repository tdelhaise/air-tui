// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Exemple du jalon M4 — la même image, mais bâtie sur l'arbre de vues.
//!
//! Au jalon M0, tout était dessiné à la main sur la surface et un texte trop long écrasait
//! le cadre qui l'entourait. Ici, chaque élément est une **vue** : le bureau, la barre de
//! menu, une fenêtre à titre, son étiquette, la barre de statut. Aucune ne sait où elle se
//! trouve, aucune ne peut déborder de son propriétaire.
//!
//! La dernière vue insérée est **volontairement trop large** pour sa fenêtre : c'est la
//! démonstration du découpage, et au jalon M0 elle aurait mangé la bordure droite.
//!
//! ```sh
//! cargo run --example frame
//! ```

use std::time::Duration;

use air_tui::cell::{AirAttributes, AirCell};
use air_tui::command::AirCommandSet;
use air_tui::command::{CANCEL, OK};
use air_tui::geometry::{AirColumn, AirPoint, AirRectangle, AirRow, AirSize};
use air_tui::surface::AirSurface;
use air_tui::terminal::AirTerminal;
use air_tui::theme::{AirRole, AirTheme};
use air_tui::view::painter::AirPainter;
use air_tui::view::{AirGroup, AirView, AirViewId, AirViewState, draw_view};
use air_tui::widgets::{AirButton, AirStaticText, AirWindow};
use air_tui::{InputReadiness, PlatformError, PlatformErrorKind};
use air_tui_platform_posix::PosixTerminal;

/// Le bureau — un fond hachuré, sans bordure, qui occupe tout ce qu'on lui donne.
///
/// Une vue écrite par l'application, pour montrer qu'il n'y a rien à hériter : un état, un
/// dessin, et c'est tout.
struct Desktop {
    /// L'état commun.
    state: AirViewState,
}

impl Desktop {
    /// Un bureau sans bordure : c'est le fond, pas une fenêtre.
    fn new(state: AirViewState) -> Self {
        let mut state = state;
        state.set_border(false);
        Self { state }
    }
}

impl AirView for Desktop {
    fn state(&self) -> &AirViewState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut AirViewState {
        &mut self.state
    }

    fn draw_content(&self, painter: &mut AirPainter<'_>, theme: &AirTheme) {
        let colors = theme.palette.color_pair(AirRole::DesktopBackground);
        let size = painter.size();
        painter.fill_rectangle(
            AirRectangle::new(AirPoint::ORIGIN, size),
            AirCell::new(
                theme.glyphs.desktop_fill,
                colors.foreground,
                colors.background,
            ),
        );
    }
}

/// Une barre horizontale à items, pour le menu comme pour le statut.
struct Bar {
    /// L'état commun.
    state: AirViewState,
    /// Les items : la lettre d'accès rapide, puis le reste du libellé.
    items: Vec<(String, String)>,
    /// Le rôle du fond et du texte ordinaire.
    role: AirRole,
    /// Le rôle de la lettre d'accès rapide.
    hotkey_role: AirRole,
}

impl Bar {
    /// Une barre sans bordure, à un rôle de fond et un rôle de lettre d'accès rapide.
    fn new(state: AirViewState, role: AirRole, hotkey_role: AirRole) -> Self {
        let mut state = state;
        state.set_border(false);
        Self {
            state,
            items: Vec::new(),
            role,
            hotkey_role,
        }
    }

    /// La même barre, avec un item de plus.
    fn with_item(mut self, hotkey: &str, remainder: &str) -> Self {
        self.items.push((hotkey.to_owned(), remainder.to_owned()));
        self
    }
}

impl AirView for Bar {
    fn state(&self) -> &AirViewState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut AirViewState {
        &mut self.state
    }

    fn draw_content(&self, painter: &mut AirPainter<'_>, theme: &AirTheme) {
        let colors = theme.palette.color_pair(self.role);
        let hotkey_colors = theme.palette.color_pair(self.hotkey_role);
        let size = painter.size();
        painter.fill_rectangle(
            AirRectangle::new(AirPoint::ORIGIN, size),
            AirCell::new(' ', colors.foreground, colors.background),
        );
        let mut column = AirColumn::new(1);
        for (hotkey, remainder) in &self.items {
            column = painter.draw_text(
                AirPoint::new(column, AirRow::FIRST),
                hotkey,
                hotkey_colors.foreground,
                hotkey_colors.background,
                AirAttributes::NONE,
            );
            column = painter.draw_text(
                AirPoint::new(column, AirRow::FIRST),
                remainder,
                colors.foreground,
                colors.background,
                AirAttributes::NONE,
            );
            column = AirColumn::new(column.value().saturating_add(2));
        }
    }
}

/// Un générateur d'identifiants de vue.
///
/// Le compteur part de un et sature : il ne peut pas produire zéro, et le repli n'est là
/// que pour ne pas écrire un `expect` dans un exemple.
struct Identifiers {
    /// Le dernier identifiant distribué.
    last: u32,
}

impl Identifiers {
    /// Un générateur neuf.
    fn new() -> Self {
        Self { last: 0 }
    }

    /// L'identifiant suivant.
    fn next(&mut self) -> AirViewId {
        self.last = self.last.saturating_add(1);
        AirViewId::new(self.last).unwrap_or(AirViewId::FIRST)
    }
}

/// La barre de menu.
fn build_menu_bar(identifiers: &mut Identifiers, columns: u16) -> Bar {
    Bar::new(
        AirViewState::new(
            identifiers.next(),
            AirRectangle::from_parts(0, 0, columns, 1),
        ),
        AirRole::MenuBar,
        AirRole::MenuHotkey,
    )
    .with_item("F", "ile")
    .with_item("E", "dit")
    .with_item("S", "earch")
    .with_item("W", "indows")
}

/// La barre de statut.
fn build_status_bar(identifiers: &mut Identifiers, size: AirSize) -> Bar {
    Bar::new(
        AirViewState::new(
            identifiers.next(),
            AirRectangle::from_parts(0, size.rows.saturating_sub(1), size.columns, 1),
        ),
        AirRole::StatusLine,
        AirRole::StatusHotkey,
    )
    .with_item("Toute touche", " Quitter")
}

/// La fenêtre, avec son titre, ses étiquettes et ses deux boutons.
fn build_window(identifiers: &mut Identifiers, bounds: AirRectangle) -> AirWindow {
    let columns = bounds.size.columns;
    let rows = bounds.size.rows;
    let mut window =
        AirWindow::new(AirViewState::new(identifiers.next(), bounds)).with_title("air-tui");
    window.state_mut().set_focused(true);

    window.insert(Box::new(AirStaticText::new(
        AirViewState::new(
            identifiers.next(),
            AirRectangle::from_parts(1, 1, columns.saturating_sub(3), 1),
        ),
        "Arbre de vues : chaque élément est une vue.",
    )));
    // Volontairement plus large que sa fenêtre : au jalon M0, cette ligne mangeait la
    // bordure droite. Le peintre la découpe désormais sur la vue.
    window.insert(Box::new(AirStaticText::new(
        AirViewState::new(
            identifiers.next(),
            AirRectangle::from_parts(1, 3, columns.saturating_add(40), 1),
        ),
        "Ce texte est trop long pour sa fenêtre et ne peut plus déborder sur le cadre.",
    )));

    // Deux boutons aux couleurs transcrites de Borland : noir sur vert, lettre d'accès
    // rapide en jaune. Le bouton par défaut se distingue par sa couleur de texte.
    let button_row = rows.saturating_sub(4);
    let mut accept = AirButton::new(
        AirViewState::new(
            identifiers.next(),
            AirRectangle::from_parts(2, button_row, 10, 1),
        ),
        "~O~K",
        OK,
    )
    .as_default();
    accept.state_mut().set_focused(true);
    window.insert(Box::new(accept));
    window.insert(Box::new(AirButton::new(
        AirViewState::new(
            identifiers.next(),
            AirRectangle::from_parts(14, button_row, 12, 1),
        ),
        "~A~nnuler",
        CANCEL,
    )));
    window
}

/// Construit l'arbre pour une dimension d'écran donnée.
fn build_tree(size: AirSize) -> AirGroup {
    let mut identifiers = Identifiers::new();

    // La racine occupe tout l'écran et n'a pas de bordure : c'est l'écran, pas une fenêtre.
    let mut root = AirGroup::new(AirViewState::new(
        identifiers.next(),
        AirRectangle::new(AirPoint::ORIGIN, size),
    ));
    root.state_mut().set_border(false);
    root.insert(Box::new(build_menu_bar(&mut identifiers, size.columns)));

    let desktop_height = size.rows.saturating_sub(2);
    let window_columns = size.columns.saturating_sub(20).clamp(20, 54);
    let window_rows = desktop_height.saturating_sub(6).clamp(5, 12);
    let window_column = size.columns.saturating_sub(window_columns) / 2;
    let window_row = desktop_height.saturating_sub(window_rows) / 2;

    let mut desktop_group = AirGroup::new(AirViewState::new(
        identifiers.next(),
        AirRectangle::from_parts(0, 1, size.columns, desktop_height),
    ));
    desktop_group.state_mut().set_border(false);
    desktop_group.insert(Box::new(Desktop::new(AirViewState::new(
        identifiers.next(),
        AirRectangle::from_parts(0, 0, size.columns, desktop_height),
    ))));
    desktop_group.insert(Box::new(build_window(
        &mut identifiers,
        AirRectangle::from_parts(window_column, window_row, window_columns, window_rows),
    )));
    root.insert(Box::new(desktop_group));
    root.insert(Box::new(build_status_bar(&mut identifiers, size)));
    root
}

fn main() -> Result<(), PlatformError> {
    // C'est ici, et nulle part ailleurs, que le choix de la plateforme est fait : le cœur
    // du framework ne connaît que le contrat (ADR-011).
    let device = PosixTerminal::open()?;
    let mut terminal = AirTerminal::new(Box::new(device))?;
    let theme = AirTheme::turbo_vision();
    let size = terminal.size()?;
    let mut surface = AirSurface::new(size);
    let mut commands = AirCommandSet::new();
    let _ = &mut commands;

    let tree = build_tree(size);
    {
        let mut painter = AirPainter::new(&mut surface);
        draw_view(&tree, &mut painter, &theme);
    }
    terminal.present(&mut surface)?;

    // Attendre une frappe. Le décodeur existe depuis le jalon M1, mais la boucle
    // d'événements complète — celle qui alimentera `dispatch_event` — vient avec
    // l'application `redit` au jalon M6.
    let mut input = [0_u8; 32];
    loop {
        match terminal.wait_for_input(Some(Duration::from_millis(250))) {
            Ok(InputReadiness::Ready) => {
                let read = terminal.read_available(&mut input)?;
                if read > 0 {
                    break;
                }
            }
            Ok(InputReadiness::Closed) => break,
            Ok(InputReadiness::TimedOut) => {}
            Err(error) if error.is_interrupted() => {}
            Err(error) if error.kind() == PlatformErrorKind::InvalidDevice => break,
            Err(error) => return Err(error),
        }
    }

    Ok(())
}
