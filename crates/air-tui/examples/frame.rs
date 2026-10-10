// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! L'exemple du jalon M6 — ce qui reste à écrire quand le framework porte le reste.
//!
//! Au jalon M0, tout était dessiné à la main sur la surface et un texte trop long écrasait
//! le cadre qui l'entourait. Au jalon M4, chaque élément est devenu une **vue**. Au jalon
//! M5b, l'exemple bouclait — mais il montait la boucle, la disposition de l'écran et
//! l'ordre d'insertion des barres à la main, en quatre-vingts lignes qui étaient du
//! framework déguisé en exemple.
//!
//! Ici, [`AirApplication`], [`AirScreen`] et [`AirDesktop`] les portent. Ce qui reste est
//! ce qui appartient vraiment à l'application : ses menus, ses raccourcis, sa fenêtre.
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
//! - redimensionner le terminal replace les barres et le bureau sans rien relancer.
//!
//! La dernière étiquette de la fenêtre est **volontairement trop large** : c'est la
//! démonstration du découpage, et au jalon M0 elle aurait mangé la bordure droite.
//!
//! ```sh
//! cargo run --example frame
//! ```

use std::time::Duration;

use air_tui::application::{AirApplication, AirDesktop, AirPumpOutcome, AirScreen};
use air_tui::command::{AirCommand, CANCEL, OK, OPEN, QUIT, SAVE};
use air_tui::geometry::{AirRectangle, AirSize};
use air_tui::input::event::AirKey;
use air_tui::theme::AirTheme;
use air_tui::view::{AirView, AirViewId, AirViewState};
use air_tui::widgets::{
    AirButton, AirInputLine, AirMenu, AirMenuBar, AirMenuItem, AirStaticText, AirStatusItem,
    AirStatusLine, AirWindow,
};
use air_tui::{PlatformError, PlatformErrorKind, TerminalDevice};
use air_tui_platform_posix::PosixTerminal;

/// La commande « À propos », propre à cet exemple.
const ABOUT: AirCommand = AirCommand::new(256);

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
///
/// Ses limites sont provisoires : [`AirScreen`] la replace à chaque dimension d'écran.
fn build_menu_bar(identifiers: &mut Identifiers) -> AirMenuBar {
    AirMenuBar::new(AirViewState::new(
        identifiers.next(),
        AirRectangle::from_parts(0, 0, 1, 1),
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
fn build_status_line(identifiers: &mut Identifiers) -> AirStatusLine {
    AirStatusLine::new(AirViewState::new(
        identifiers.next(),
        AirRectangle::from_parts(0, 0, 1, 1),
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

/// Construit l'écran : un bureau à une fenêtre, les deux barres.
///
/// L'ordre des trois appels n'a plus d'importance : [`AirScreen`] range la barre de menu
/// en dernier lui-même, et c'est tout l'intérêt de l'avoir écrit (jalon M6).
fn build_screen(size: AirSize) -> AirScreen {
    let mut identifiers = Identifiers::new();
    let mut screen = AirScreen::new(AirViewState::new(
        identifiers.next(),
        AirRectangle::from_parts(0, 0, size.columns, size.rows),
    ));

    let desktop_rows = size.rows.saturating_sub(2);
    let window_columns = size.columns.saturating_sub(20).clamp(20, 54);
    let window_rows = desktop_rows.saturating_sub(6).clamp(5, 12);
    let window_column = size.columns.saturating_sub(window_columns) / 2;
    let window_row = desktop_rows.saturating_sub(window_rows) / 2;

    let mut desktop = AirDesktop::new(AirViewState::new(
        identifiers.next(),
        AirRectangle::from_parts(0, 0, size.columns, desktop_rows),
    ));
    desktop.insert(Box::new(build_window(
        &mut identifiers,
        AirRectangle::from_parts(window_column, window_row, window_columns, window_rows),
    )));

    screen.set_menu_bar(Box::new(build_menu_bar(&mut identifiers)));
    screen.set_desktop(Box::new(desktop));
    screen.set_status_line(Box::new(build_status_line(&mut identifiers)));
    screen
}

fn main() -> Result<(), PlatformError> {
    // C'est ici, et nulle part ailleurs, que le choix de la plateforme est fait : le cœur
    // du framework ne connaît que le contrat (ADR-011).
    let device = PosixTerminal::open()?;
    // La dimension sert ici à placer la **fenêtre** dans le bureau, ce que l'écran ne
    // fait pas : il replace ses trois pièces, pas le contenu du bureau. Une dimension
    // inconnue retombe sur la taille historique d'un terminal.
    let size = device
        .window_size()?
        .map_or(AirSize::new(80, 25), |window| {
            AirSize::new(window.columns, window.rows)
        });
    // L'écran reste à l'exemple : c'est l'appelant qui possède l'arbre, et c'est ce qui
    // laisse un contrôleur atteindre ses vues typées (jalon M6).
    let mut screen = build_screen(size);
    let mut application = AirApplication::new(Box::new(device), AirTheme::turbo_vision())?;
    // Le meilleur protocole clavier que le terminal accepte : sans lui, `Alt`+lettre
    // n'arrive pas de façon fiable (ADR-003).
    let _ = application.enable_best_keyboard_protocol(Duration::from_millis(100));

    // `OPEN` est interdite : la barre de statut la grise, le menu la grise, et aucun des
    // deux ne l'émet. Une seule source, deux chemins (ADR-007).
    application.commands_mut().disable(OPEN);

    let mut commands: Vec<AirCommand> = Vec::new();
    loop {
        application.draw(&screen)?;
        let outcome =
            match application.pump(&mut screen, Some(Duration::from_millis(250)), &mut commands) {
                Ok(outcome) => outcome,
                Err(error) if error.kind() == PlatformErrorKind::InvalidDevice => break,
                Err(error) => return Err(error),
            };
        if outcome == AirPumpOutcome::Closed || commands.contains(&QUIT) {
            break;
        }
    }
    Ok(())
}
