// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! L'application — la boucle, le terminal, le thème et la pile d'écrans.
//!
//! # Elle ne prend pas la boucle, elle la sert
//!
//! `TApplication::run` de Turbo Vision bouclait jusqu'à la sortie et appelait
//! `handleEvent` sur ce qu'elle trouvait. Ici, c'est l'inverse : [`AirApplication::pump`]
//! fait **un** tour — attendre, lire, décoder, router — et **rend les commandes** que
//! l'arbre n'a pas traitées. L'appelant garde sa boucle.
//!
//! Ce n'est pas une préférence de style, c'est la même décision que l'ADR-007 prend pour
//! le modèle : la notification devient une donnée rendue à l'appelant. Un rappel
//! `FnMut(AirCommand)` passé à `run` aurait besoin de modifier l'arbre et le modèle
//! pendant que l'application les tient — exactement l'emprunt impossible qu'on cherche à
//! éviter. En rendant les commandes, le contrôleur les traite **après**, et il est seul à
//! tenir quoi que ce soit.
//!
//! # Elle ne possède pas l'arbre non plus, et c'est le même argument
//!
//! La racine est **passée en paramètre** à [`AirApplication::draw`] et à
//! [`AirApplication::pump`]. Le contrôleur la possède.
//!
//! La première écriture de ce module faisait l'inverse : l'application possédait un
//! [`AirScreen`], et une pile d'écrans par-dessus. C'était intenable dès la première
//! application réelle. `children` rend des `Box<dyn AirView>` : une fois la vue d'édition
//! confiée à l'application, plus personne ne pouvait lui demander de se synchroniser sur
//! le modèle ni lui prendre ses intentions. Le contrôleur a besoin d'un accès **typé** à
//! ses vues, et le seul moyen de le lui laisser est de ne pas les lui prendre.
//!
//! La pile d'écrans en découle : elle appartient à l'appelant, qui tient autant d'arbres
//! qu'il veut et passe celui qu'il veut dessiner. C'est plus simple que ce qui était
//! écrit ici, et cela ne retire aucune possibilité.
//!
//! Le tour de boucle d'une application MVC complète :
//!
//! ```text
//! loop {
//!     view.synchronize(&model);                      // 3. faits → réaction
//!     application.draw(&root)?;                      // 4. rendu différentiel
//!     if application.pump(&mut screen, &mut root, …, &mut commands)? == Closed { break } // 1. entrée
//!     for action in view.drain_actions() { model.apply(action, …) }         // 2. mutation
//!     for command in &commands { … }
//! }
//! ```
//!
//! # Ce que la boucle fait sans qu'on le demande
//!
//! - elle **relit la dimension du terminal** à chaque tour et redimensionne la racine
//!   quand elle a changé, par [`crate::view::AirView::resize`] — qu'un conteneur à
//!   disposition redéfinit. Sans signal, sans `SIGWINCH` : une lecture d'`ioctl` par tour
//!   coûte moins que le gestionnaire de signal qu'il faudrait écrire dans la couche 0 ;
//! - elle traite la **fin de flux** comme une fermeture. Un `poll` prêt qui ne rend aucun
//!   octet est une fin de flux sur certaines plateformes et un `POLLHUP` sur d'autres ;
//!   sans ce test, le premier cas boucle indéfiniment. Ce défaut a existé dans l'exemple du
//!   jalon M5b, et il est corrigé ici une fois pour toutes les applications.

pub mod desktop;
pub mod screen;

pub use desktop::AirDesktop;
pub use screen::AirScreen;

use core::time::Duration;
use std::time::Instant;

use air_tui_platform::{Decoder, InputReadiness, PlatformError, PlatformErrorKind, TerminalDevice};

use crate::command::{AirCommand, AirCommandSet};
use crate::geometry::AirSize;
use crate::input::ansi::AirAnsiDecoder;
use crate::input::clicks::{AirClickTracker, extra_event};
use crate::input::event::{AirInputEvent, AirMouseEvent, AirMouseEventKind};
use crate::surface::AirSurface;
use crate::terminal::AirTerminal;
use crate::theme::AirTheme;
use crate::view::painter::AirPainter;
use crate::view::{
    AirContext, AirDrawContext, AirEvent, AirOutcome, AirView, dispatch_event, draw_view,
};

/// La capacité du tampon de lecture, en octets.
///
/// Deux cent cinquante-six : un collage encadré arrive en plusieurs lectures de toute
/// façon, et le décodeur recolle ce qui est coupé.
const READ_BUFFER_SIZE: usize = 256;

/// Ce qu'un tour de boucle a conclu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AirPumpOutcome {
    /// La boucle continue.
    Continued,
    /// Le terminal s'est fermé : la boucle doit s'arrêter.
    Closed,
}

/// Une application en mode texte.
pub struct AirApplication {
    /// Le terminal.
    terminal: AirTerminal,
    /// La surface de rendu.
    surface: AirSurface,
    /// Le thème en vigueur.
    theme: AirTheme,
    /// Le jeu d'activation des commandes.
    commands: AirCommandSet,
    /// Le décodeur d'entrée.
    decoder: AirAnsiDecoder,
    /// Les octets lus et pas encore décodés.
    pending: Vec<u8>,
    /// Le tampon de lecture.
    input: [u8; READ_BUFFER_SIZE],
    /// Les événements décodés du tour en cours.
    events: Vec<AirInputEvent>,
    /// Les événements en cours de distribution.
    ///
    /// Un second vecteur, échangé avec le premier : distribuer depuis celui que le
    /// décodeur remplit demanderait de le tenir pendant qu'on le modifie, et le vider à
    /// chaque tour perdrait sa capacité.
    dispatching: Vec<AirInputEvent>,
    /// La dimension du terminal au dernier tour.
    size: AirSize,
    /// Le suiveur des clics multiples.
    ///
    /// Ici et non dans le décodeur, qui est pur, ni dans les widgets, où ce serait la même
    /// règle écrite douze fois : c'est la leçon du jalon M7c, appliquée avant d'avoir à la
    /// repayer. Voir [`crate::input::clicks`].
    clicks: AirClickTracker<Instant>,
}

impl AirApplication {
    /// Une application, sur un périphérique.
    ///
    /// Le terminal passe en mode brut à la construction et sera restauré à la destruction
    /// (RAII). L'arbre, lui, reste à l'appelant : il le passe à [`Self::draw`] et à
    /// [`Self::pump`].
    ///
    /// # Errors
    ///
    /// Si le terminal refuse le mode brut ou ne rend pas sa dimension.
    pub fn new(device: Box<dyn TerminalDevice>, theme: AirTheme) -> Result<Self, PlatformError> {
        let terminal = AirTerminal::new(device)?;
        let size = terminal.size()?;
        Ok(Self {
            terminal,
            surface: AirSurface::new(size),
            theme,
            commands: AirCommandSet::new(),
            decoder: AirAnsiDecoder::new(),
            pending: Vec::with_capacity(READ_BUFFER_SIZE),
            input: [0; READ_BUFFER_SIZE],
            events: Vec::new(),
            dispatching: Vec::new(),
            size,
            clicks: AirClickTracker::new(),
        })
    }

    /// Négocie le meilleur protocole clavier que le terminal accepte.
    ///
    /// Sans lui, `Alt`+lettre n'arrive pas de façon fiable (ADR-003). Les événements qui
    /// arriveraient pendant la négociation sont ignorés : l'écran n'est pas encore
    /// dessiné, et rien n'a de sens à recevoir une frappe.
    ///
    /// # Errors
    ///
    /// Si l'écriture de l'interrogation ou la lecture de la réponse échoue.
    pub fn enable_best_keyboard_protocol(
        &mut self,
        timeout: Duration,
    ) -> Result<(), PlatformError> {
        self.terminal
            .enable_best_keyboard_protocol(timeout, &mut |_| {})
            .map(|_| ())
    }

    /// Le thème en vigueur.
    #[must_use]
    pub const fn theme(&self) -> &AirTheme {
        &self.theme
    }

    /// Change le thème. Le prochain dessin l'applique à tout l'arbre.
    pub fn set_theme(&mut self, theme: AirTheme) {
        self.theme = theme;
    }

    /// Le jeu d'activation des commandes.
    #[must_use]
    pub const fn commands(&self) -> &AirCommandSet {
        &self.commands
    }

    /// Le jeu d'activation des commandes, en écriture.
    pub const fn commands_mut(&mut self) -> &mut AirCommandSet {
        &mut self.commands
    }

    /// La dimension du terminal au dernier tour.
    #[must_use]
    pub const fn size(&self) -> AirSize {
        self.size
    }

    /// Dessine un arbre et pose le curseur.
    ///
    /// Le curseur rendu est celui de la vue focalisée la plus profonde, et il est émis
    /// **après** les cellules — sans quoi le rendu le déplacerait.
    ///
    /// # Errors
    ///
    /// Si l'écriture sur le terminal échoue.
    pub fn draw(&mut self, root: &dyn AirView) -> Result<(), PlatformError> {
        let context = AirDrawContext::new(&self.theme, &self.commands);
        let cursor = {
            let mut painter = AirPainter::new(&mut self.surface);
            draw_view(root, &mut painter, &context)
        };
        self.terminal.set_cursor(cursor);
        self.terminal.present(&mut self.surface)
    }

    /// Relit la dimension du terminal et redimensionne l'arbre si elle a changé.
    ///
    /// Rend `true` quand la dimension a changé. L'appelant qui tient plusieurs arbres —
    /// une pile d'écrans — redimensionne les autres lui-même : seul lui sait qu'ils
    /// existent.
    ///
    /// # Errors
    ///
    /// Si le terminal ne rend pas sa dimension.
    pub fn refresh_size(&mut self, root: &mut dyn AirView) -> Result<bool, PlatformError> {
        let size = self.terminal.size()?;
        if size == self.size {
            return Ok(false);
        }
        self.size = size;
        self.surface.resize(size);
        root.resize(size);
        // Le redimensionnement déplace tout : la cellule du clic précédent ne désigne plus
        // la même chose, et enchaîner dessus ferait un double clic sur un autre widget.
        self.clicks.forget();
        Ok(true)
    }

    /// Un tour de boucle : attendre, lire, décoder, router.
    ///
    /// `commands` est **vidé** au début du tour, puis reçoit les commandes que l'arbre n'a
    /// pas traitées, dans l'ordre. Le vider ici plutôt que de l'y laisser accumuler évite
    /// le défaut le plus facile à écrire : rejouer une commande déjà traitée au tour
    /// suivant.
    ///
    /// Ne dessine pas : c'est l'appelant qui dessine, après avoir réagi. L'ordre importe —
    /// dessiner ici afficherait l'état d'avant la réaction.
    ///
    /// # Errors
    ///
    /// Si la lecture sur le terminal échoue autrement que par une interruption ou une
    /// fermeture.
    pub fn pump(
        &mut self,
        root: &mut dyn AirView,
        timeout: Option<Duration>,
        commands: &mut Vec<AirCommand>,
    ) -> Result<AirPumpOutcome, PlatformError> {
        commands.clear();
        match self.terminal.wait_for_input(timeout) {
            Ok(InputReadiness::Ready) => {
                let read = self.terminal.read_available(&mut self.input)?;
                if read == 0 {
                    // Prêt mais zéro octet : l'autre bout a fermé. Selon la plateforme,
                    // `poll` signale cette fin par `POLLHUP` ou par un `POLLIN` qui ne
                    // rend rien — sans ce test, le second cas boucle indéfiniment.
                    return Ok(AirPumpOutcome::Closed);
                }
                if let Some(received) = self.input.get(..read) {
                    self.pending.extend_from_slice(received);
                }
            }
            Ok(InputReadiness::Closed) => return Ok(AirPumpOutcome::Closed),
            Ok(InputReadiness::TimedOut) => {}
            Err(error) if error.is_interrupted() => {}
            // Le périphérique a disparu : c'est une fermeture, pas une erreur à remonter.
            Err(error) if error.kind() == PlatformErrorKind::InvalidDevice => {
                return Ok(AirPumpOutcome::Closed);
            }
            Err(error) => return Err(error),
        }

        self.refresh_size(root)?;
        self.decode_pending();
        self.dispatch_events(root, commands);
        Ok(AirPumpOutcome::Continued)
    }

    /// Décode ce qui est en attente dans `events`.
    fn decode_pending(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        let decoder = &mut self.decoder;
        let pending = &self.pending;
        let events = &mut self.events;
        // Une séquence invalide ne doit pas figer l'application : on jette ce qui est en
        // attente et on repart. Le décodeur, lui, ne rend une erreur que sur une entrée
        // qu'aucune suite d'octets ne peut compléter.
        let Ok(consumed) = decoder.decode(pending, &mut |event| events.push(event)) else {
            self.pending.clear();
            return;
        };
        self.pending.drain(..consumed);
    }

    /// Distribue les événements décodés et collecte les commandes.
    ///
    /// Un enfoncement de souris est distribué, puis **suivi** de son double ou triple clic
    /// quand il en termine un : l'ordre compte, parce que c'est l'enfoncement qui place le
    /// curseur et le clic multiple qui étend ensuite la sélection.
    fn dispatch_events(&mut self, root: &mut dyn AirView, commands: &mut Vec<AirCommand>) {
        core::mem::swap(&mut self.events, &mut self.dispatching);
        // Le vecteur est **emprunté au champ** le temps du tour, puis rendu : itérer
        // directement sur `self.dispatching` emprunterait `self` et interdirait d'appeler
        // la synthèse, qui a besoin du suiveur de clics. Le rendre conserve sa capacité,
        // ce qui est la raison d'être du second vecteur.
        let mut dispatching = core::mem::take(&mut self.dispatching);
        let now = Instant::now();
        for input in dispatching.drain(..) {
            let multiple = self.multiple_click(&input, now);
            self.dispatch_one(root, input, commands);
            if let Some(synthesized) = multiple {
                self.dispatch_one(root, synthesized, commands);
            }
        }
        self.dispatching = dispatching;
    }

    /// Distribue un événement et retient la commande qu'il produit.
    fn dispatch_one(
        &mut self,
        root: &mut dyn AirView,
        input: AirInputEvent,
        commands: &mut Vec<AirCommand>,
    ) {
        let mut event = AirEvent::new(input);
        let mut context = AirContext {
            commands: &mut self.commands,
        };
        if let AirOutcome::Command(command) = dispatch_event(root, &mut event, &mut context) {
            commands.push(command);
        }
    }

    /// L'événement de clic multiple qu'un enfoncement termine, s'il en termine un.
    fn multiple_click(&mut self, input: &AirInputEvent, now: Instant) -> Option<AirInputEvent> {
        let AirInputEvent::Mouse(mouse) = input else {
            return None;
        };
        let AirMouseEventKind::Pressed(button) = mouse.kind else {
            return None;
        };
        let count = self
            .clicks
            .press(button, mouse.position, now, |later, earlier| {
                later.saturating_duration_since(earlier)
            });
        let kind = extra_event(count, button)?;
        Some(AirInputEvent::Mouse(AirMouseEvent { kind, ..*mouse }))
    }
}

impl core::fmt::Debug for AirApplication {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Le terminal, la surface et le thème ne s'affichent pas : ce qui intéresse une
        // trace, c'est la dimension, la profondeur de pile et ce qui reste à décoder.
        formatter
            .debug_struct("AirApplication")
            .field("size", &self.size)
            .field("pending", &self.pending.len())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use core::cell::RefCell;
    use core::time::Duration;
    use std::rc::Rc;

    use air_tui_platform::{
        InputReadiness, PlatformError, PlatformErrorKind, TerminalDevice, WindowSize,
    };

    use super::{AirApplication, AirPumpOutcome, AirScreen};
    use crate::application::AirDesktop;
    use crate::command::{AirCommand, QUIT, SAVE};
    use crate::geometry::{AirRectangle, AirSize};
    use crate::input::event::{AirInputEvent, AirKey, AirMouseButton, AirMouseEventKind};
    use crate::theme::AirTheme;
    use crate::view::{AirView, AirViewId, AirViewState};
    use crate::widgets::{AirMenu, AirMenuBar, AirMenuItem, AirStatusItem, AirStatusLine};

    /// L'état partagé d'un périphérique en mémoire.
    ///
    /// C'est tout l'intérêt de la couture par traits (ADR-011) : la boucle d'une
    /// application se vérifie **sans terminal**, de façon déterministe.
    #[derive(Default)]
    struct DeviceState {
        /// La dimension annoncée.
        size: Option<WindowSize>,
        /// Ce qui a été écrit.
        written: Vec<u8>,
        /// Les lectures à rendre, dans l'ordre : chacune est un tour de boucle.
        reads: Vec<Vec<u8>>,
        /// Ce que l'attente doit rendre, dans l'ordre.
        readiness: Vec<InputReadiness>,
        /// Vrai si le mode a été restauré.
        restored: bool,
    }

    /// Un périphérique en mémoire, piloté par un état partagé.
    struct MemoryDevice {
        /// L'état, partagé avec le test.
        state: Rc<RefCell<DeviceState>>,
    }

    impl TerminalDevice for MemoryDevice {
        fn window_size(&self) -> Result<Option<WindowSize>, PlatformError> {
            Ok(self.state.borrow().size)
        }

        fn enter_raw_mode(&mut self) -> Result<(), PlatformError> {
            Ok(())
        }

        fn restore_mode(&mut self) -> Result<(), PlatformError> {
            self.state.borrow_mut().restored = true;
            Ok(())
        }

        fn wait_for_input(
            &mut self,
            _timeout: Option<Duration>,
        ) -> Result<InputReadiness, PlatformError> {
            let mut state = self.state.borrow_mut();
            if state.readiness.is_empty() {
                return Ok(InputReadiness::Closed);
            }
            Ok(state.readiness.remove(0))
        }

        fn read_input(&mut self, buffer: &mut [u8]) -> Result<usize, PlatformError> {
            let mut state = self.state.borrow_mut();
            if state.reads.is_empty() {
                return Ok(0);
            }
            let bytes = state.reads.remove(0);
            let count = bytes.len().min(buffer.len());
            let Some(slot) = buffer.get_mut(..count) else {
                return Err(PlatformError::new(PlatformErrorKind::Other));
            };
            let Some(taken) = bytes.get(..count) else {
                return Err(PlatformError::new(PlatformErrorKind::Other));
            };
            slot.copy_from_slice(taken);
            Ok(count)
        }

        fn write_output(&mut self, bytes: &[u8]) -> Result<(), PlatformError> {
            self.state.borrow_mut().written.extend_from_slice(bytes);
            Ok(())
        }

        fn flush_output(&mut self) -> Result<(), PlatformError> {
            Ok(())
        }
    }

    fn identifier(value: u32) -> AirViewId {
        AirViewId::new(value).expect("un identifiant non nul")
    }

    fn screen_with_chrome(value: u32) -> AirScreen {
        let mut screen = AirScreen::new(AirViewState::new(
            identifier(value),
            AirRectangle::from_parts(0, 0, 1, 1),
        ));
        screen.set_desktop(Box::new(AirDesktop::new(AirViewState::new(
            identifier(value.saturating_add(100)),
            AirRectangle::from_parts(0, 0, 1, 1),
        ))));
        screen.set_status_line(Box::new(
            AirStatusLine::new(AirViewState::new(
                identifier(value.saturating_add(200)),
                AirRectangle::from_parts(0, 0, 1, 1),
            ))
            .with_item(AirStatusItem::new("~F2~ Save", AirKey::Function(2), SAVE))
            .with_item(AirStatusItem::new("~F10~ Exit", AirKey::Function(10), QUIT)),
        ));
        screen.set_menu_bar(Box::new(
            AirMenuBar::new(AirViewState::new(
                identifier(value.saturating_add(300)),
                AirRectangle::from_parts(0, 0, 1, 1),
            ))
            .with_menu(AirMenu::new("~F~ile").with_item(AirMenuItem::new("E~x~it", QUIT))),
        ));
        screen
    }

    /// Une application sur un périphérique en mémoire, son écran, et l'état du
    /// périphérique.
    ///
    /// L'écran est rendu séparément : il appartient à l'appelant, comme dans une vraie
    /// application.
    fn application(
        columns: u16,
        rows: u16,
        readiness: Vec<InputReadiness>,
        reads: Vec<&str>,
    ) -> (AirApplication, AirScreen, Rc<RefCell<DeviceState>>) {
        let state = Rc::new(RefCell::new(DeviceState {
            size: Some(WindowSize { columns, rows }),
            reads: reads
                .into_iter()
                .map(|text| text.as_bytes().to_vec())
                .collect(),
            readiness,
            ..DeviceState::default()
        }));
        let device = MemoryDevice {
            state: Rc::clone(&state),
        };
        let application = AirApplication::new(Box::new(device), AirTheme::turbo_vision())
            .expect("le périphérique en mémoire accepte le mode brut");
        let mut screen = screen_with_chrome(1);
        screen.resize(application.size());
        (application, screen, state)
    }

    #[test]
    fn une_application_neuve_lit_la_dimension_du_terminal() {
        let (application, screen, _state) = application(80, 25, Vec::new(), Vec::new());
        assert_eq!(application.size(), AirSize::new(80, 25));
        // L'écran appartient à l'appelant : c'est lui qui l'a redimensionné.
        assert_eq!(screen.size(), AirSize::new(80, 25));
    }

    #[test]
    fn le_dessin_ecrit_sur_le_terminal() {
        let (mut application, screen, state) = application(20, 5, Vec::new(), Vec::new());
        application.draw(&screen).expect("le dessin aboutit");
        assert_ne!(state.borrow().written, [] as [u8; 0]);
    }

    #[test]
    fn une_touche_de_fonction_rend_sa_commande() {
        // `F10` est publiée par la barre de statut, en post-traitement.
        let (mut application, mut screen, _state) =
            application(40, 10, vec![InputReadiness::Ready], vec!["\u{1b}[21~"]);
        let mut commands: Vec<AirCommand> = Vec::new();
        let outcome = application
            .pump(&mut screen, Some(Duration::from_millis(1)), &mut commands)
            .expect("le tour aboutit");
        assert_eq!(outcome, AirPumpOutcome::Continued);
        assert_eq!(commands, vec![QUIT]);
    }

    #[test]
    fn les_commandes_sont_videes_a_chaque_tour() {
        // Sans ce vidage, une commande déjà traitée serait rejouée au tour suivant.
        let (mut application, mut screen, _state) = application(
            40,
            10,
            vec![InputReadiness::Ready, InputReadiness::TimedOut],
            vec!["\u{1b}[21~"],
        );
        let mut commands: Vec<AirCommand> = Vec::new();
        application
            .pump(&mut screen, Some(Duration::from_millis(1)), &mut commands)
            .expect("le premier tour aboutit");
        assert_eq!(commands, vec![QUIT]);
        application
            .pump(&mut screen, Some(Duration::from_millis(1)), &mut commands)
            .expect("le second tour aboutit");
        assert_eq!(commands, [] as [AirCommand; 0]);
    }

    #[test]
    fn une_attente_expiree_continue_sans_commande() {
        let (mut application, mut screen, _state) =
            application(40, 10, vec![InputReadiness::TimedOut], Vec::new());
        let mut commands: Vec<AirCommand> = Vec::new();
        assert_eq!(
            application
                .pump(&mut screen, Some(Duration::from_millis(1)), &mut commands)
                .expect("le tour aboutit"),
            AirPumpOutcome::Continued
        );
        assert_eq!(commands, [] as [AirCommand; 0]);
    }

    #[test]
    fn une_fermeture_annoncee_arrete_la_boucle() {
        let (mut application, mut screen, _state) =
            application(40, 10, vec![InputReadiness::Closed], Vec::new());
        let mut commands: Vec<AirCommand> = Vec::new();
        assert_eq!(
            application
                .pump(&mut screen, None, &mut commands)
                .expect("le tour aboutit"),
            AirPumpOutcome::Closed
        );
    }

    #[test]
    fn un_prêt_sans_octet_est_une_fermeture() {
        // Le défaut de l'exemple du jalon M5b : selon la plateforme, la fin de flux se
        // signale par `POLLHUP` ou par un `POLLIN` qui ne rend rien.
        let (mut application, mut screen, _state) =
            application(40, 10, vec![InputReadiness::Ready], Vec::new());
        let mut commands: Vec<AirCommand> = Vec::new();
        assert_eq!(
            application
                .pump(&mut screen, None, &mut commands)
                .expect("le tour aboutit"),
            AirPumpOutcome::Closed
        );
    }

    #[test]
    fn un_redimensionnement_du_terminal_replace_l_ecran() {
        let (mut application, mut screen, state) =
            application(40, 10, vec![InputReadiness::TimedOut], Vec::new());
        state.borrow_mut().size = Some(WindowSize {
            columns: 100,
            rows: 30,
        });
        let mut commands: Vec<AirCommand> = Vec::new();
        application
            .pump(&mut screen, Some(Duration::from_millis(1)), &mut commands)
            .expect("le tour aboutit");
        assert_eq!(application.size(), AirSize::new(100, 30));
        // L'arbre passé à `pump` a été redimensionné par `AirView::resize`, que l'écran
        // redéfinit pour replacer ses pièces.
        assert_eq!(screen.size(), AirSize::new(100, 30));
    }

    #[test]
    fn relire_la_dimension_sans_changement_ne_rend_rien() {
        let (mut application, mut screen, _state) = application(40, 10, Vec::new(), Vec::new());
        assert!(
            !application
                .refresh_size(&mut screen)
                .expect("la dimension est lue")
        );
    }

    #[test]
    fn une_sequence_coupee_en_deux_lectures_produit_un_seul_evenement() {
        // La propriété qui compte sur un terminal : une séquence arrive coupée sans
        // prévenir, et le décodeur garde le reste jusqu'au tour suivant.
        let (mut application, mut screen, _state) = application(
            40,
            10,
            vec![InputReadiness::Ready, InputReadiness::Ready],
            vec!["\u{1b}[2", "1~"],
        );
        let mut commands: Vec<AirCommand> = Vec::new();
        application
            .pump(&mut screen, None, &mut commands)
            .expect("le premier tour aboutit");
        assert_eq!(commands, [] as [AirCommand; 0]);
        application
            .pump(&mut screen, None, &mut commands)
            .expect("le second tour aboutit");
        assert_eq!(commands, vec![QUIT]);
    }

    #[test]
    fn plusieurs_arbres_se_dessinent_tour_a_tour() {
        // La pile d'écrans appartient à l'appelant : il tient autant d'arbres qu'il veut
        // et passe celui qu'il dessine. C'est ce que l'application possédait avant, et
        // elle n'avait aucune raison de le posséder.
        let (mut application, first, _state) = application(40, 10, Vec::new(), Vec::new());
        let mut second = screen_with_chrome(2);
        second.resize(application.size());
        application.draw(&first).expect("le premier dessin aboutit");
        application.draw(&second).expect("le second dessin aboutit");
        assert_eq!(second.size(), AirSize::new(40, 10));
    }

    #[test]
    fn le_theme_se_change_a_chaud() {
        let (mut application, screen, _state) = application(40, 10, Vec::new(), Vec::new());
        application.set_theme(AirTheme::turbo_vision());
        application.draw(&screen).expect("le dessin aboutit");
        assert_eq!(
            application.theme().glyphs.desktop_fill,
            AirTheme::turbo_vision().glyphs.desktop_fill
        );
    }

    #[test]
    fn le_jeu_de_commandes_est_accessible_dans_les_deux_sens() {
        let (mut application, _screen, _state) = application(40, 10, Vec::new(), Vec::new());
        application.commands_mut().disable(SAVE);
        assert!(!application.commands().is_enabled(SAVE));
    }

    #[test]
    fn une_commande_desactivee_n_est_pas_emise() {
        // Une seule source, deux chemins : le grisage et le refus d'activer lisent le même
        // jeu (ADR-007).
        let (mut application, mut screen, _state) =
            application(40, 10, vec![InputReadiness::Ready], vec!["\u{1b}[21~"]);
        application.commands_mut().disable(QUIT);
        let mut commands: Vec<AirCommand> = Vec::new();
        application
            .pump(&mut screen, None, &mut commands)
            .expect("le tour aboutit");
        assert_eq!(commands, [] as [AirCommand; 0]);
    }

    #[test]
    fn une_entree_indecodable_ne_fige_pas_la_boucle() {
        // Un octet de continuation UTF-8 isolé : aucune suite ne peut le compléter.
        let (mut application, mut screen, _state) =
            application(40, 10, vec![InputReadiness::Ready], vec!["\u{feff}"]);
        let mut commands: Vec<AirCommand> = Vec::new();
        assert_eq!(
            application
                .pump(&mut screen, None, &mut commands)
                .expect("le tour aboutit"),
            AirPumpOutcome::Continued
        );
    }

    #[test]
    fn la_negociation_du_protocole_clavier_ecrit_sur_le_terminal() {
        let (mut application, _screen, state) = application(40, 10, Vec::new(), Vec::new());
        let before = state.borrow().written.len();
        let _ = application.enable_best_keyboard_protocol(Duration::from_millis(1));
        assert!(state.borrow().written.len() > before);
    }

    #[test]
    fn le_terminal_est_restaure_a_la_destruction() {
        let (application, _screen, state) = application(40, 10, Vec::new(), Vec::new());
        drop(application);
        assert!(state.borrow().restored);
    }

    #[test]
    fn une_application_s_affiche_avec_sa_dimension() {
        let (application, _screen, _state) = application(40, 10, Vec::new(), Vec::new());
        let text = format!("{application:?}");
        assert!(text.contains("AirApplication"), "rendu : {text}");
        assert!(text.contains("size: AirSize"), "rendu : {text}");
    }

    // ------------------------------------------- la synthèse du clic multiple

    /// Une vue qui note les natures d'événement de souris qu'elle reçoit.
    struct MouseSpy {
        /// L'état commun.
        state: AirViewState,
        /// Ce qu'elle a vu, dans l'ordre.
        seen: Rc<RefCell<Vec<AirMouseEventKind>>>,
    }

    impl AirView for MouseSpy {
        fn state(&self) -> &AirViewState {
            &self.state
        }

        fn state_mut(&mut self) -> &mut AirViewState {
            &mut self.state
        }

        fn draw_content(
            &self,
            painter: &mut crate::view::painter::AirPainter<'_>,
            context: &crate::view::AirDrawContext<'_>,
        ) {
            let _ = (painter, context);
        }

        fn handle_event(
            &mut self,
            event: &mut crate::view::AirEvent,
            context: &mut crate::view::AirContext<'_>,
        ) -> crate::view::AirOutcome {
            let _ = context;
            if let Some(AirInputEvent::Mouse(mouse)) = event.input() {
                self.seen.borrow_mut().push(mouse.kind);
                event.consume();
                return crate::view::AirOutcome::Consumed;
            }
            crate::view::AirOutcome::Ignored
        }
    }

    /// L'enfoncement du bouton gauche en haut à gauche, au format SGR 1006.
    const LEFT_PRESS: &str = "\u{1b}[<0;1;1M";

    fn mouse_spy() -> (MouseSpy, Rc<RefCell<Vec<AirMouseEventKind>>>) {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let spy = MouseSpy {
            state: AirViewState::new(AirViewId::FIRST, AirRectangle::from_parts(0, 0, 40, 10)),
            seen: Rc::clone(&seen),
        };
        (spy, seen)
    }

    #[test]
    fn deux_clics_dans_le_meme_tour_synthetisent_un_double() {
        // Les deux enfoncements arrivent dans la même lecture, donc au même instant : le
        // délai est nul, et la règle de Borland s'applique. L'essai est déterministe sans
        // faire dormir personne.
        let (mut application, _screen, _state) = application(
            40,
            10,
            vec![InputReadiness::Ready],
            vec![&format!("{LEFT_PRESS}{LEFT_PRESS}")],
        );
        let (mut spy, seen) = mouse_spy();
        let mut commands: Vec<AirCommand> = Vec::new();
        application
            .pump(&mut spy, None, &mut commands)
            .expect("le tour aboutit");

        // L'enfoncement **précède** son double clic : c'est lui qui place le curseur, et
        // le double qui étend ensuite.
        assert_eq!(
            seen.borrow().as_slice(),
            [
                AirMouseEventKind::Pressed(AirMouseButton::Left),
                AirMouseEventKind::Pressed(AirMouseButton::Left),
                AirMouseEventKind::DoubleClicked(AirMouseButton::Left),
            ]
        );
    }

    #[test]
    fn trois_clics_dans_le_meme_tour_synthetisent_un_triple() {
        let (mut application, _screen, _state) = application(
            40,
            10,
            vec![InputReadiness::Ready],
            vec![&format!("{LEFT_PRESS}{LEFT_PRESS}{LEFT_PRESS}")],
        );
        let (mut spy, seen) = mouse_spy();
        let mut commands: Vec<AirCommand> = Vec::new();
        application
            .pump(&mut spy, None, &mut commands)
            .expect("le tour aboutit");

        // La suite exacte, et elle dit tout : chaque enfoncement passe, le deuxième est
        // suivi d'un double, le troisième d'un triple — et d'un triple **seulement**, le
        // double n'étant pas réémis.
        assert_eq!(
            seen.borrow().as_slice(),
            [
                AirMouseEventKind::Pressed(AirMouseButton::Left),
                AirMouseEventKind::Pressed(AirMouseButton::Left),
                AirMouseEventKind::DoubleClicked(AirMouseButton::Left),
                AirMouseEventKind::Pressed(AirMouseButton::Left),
                AirMouseEventKind::TripleClicked(AirMouseButton::Left),
            ]
        );
    }

    #[test]
    fn un_clic_isole_ne_synthetise_rien() {
        let (mut application, _screen, _state) =
            application(40, 10, vec![InputReadiness::Ready], vec![LEFT_PRESS]);
        let (mut spy, seen) = mouse_spy();
        let mut commands: Vec<AirCommand> = Vec::new();
        application
            .pump(&mut spy, None, &mut commands)
            .expect("le tour aboutit");
        assert_eq!(
            seen.borrow().as_slice(),
            [AirMouseEventKind::Pressed(AirMouseButton::Left)]
        );
    }

    #[test]
    fn un_redimensionnement_coupe_l_enchainement_des_clics() {
        // La cellule d'avant ne désigne plus la même chose : enchaîner dessus ferait un
        // double clic sur un autre widget.
        let (mut application, _screen, state) = application(
            40,
            10,
            vec![InputReadiness::Ready, InputReadiness::Ready],
            vec![LEFT_PRESS, LEFT_PRESS],
        );
        let (mut spy, seen) = mouse_spy();
        let mut commands: Vec<AirCommand> = Vec::new();
        application
            .pump(&mut spy, None, &mut commands)
            .expect("le premier tour aboutit");
        state.borrow_mut().size = Some(WindowSize {
            columns: 50,
            rows: 12,
        });
        application
            .pump(&mut spy, None, &mut commands)
            .expect("le second tour aboutit");
        assert!(
            !seen
                .borrow()
                .contains(&AirMouseEventKind::DoubleClicked(AirMouseButton::Left)),
            "l'enchaînement a survécu au redimensionnement : {:?}",
            seen.borrow()
        );
    }
}
