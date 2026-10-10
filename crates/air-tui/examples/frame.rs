// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! L'exemple du jalon M5b — l'arbre de vues, et maintenant il répond.
//!
//! Au jalon M0, tout était dessiné à la main sur la surface et un texte trop long écrasait
//! le cadre qui l'entourait. Au jalon M4, chaque élément est devenu une **vue** : aucune
//! ne sait où elle se trouve, aucune ne peut déborder de son propriétaire. Ici, l'exemple
//! **boucle** : il décode les frappes, les route dans l'arbre, et redessine.
//!
//! Ce qu'il y a à essayer :
//!
//! - `Alt`+`F`, `Alt`+`E`, `Alt`+`W` ouvrent un menu ; les flèches s'y déplacent, `Entrée`
//!   active, `Échap` referme. La souris fait la même chose.
//! - le champ de saisie a le focus : la saisie, les flèches, `Début`, `Fin`, `Suppr` et
//!   `Retour arrière` y travaillent par **grappes de graphème** — un accent combinant ou
//!   un émoji s'effacent d'une touche.
//! - la barre de statut publie `F2` et `F3`. `F3` est **interdite** : elle est grisée, et
//!   ni la touche ni le clic ne l'activent.
//! - `F10` et « Exit » quittent.
//!
//! La dernière étiquette de la fenêtre est **volontairement trop large** : c'est la
//! démonstration du découpage, et au jalon M0 elle aurait mangé la bordure droite.
//!
//! ```sh
//! cargo run --example frame
//! ```

use std::time::Duration;

use air_tui::cell::AirCell;
use air_tui::command::{AirCommand, AirCommandSet, CANCEL, OK, OPEN, QUIT, SAVE};
use air_tui::geometry::{AirPoint, AirRectangle, AirSize};
use air_tui::input::ansi::AirAnsiDecoder;
use air_tui::input::event::{AirInputEvent, AirKey};
use air_tui::surface::AirSurface;
use air_tui::terminal::AirTerminal;
use air_tui::theme::{AirRole, AirTheme};
use air_tui::view::painter::AirPainter;
use air_tui::view::{
    AirContext, AirDrawContext, AirEvent, AirGroup, AirOutcome, AirView, AirViewId, AirViewState,
    dispatch_event, draw_view,
};
use air_tui::widgets::{
    AirButton, AirInputLine, AirMenu, AirMenuBar, AirMenuItem, AirStaticText, AirStatusItem,
    AirStatusLine, AirWindow,
};
use air_tui::{Decoder, InputReadiness, PlatformError, PlatformErrorKind};
use air_tui_platform_posix::PosixTerminal;

/// La commande « À propos », propre à cet exemple.
const ABOUT: AirCommand = AirCommand::new(256);

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

    fn draw_content(&self, painter: &mut AirPainter<'_>, context: &AirDrawContext<'_>) {
        let colors = context.theme.palette.color_pair(AirRole::DesktopBackground);
        let size = painter.size();
        painter.fill_rectangle(
            AirRectangle::new(AirPoint::ORIGIN, size),
            AirCell::new(
                context.theme.glyphs.desktop_fill,
                colors.foreground,
                colors.background,
            ),
        );
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

/// La barre de menu, avec ses trois menus déroulants.
fn build_menu_bar(identifiers: &mut Identifiers, columns: u16) -> AirMenuBar {
    AirMenuBar::new(AirViewState::new(
        identifiers.next(),
        AirRectangle::from_parts(0, 0, columns, 1),
    ))
    .with_menu(
        AirMenu::new("~F~ile")
            .with_item(AirMenuItem::new("~O~pen", OPEN))
            .with_item(AirMenuItem::new("~S~ave", SAVE))
            .with_item(AirMenuItem::separator())
            .with_item(AirMenuItem::new("E~x~it", QUIT)),
    )
    .with_menu(
        AirMenu::new("~E~dit")
            .with_item(AirMenuItem::new("~C~opy", AirCommand::new(257)))
            .with_item(AirMenuItem::new("~P~aste", AirCommand::new(258))),
    )
    .with_menu(AirMenu::new("~W~indow").with_item(AirMenuItem::new("~A~bout", ABOUT)))
}

/// La barre de statut, avec un raccourci interdit pour montrer le grisage.
fn build_status_line(identifiers: &mut Identifiers, size: AirSize) -> AirStatusLine {
    AirStatusLine::new(AirViewState::new(
        identifiers.next(),
        AirRectangle::from_parts(0, size.rows.saturating_sub(1), size.columns, 1),
    ))
    .with_item(AirStatusItem::new("~F2~ Save", AirKey::Function(2), SAVE))
    .with_item(AirStatusItem::new("~F3~ Open", AirKey::Function(3), OPEN))
    .with_item(AirStatusItem::new("~F10~ Exit", AirKey::Function(10), QUIT))
    .with_item(AirStatusItem::clickable("Exit", QUIT))
}

/// La fenêtre, avec son titre, son champ de saisie, ses étiquettes et ses deux boutons.
fn build_window(identifiers: &mut Identifiers, bounds: AirRectangle) -> AirWindow {
    let columns = bounds.size.columns;
    let rows = bounds.size.rows;
    let mut window =
        AirWindow::new(AirViewState::new(identifiers.next(), bounds)).with_title("air-tui");
    window.state_mut().set_focused(true);

    window.insert(Box::new(AirStaticText::new(
        AirViewState::new(
            identifiers.next(),
            AirRectangle::from_parts(1, 0, columns.saturating_sub(3), 1),
        ),
        "Nom :",
    )));
    // Le champ a le focus : c'est lui qui porte le curseur du terminal.
    let mut field = AirInputLine::new(AirViewState::new(
        identifiers.next(),
        AirRectangle::from_parts(7, 0, columns.saturating_sub(10), 1),
    ))
    .with_text("saisis ici")
    .with_maximum_length(64);
    field.state_mut().set_focused(true);
    window.insert(Box::new(field));

    // Volontairement plus large que sa fenêtre : au jalon M0, cette ligne mangeait la
    // bordure droite. Le peintre la découpe désormais sur la vue.
    window.insert(Box::new(AirStaticText::new(
        AirViewState::new(
            identifiers.next(),
            AirRectangle::from_parts(1, 2, columns.saturating_add(40), 1),
        ),
        "Ce texte est trop long pour sa fenêtre et ne peut plus déborder sur le cadre.",
    )));

    // Deux boutons aux couleurs transcrites de Borland : noir sur vert, lettre d'accès
    // rapide en jaune. Le bouton par défaut se distingue par sa couleur de texte.
    let button_row = rows.saturating_sub(4);
    window.insert(Box::new(
        AirButton::new(
            AirViewState::new(
                identifiers.next(),
                AirRectangle::from_parts(2, button_row, 10, 1),
            ),
            "~O~K",
            OK,
        )
        .as_default(),
    ));
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
    desktop_group.state_mut().set_focused(true);
    root.insert(Box::new(desktop_group));
    root.insert(Box::new(build_status_line(&mut identifiers, size)));
    // La barre de menu est insérée **en dernier** : elle est au premier plan, et son menu
    // déroulant recouvre donc ce qui est derrière, au dessin comme au routage. C'est
    // obligatoire, pas conseillé — insérée ailleurs, elle serait recouverte.
    root.insert(Box::new(build_menu_bar(&mut identifiers, size.columns)));
    root
}

/// Dessine l'arbre et rend le curseur à poser.
fn render(
    tree: &AirGroup,
    surface: &mut AirSurface,
    theme: &AirTheme,
    commands: &AirCommandSet,
) -> Option<AirPoint> {
    let mut painter = AirPainter::new(surface);
    draw_view(tree, &mut painter, &AirDrawContext::new(theme, commands))
}

fn main() -> Result<(), PlatformError> {
    // C'est ici, et nulle part ailleurs, que le choix de la plateforme est fait : le cœur
    // du framework ne connaît que le contrat (ADR-011).
    let device = PosixTerminal::open()?;
    let mut terminal = AirTerminal::new(Box::new(device))?;
    // Le meilleur protocole clavier que le terminal accepte : sans lui, `Alt`+lettre
    // n'arrive pas de façon fiable (ADR-003).
    // Les événements qui arriveraient pendant la négociation sont ignorés : l'écran n'est
    // pas encore dessiné, et rien n'a de sens à recevoir une frappe.
    let _ = terminal.enable_best_keyboard_protocol(Duration::from_millis(100), &mut |_| {});
    let theme = AirTheme::turbo_vision();
    let size = terminal.size()?;
    let mut surface = AirSurface::new(size);

    // `OPEN` est interdite : la barre de statut la grise, le menu la grise, et aucun des
    // deux ne l'émet. Une seule source, deux chemins (ADR-007).
    let mut commands = AirCommandSet::new();
    commands.disable(OPEN);

    let mut tree = build_tree(size);
    let cursor = render(&tree, &mut surface, &theme, &commands);
    terminal.set_cursor(cursor);
    terminal.present(&mut surface)?;

    let mut decoder = AirAnsiDecoder::new();
    let mut pending: Vec<u8> = Vec::with_capacity(256);
    let mut input = [0_u8; 256];
    'session: loop {
        match terminal.wait_for_input(Some(Duration::from_millis(250))) {
            Ok(InputReadiness::Ready) => {
                let read = terminal.read_available(&mut input)?;
                if read == 0 {
                    // Prêt mais zéro octet : l'autre bout a fermé. Selon la plateforme,
                    // `poll` signale cette fin par `POLLHUP` ou par un `POLLIN` qui ne
                    // rend rien — sans ce test, le second cas boucle indéfiniment.
                    break 'session;
                }
                let Some(received) = input.get(..read) else {
                    continue;
                };
                pending.extend_from_slice(received);
            }
            Ok(InputReadiness::Closed) => break 'session,
            Ok(InputReadiness::TimedOut) => continue,
            Err(error) if error.is_interrupted() => continue,
            Err(error) if error.kind() == PlatformErrorKind::InvalidDevice => break 'session,
            Err(error) => return Err(error),
        }

        let mut events: Vec<AirInputEvent> = Vec::new();
        // Une séquence invalide ne doit pas figer l'exemple : on jette ce qui est en
        // attente et on repart. Une application réelle la journaliserait.
        let Ok(consumed) = decoder.decode(&pending, &mut |event| events.push(event)) else {
            pending.clear();
            continue;
        };
        pending.drain(..consumed);

        for input_event in events {
            let mut event = AirEvent::new(input_event);
            let mut context = AirContext {
                commands: &mut commands,
            };
            if let AirOutcome::Command(command) =
                dispatch_event(&mut tree, &mut event, &mut context)
                && command == QUIT
            {
                break 'session;
            }
        }

        let cursor = render(&tree, &mut surface, &theme, &commands);
        terminal.set_cursor(cursor);
        terminal.present(&mut surface)?;
    }

    Ok(())
}
