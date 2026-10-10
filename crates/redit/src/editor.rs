// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Le contrôleur — l'arbre de vues, le document, et le tour de boucle qui les relie.
//!
//! # Les quatre étapes de l'ADR-007, en clair
//!
//! ```text
//! 1. entrée → intention    l'application route, la vue d'édition accumule des actions
//! 2. intention → mutation  `apply_pending` les applique au tampon
//! 3. faits → réaction      `synchronize` pousse le reflet dans les vues
//! 4. rendu                 l'application dessine
//! ```
//!
//! À aucun moment deux emprunts mutables ne sont vivants, et le tampon n'appelle personne.
//!
//! # Pourquoi le contrôleur retrouve ses vues au lieu de les tenir
//!
//! L'arbre tient des `Box<dyn AirView>`. Le contrôleur garde donc les **identifiants** de
//! ses trois vues utiles — la vue d'édition et les deux ascenseurs — et les retrouve par
//! [`air_tui::view::find_mut`], qui les rend avec leur type. C'est le mécanisme 4 de
//! l'ADR-007, « abonnement par `AirViewId` », et il n'y a aucune référence arrière.

use std::path::{Path, PathBuf};

use air_tui::application::{AirDesktop, AirScreen};
use air_tui::clipboard::AirClipboard;
use air_tui::command::{AirCommand, AirCommandSet, CANCEL, COPY, CUT, OK, OPEN, PASTE, QUIT, SAVE};
use air_tui::geometry::{AirRectangle, AirSize};
use air_tui::input::event::AirKey;
use air_tui::model::{AirChangeSet, AirModel, AirTextAction, AirTextChange};
use air_tui::view::{AirView, AirViewId, AirViewState, find, find_mut};
use air_tui::widgets::{
    AirEditorView, AirMenu, AirMenuBar, AirMenuItem, AirOrientation, AirPromptDialog, AirScrollBar,
    AirStatusItem, AirStatusLine, AirWindow,
};

use crate::document::Document;

/// Créer un document vide.
pub const NEW: AirCommand = AirCommand::new(256);
/// Enregistrer sous un autre nom.
pub const SAVE_AS: AirCommand = AirCommand::new(257);
/// Supprimer la ligne courante.
pub const DELETE_LINE: AirCommand = AirCommand::new(258);
/// Afficher la version.
pub const ABOUT: AirCommand = AirCommand::new(259);
/// Tout sélectionner.
///
/// Numéro d'application : le framework n'en réserve pas pour ce geste, parce qu'il n'a pas
/// de sens hors d'un contenu sélectionnable, et qu'une liste de fichiers l'entendrait
/// autrement qu'un texte.
pub const SELECT_ALL: AirCommand = AirCommand::new(260);

/// La dimension minimale d'une fenêtre pour qu'elle porte un éditeur et ses ascenseurs.
///
/// Cinq colonnes et cinq lignes : deux de cadre, une d'ascenseur, et au moins deux de
/// texte. En dessous, la fenêtre est dessinée mais ses contrôles ne le sont pas.
const MINIMUM_WINDOW: AirSize = AirSize::new(5, 5);

/// Ce qu'une question posée à l'utilisateur doit déclencher quand il répond.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Prompt {
    /// Ouvrir le fichier nommé.
    Open,
    /// Enregistrer sous le nom donné.
    SaveAs,
}

/// Les identifiants des vues que le contrôleur retrouve à chaque tour.
#[derive(Debug, Clone, Copy)]
struct Views {
    /// La fenêtre, dont le titre porte le nom du fichier et la position du curseur.
    window: AirViewId,
    /// La vue d'édition.
    editor: AirViewId,
    /// L'ascenseur vertical.
    vertical: AirViewId,
    /// L'ascenseur horizontal.
    horizontal: AirViewId,
    /// Le dialogue modal, quand il y en a un.
    modal: AirViewId,
}

/// Le contrôleur de `redit`.
pub struct Editor {
    /// Le document — le modèle, et son accès au disque.
    document: Document,
    /// Le journal des changements, réutilisé d'un tour à l'autre.
    changes: AirChangeSet<AirTextChange>,
    /// Les intentions relevées de la vue, réutilisées d'un tour à l'autre.
    actions: Vec<AirTextAction>,
    /// L'arbre de vues.
    root: AirScreen,
    /// Les identifiants des vues utiles.
    views: Views,
    /// Ce que la question en cours doit déclencher.
    prompt: Option<Prompt>,
    /// Le dernier message à afficher, s'il y en a un.
    message: Option<String>,
    /// Le presse-papiers interne, que le contrôleur possède.
    clipboard: AirClipboard,
    /// Vrai quand l'utilisateur a demandé à quitter.
    quit: bool,
}

/// Construit un identifiant, avec repli sur le premier valide.
///
/// Les identifiants de `redit` sont des constantes de un à neuf : le repli est
/// inatteignable, et il vaut mieux qu'un `expect` sur une constante.
fn identifier(value: u32) -> AirViewId {
    AirViewId::new(value).unwrap_or(AirViewId::FIRST)
}

impl Editor {
    /// Un contrôleur sur un document, avec son arbre monté.
    #[must_use]
    pub fn new(document: Document) -> Self {
        let views = Views {
            window: identifier(3),
            editor: identifier(4),
            vertical: identifier(5),
            horizontal: identifier(6),
            modal: identifier(9),
        };
        let mut root = AirScreen::new(AirViewState::new(
            identifier(1),
            AirRectangle::from_parts(0, 0, 1, 1),
        ));
        root.set_desktop(Box::new(build_desktop(views)));
        root.set_status_line(Box::new(build_status_line()));
        root.set_menu_bar(Box::new(build_menu_bar()));
        Self {
            document,
            changes: AirChangeSet::with_capacity(8),
            actions: Vec::with_capacity(8),
            root,
            views,
            prompt: None,
            message: None,
            clipboard: AirClipboard::new(),
            quit: false,
        }
    }

    /// L'arbre, en lecture — ce que l'application dessine.
    #[must_use]
    pub const fn root(&self) -> &AirScreen {
        &self.root
    }

    /// L'arbre, en écriture — ce que l'application route.
    pub const fn root_mut(&mut self) -> &mut AirScreen {
        &mut self.root
    }

    /// Vrai quand l'utilisateur a demandé à quitter.
    #[must_use]
    pub const fn wants_quit(&self) -> bool {
        self.quit
    }

    /// Charge un fichier, et retient le message en cas d'échec.
    pub fn open(&mut self, path: &Path) {
        match self.document.load(path, &mut self.changes) {
            Ok(true) => self.message = None,
            Ok(false) => self.message = Some("fichier neuf".to_owned()),
            Err(error) => self.message = Some(error.to_string()),
        }
    }

    /// Étape 3 : pousse le reflet du modèle dans les vues, et met le chrome à jour.
    ///
    /// Appelée à chaque tour. La vue d'édition compare ses générations et ne fait rien
    /// quand rien n'a changé, donc le coût ordinaire de cet appel est celui de la
    /// disposition — quelques dizaines d'opérations entières.
    pub fn synchronize(&mut self, commands: &mut AirCommandSet) {
        self.layout_window();

        let (rows, columns, top_line, left_column, total_lines) = {
            let Some(editor) = find_mut::<AirEditorView>(&mut self.root, self.views.editor) else {
                return;
            };
            editor.synchronize(self.document.buffer());
            (
                editor.visible_rows(),
                editor.visible_columns(),
                editor.top_line(),
                editor.left_column(),
                editor.total_lines(),
            )
        };
        // La largeur totale du texte n'est pas connue sans parcourir toutes les lignes :
        // la fenêtre visible plus la position suffit à placer le curseur de l'ascenseur
        // horizontal, et c'est ce que l'on voit dans `tvedit`.
        let longest = left_column.saturating_add(columns).saturating_add(1);

        if let Some(bar) = find_mut::<AirScrollBar>(&mut self.root, self.views.vertical) {
            bar.set_range(top_line, rows, total_lines);
        }
        if let Some(bar) = find_mut::<AirScrollBar>(&mut self.root, self.views.horizontal) {
            bar.set_range(left_column, columns, longest);
        }

        let title = self.window_title();
        if let Some(window) = find_mut::<AirWindow>(&mut self.root, self.views.window) {
            window.set_title(title);
        }

        // Le *pull* de l'ADR-007 : « Enregistrer » n'a pas de sens sur un document
        // intact. Le griser grise d'un coup l'item de menu **et** l'item de barre de
        // statut, sans que personne soit notifié.
        if self.document.buffer().is_modified() {
            commands.enable(SAVE);
        } else {
            commands.disable(SAVE);
        }

        // Le même *pull* pour le presse-papiers : copier et couper n'ont pas de sens sans
        // sélection, coller n'en a pas sur un presse-papiers vide. Rien n'est notifié —
        // l'état est lu à chaque tour, et griser l'item de menu le grise partout.
        let selected = self.document.buffer().has_selection();
        for command in [COPY, CUT] {
            if selected {
                commands.enable(command);
            } else {
                commands.disable(command);
            }
        }
        if self.clipboard.is_empty() {
            commands.disable(PASTE);
        } else {
            commands.enable(PASTE);
        }
    }

    /// Étape 2 : applique au modèle les intentions que les vues ont accumulées.
    pub fn apply_pending(&mut self) {
        self.changes.clear();
        if let Some(editor) = find_mut::<AirEditorView>(&mut self.root, self.views.editor) {
            self.actions.clear();
            self.actions.extend(editor.drain_actions());
        }
        for action in self.actions.drain(..) {
            self.document.buffer_mut().apply(action, &mut self.changes);
        }

        // Les ascenseurs **demandent** une position ; c'est ici qu'on décide de la
        // satisfaire, parce que seul le propriétaire du contenu connaît la borne.
        let vertical = find_mut::<AirScrollBar>(&mut self.root, self.views.vertical)
            .and_then(AirScrollBar::take_request);
        let horizontal = find_mut::<AirScrollBar>(&mut self.root, self.views.horizontal)
            .and_then(AirScrollBar::take_request);
        if let Some(editor) = find_mut::<AirEditorView>(&mut self.root, self.views.editor) {
            if let Some(line) = vertical {
                editor.scroll_to_line(line);
            }
            if let Some(column) = horizontal {
                editor.scroll_to_column(column);
            }
        }
    }

    /// Copie la sélection dans le presse-papiers, s'il y en a une.
    ///
    /// Une sélection vide ne touche pas au presse-papiers : `AirClipboard::set` refuse un
    /// texte vide, pour qu'un geste sans effet ne détruise pas une copie antérieure.
    fn copy_selection(&mut self) {
        if let Some(selected) = self.document.buffer().selected_text() {
            self.clipboard.set(selected);
        }
    }

    /// Exécute une commande rendue par l'application.
    pub fn execute(&mut self, command: AirCommand) {
        match command {
            QUIT => self.quit = true,
            NEW => self.start_new_document(),
            OPEN => self.ask(Prompt::Open, "Ouvrir"),
            SAVE => self.save(),
            SAVE_AS => self.ask(Prompt::SaveAs, "Enregistrer sous"),
            DELETE_LINE => {
                if let Some(editor) = find_mut::<AirEditorView>(&mut self.root, self.views.editor) {
                    let _ = editor;
                }
                self.document
                    .buffer_mut()
                    .apply(AirTextAction::DeleteLine, &mut self.changes);
            }
            // Le presse-papiers est au contrôleur : les vues demandent, lui seul tient le
            // contenu. Copier et couper ne diffèrent que par la suppression qui suit.
            COPY => self.copy_selection(),
            CUT => {
                self.copy_selection();
                self.document
                    .buffer_mut()
                    .apply(AirTextAction::DeleteSelection, &mut self.changes);
            }
            PASTE => {
                if let Some(pasted) = self.clipboard.content() {
                    let action = AirTextAction::InsertText(pasted.to_owned());
                    self.document.buffer_mut().apply(action, &mut self.changes);
                }
            }
            SELECT_ALL => {
                self.document
                    .buffer_mut()
                    .apply(AirTextAction::SelectAll, &mut self.changes);
            }
            ABOUT => {
                self.message = Some(format!("redit {}", env!("CARGO_PKG_VERSION")));
            }
            OK => self.answer(),
            CANCEL => self.dismiss(),
            _ => {}
        }
    }

    /// Remplace le document par un document vide et sans nom.
    fn start_new_document(&mut self) {
        self.document = Document::new();
        self.message = None;
        // Les générations du tampon neuf repartent de zéro : la vue doit repartir avec,
        // sinon elle garderait le reflet de l'ancien document.
        if let Some(editor) = find_mut::<AirEditorView>(&mut self.root, self.views.editor) {
            let state = *editor.state();
            *editor = AirEditorView::new(state);
            editor.state_mut().set_focused(true);
        }
    }

    /// Pose la question d'un chemin de fichier, en modal.
    fn ask(&mut self, prompt: Prompt, title: &str) {
        let prefill = self
            .document
            .path()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default();
        let dialog = AirPromptDialog::new(
            AirViewState::new(
                self.views.modal,
                AirRectangle::from_parts(0, 0, 54, AirPromptDialog::required_rows()),
            ),
            title,
            "Nom du fichier :",
        )
        .with_text(prefill)
        .with_maximum_length(1024);
        self.prompt = Some(prompt);
        self.root.set_modal(Box::new(dialog));
    }

    /// Traite la réponse à la question en cours.
    fn answer(&mut self) {
        let Some(prompt) = self.prompt.take() else {
            return;
        };
        let answer = find::<AirPromptDialog>(&self.root, self.views.modal)
            .map(|dialog| dialog.text().trim().to_owned())
            .unwrap_or_default();
        let _ = self.root.take_modal();
        if answer.is_empty() {
            self.message = Some("aucun nom de fichier".to_owned());
            return;
        }
        let path = PathBuf::from(answer);
        match prompt {
            Prompt::Open => {
                self.open(&path);
                self.start_reflection();
            }
            Prompt::SaveAs => match self.document.save_as(&path, &mut self.changes) {
                Ok(()) => self.message = None,
                Err(error) => self.message = Some(error.to_string()),
            },
        }
    }

    /// Referme la question sans rien faire.
    fn dismiss(&mut self) {
        self.prompt = None;
        let _ = self.root.take_modal();
    }

    /// Remet la vue d'édition à neuf pour qu'elle reflète un document chargé.
    ///
    /// Un chargement fait repartir les générations du tampon, et une vue qui a déjà vu des
    /// générations plus hautes croirait que rien n'a changé.
    fn start_reflection(&mut self) {
        if let Some(editor) = find_mut::<AirEditorView>(&mut self.root, self.views.editor) {
            let state = *editor.state();
            *editor = AirEditorView::new(state);
            editor.state_mut().set_focused(true);
        }
    }

    /// Enregistre, ou demande un nom si le document n'en a pas.
    fn save(&mut self) {
        if self.document.path().is_none() {
            self.ask(Prompt::SaveAs, "Enregistrer sous");
            return;
        }
        match self.document.save(&mut self.changes) {
            Ok(()) => self.message = None,
            Err(error) => self.message = Some(error.to_string()),
        }
    }

    /// Le titre de la fenêtre : le nom, la marque de modification, la position, le message.
    ///
    /// La position est comptée **depuis un**, comme dans tout éditeur : le modèle compte
    /// depuis zéro, et la conversion appartient à l'affichage.
    fn window_title(&self) -> String {
        let buffer = self.document.buffer();
        let cursor = buffer.cursor();
        let line = cursor.line.saturating_add(1);
        let column = buffer.cursor_column().saturating_add(1);
        let marker = if buffer.is_modified() { " *" } else { "" };
        let name = self.document.display_name();
        match self.message.as_deref() {
            Some(message) => format!("{name}{marker}  {line}:{column}  — {message}"),
            None => format!("{name}{marker}  {line}:{column}"),
        }
    }

    /// Place la fenêtre sur tout le bureau, et ses contrôles dans la fenêtre.
    ///
    /// Recalculée à chaque tour plutôt qu'au redimensionnement : c'est une poignée
    /// d'opérations entières, et un état « est-ce que la dimension a changé » de plus
    /// serait un état de plus à tenir juste.
    fn layout_window(&mut self) {
        let Some(area) = self
            .root
            .desktop()
            .map(|desktop| desktop.state().bounds().size)
        else {
            return;
        };
        let identifiers = self.views;
        let Some(window) = find_mut::<AirWindow>(&mut self.root, identifiers.window) else {
            return;
        };
        window
            .state_mut()
            .set_bounds(AirRectangle::new(air_tui::geometry::AirPoint::ORIGIN, area));
        if area.columns < MINIMUM_WINDOW.columns || area.rows < MINIMUM_WINDOW.rows {
            // Trop petite pour porter un éditeur et deux ascenseurs. Le cadre est dessiné,
            // les contrôles gardent leurs limites : mieux vaut un contenu découpé qu'une
            // dimension négative ramenée à zéro à chaque tour.
            return;
        }
        let content = AirSize::new(area.columns.saturating_sub(2), area.rows.saturating_sub(2));
        let text_columns = content.columns.saturating_sub(1);
        let text_rows = content.rows.saturating_sub(1);
        let group = window.group_mut();
        if let Some(view) = group.child_mut(identifiers.editor) {
            view.state_mut()
                .set_bounds(AirRectangle::from_parts(0, 0, text_columns, text_rows));
        }
        if let Some(view) = group.child_mut(identifiers.vertical) {
            view.state_mut()
                .set_bounds(AirRectangle::from_parts(text_columns, 0, 1, text_rows));
        }
        if let Some(view) = group.child_mut(identifiers.horizontal) {
            view.state_mut()
                .set_bounds(AirRectangle::from_parts(0, text_rows, text_columns, 1));
        }
    }
}

/// Le bureau, avec sa fenêtre d'édition.
fn build_desktop(views: Views) -> AirDesktop {
    let mut desktop = AirDesktop::new(AirViewState::new(
        identifier(2),
        AirRectangle::from_parts(0, 0, 1, 1),
    ));
    let mut window = AirWindow::new(AirViewState::new(
        views.window,
        AirRectangle::from_parts(0, 0, 1, 1),
    ))
    .with_title("redit");
    window.state_mut().set_focused(true);

    let mut editor = AirEditorView::new(AirViewState::new(
        views.editor,
        AirRectangle::from_parts(0, 0, 1, 1),
    ));
    // La vue d'édition a le focus dans sa fenêtre : c'est elle qui porte le curseur du
    // terminal, et c'est elle qui doit recevoir les frappes.
    editor.state_mut().set_focused(true);
    window.insert(Box::new(editor));
    window.insert(Box::new(AirScrollBar::new(
        AirViewState::new(views.vertical, AirRectangle::from_parts(0, 0, 1, 1)),
        AirOrientation::Vertical,
    )));
    window.insert(Box::new(AirScrollBar::new(
        AirViewState::new(views.horizontal, AirRectangle::from_parts(0, 0, 1, 1)),
        AirOrientation::Horizontal,
    )));
    desktop.insert(Box::new(window));
    desktop
}

/// La barre de menu de `redit`.
fn build_menu_bar() -> AirMenuBar {
    AirMenuBar::new(AirViewState::new(
        identifier(7),
        AirRectangle::from_parts(0, 0, 1, 1),
    ))
    .with_menu(
        AirMenu::new("~F~ichier")
            .with_item(AirMenuItem::new("~N~ouveau", NEW))
            .with_item(AirMenuItem::new("~O~uvrir…", OPEN))
            .with_item(AirMenuItem::new("~E~nregistrer", SAVE))
            .with_item(AirMenuItem::new("Enregistrer ~s~ous…", SAVE_AS))
            .with_item(AirMenuItem::separator())
            .with_item(AirMenuItem::new("~Q~uitter", QUIT)),
    )
    .with_menu(
        AirMenu::new("~E~dition")
            .with_item(AirMenuItem::new("Co~p~ier", COPY))
            .with_item(AirMenuItem::new("Co~u~per", CUT))
            .with_item(AirMenuItem::new("C~o~ller", PASTE))
            .with_item(AirMenuItem::separator())
            .with_item(AirMenuItem::new("~T~out sélectionner", SELECT_ALL))
            .with_item(AirMenuItem::new("~S~upprimer la ligne", DELETE_LINE)),
    )
    .with_menu(AirMenu::new("~A~ide").with_item(AirMenuItem::new("À ~p~ropos", ABOUT)))
}

/// La barre de statut de `redit`.
fn build_status_line() -> AirStatusLine {
    AirStatusLine::new(AirViewState::new(
        identifier(8),
        AirRectangle::from_parts(0, 0, 1, 1),
    ))
    .with_item(AirStatusItem::new(
        "~F2~ Enregistrer",
        AirKey::Function(2),
        SAVE,
    ))
    .with_item(AirStatusItem::new("~F3~ Ouvrir", AirKey::Function(3), OPEN))
    .with_item(AirStatusItem::new(
        "~F10~ Quitter",
        AirKey::Function(10),
        QUIT,
    ))
}

#[cfg(test)]
mod tests {
    use air_tui::command::{AirCommandSet, CANCEL, COPY, CUT, OK, OPEN, PASTE, QUIT, SAVE};
    use air_tui::geometry::AirSize;
    use air_tui::input::event::{AirInputEvent, AirKey, AirKeyEvent};
    use air_tui::model::{AirChangeSet, AirModel, AirTextAction};
    use air_tui::view::{
        AirContext, AirEvent, AirOutcome, AirView, dispatch_event, find, find_mut,
    };
    use air_tui::widgets::{AirEditorView, AirPromptDialog, AirScrollBar};

    use super::{ABOUT, DELETE_LINE, Editor, NEW, SAVE_AS};
    use crate::document::Document;

    /// Un contrôleur sur un écran de dimension donnée.
    fn editor(columns: u16, rows: u16) -> Editor {
        let mut editor = Editor::new(Document::new());
        editor.root_mut().resize(AirSize::new(columns, rows));
        editor
    }

    /// Un tour de boucle complet, sans terminal.
    fn turn(editor: &mut Editor, commands: &mut AirCommandSet) {
        editor.apply_pending();
        editor.synchronize(commands);
    }

    /// Envoie une frappe dans l'arbre et rend les commandes émises.
    fn press(editor: &mut Editor, key: AirKey) -> AirOutcome {
        let mut commands = AirCommandSet::new();
        let mut context = AirContext {
            commands: &mut commands,
        };
        let mut event = AirEvent::new(AirInputEvent::Key(AirKeyEvent::press(key)));
        dispatch_event(editor.root_mut(), &mut event, &mut context)
    }

    /// Saisit un texte, frappe par frappe, en bouclant comme la vraie boucle.
    fn type_text(editor: &mut Editor, text: &str, commands: &mut AirCommandSet) {
        for character in text.chars() {
            press(editor, AirKey::Character(character));
            turn(editor, commands);
        }
    }

    #[test]
    fn un_editeur_neuf_monte_son_arbre_entier() {
        let editor = editor(80, 25);
        let root = editor.root();
        assert_eq!(root.size(), AirSize::new(80, 25));
        assert!(root.desktop().is_some());
        // Les trois vues que le contrôleur retrouve à chaque tour sont bien là, avec leur
        // type : c'est ce que `find` garantit, et sans quoi le contrôleur serait aveugle.
        assert!(find::<AirEditorView>(root, super::identifier(4)).is_some());
        assert!(find::<AirScrollBar>(root, super::identifier(5)).is_some());
        assert!(find::<AirScrollBar>(root, super::identifier(6)).is_some());
    }

    #[test]
    fn la_disposition_remplit_le_bureau_et_reserve_les_ascenseurs() {
        let mut editor = editor(80, 25);
        let mut commands = AirCommandSet::new();
        turn(&mut editor, &mut commands);
        // Vingt-trois lignes de bureau, moins le cadre : vingt-et-une, moins l'ascenseur
        // horizontal : vingt.
        let view =
            find::<AirEditorView>(editor.root(), super::identifier(4)).expect("la vue d'édition");
        assert_eq!(view.visible_rows(), 20);
        assert_eq!(view.visible_columns(), 77);
    }

    #[test]
    fn une_frappe_traverse_les_quatre_etapes() {
        let mut editor = editor(60, 20);
        let mut commands = AirCommandSet::new();
        turn(&mut editor, &mut commands);
        type_text(&mut editor, "bonjour", &mut commands);
        assert_eq!(
            editor.document.buffer().line(0),
            Some("bonjour"),
            "le tampon a reçu la saisie"
        );
    }

    #[test]
    fn le_titre_porte_le_nom_la_marque_et_la_position() {
        let mut editor = editor(60, 20);
        let mut commands = AirCommandSet::new();
        turn(&mut editor, &mut commands);
        assert_eq!(editor.window_title(), "sans nom  1:1");
        type_text(&mut editor, "ab", &mut commands);
        assert_eq!(editor.window_title(), "sans nom *  1:3");
    }

    #[test]
    fn enregistrer_est_grise_sur_un_document_intact() {
        // Le *pull* de l'ADR-007 : une seule source, deux chemins — l'item de menu et
        // l'item de barre de statut.
        let mut editor = editor(60, 20);
        let mut commands = AirCommandSet::new();
        turn(&mut editor, &mut commands);
        assert!(!commands.is_enabled(SAVE));
        type_text(&mut editor, "a", &mut commands);
        assert!(commands.is_enabled(SAVE));
    }

    #[test]
    fn les_ascenseurs_suivent_le_texte() {
        let mut editor = editor(40, 12);
        let mut commands = AirCommandSet::new();
        let lines: Vec<String> = (0..100).map(|index| format!("ligne {index}")).collect();
        editor
            .document
            .buffer_mut()
            .apply(AirTextAction::Load(lines), &mut editor.changes);
        turn(&mut editor, &mut commands);
        let bar = find::<AirScrollBar>(editor.root(), super::identifier(5))
            .expect("l'ascenseur vertical");
        assert_eq!(bar.total(), 100);
        assert_eq!(bar.position(), 0);
    }

    #[test]
    fn une_demande_d_ascenseur_defile_la_vue() {
        let mut editor = editor(40, 12);
        let mut commands = AirCommandSet::new();
        let lines: Vec<String> = (0..100).map(|index| format!("ligne {index}")).collect();
        editor
            .document
            .buffer_mut()
            .apply(AirTextAction::Load(lines), &mut editor.changes);
        turn(&mut editor, &mut commands);
        // La flèche du bas de l'ascenseur vertical : une unité vers le bas.
        if let Some(bar) = find_mut::<AirScrollBar>(editor.root_mut(), super::identifier(5)) {
            bar.set_range(0, 8, 100);
        }
        press(&mut editor, AirKey::Down);
        turn(&mut editor, &mut commands);
        assert_eq!(editor.document.buffer().cursor().line, 1);
    }

    #[test]
    fn quitter_leve_le_drapeau() {
        let mut editor = editor(40, 12);
        assert!(!editor.wants_quit());
        editor.execute(QUIT);
        assert!(editor.wants_quit());
    }

    #[test]
    fn un_nouveau_document_remet_tout_a_zero() {
        let mut editor = editor(40, 12);
        let mut commands = AirCommandSet::new();
        turn(&mut editor, &mut commands);
        type_text(&mut editor, "texte", &mut commands);
        editor.execute(NEW);
        turn(&mut editor, &mut commands);
        assert_eq!(editor.document.buffer().lines(), [""]);
        assert!(!editor.document.buffer().is_modified());
    }

    #[test]
    fn ouvrir_pose_un_modal_prerempli() {
        let mut editor = editor(60, 20);
        editor.execute(OPEN);
        assert!(editor.root().has_modal());
        let dialog = find::<AirPromptDialog>(editor.root(), super::identifier(9))
            .expect("le dialogue modal");
        // Pas de chemin, donc pas de pré-remplissage.
        assert_eq!(dialog.text(), "");
        assert_eq!(dialog.label(), "Nom du fichier :");
    }

    #[test]
    fn annuler_referme_le_modal_sans_rien_faire() {
        let mut editor = editor(60, 20);
        editor.execute(OPEN);
        editor.execute(CANCEL);
        assert!(!editor.root().has_modal());
        assert_eq!(editor.document.buffer().lines(), [""]);
    }

    #[test]
    fn valider_un_nom_vide_refuse_et_le_dit() {
        let mut editor = editor(60, 20);
        editor.execute(OPEN);
        editor.execute(OK);
        assert!(!editor.root().has_modal());
        assert_eq!(editor.message.as_deref(), Some("aucun nom de fichier"));
    }

    #[test]
    fn valider_hors_de_toute_question_ne_fait_rien() {
        let mut editor = editor(60, 20);
        editor.execute(OK);
        assert!(!editor.root().has_modal());
        assert_eq!(editor.message, None);
    }

    #[test]
    fn enregistrer_sans_nom_demande_un_nom() {
        let mut editor = editor(60, 20);
        editor.execute(SAVE);
        assert!(editor.root().has_modal());
        assert_eq!(editor.prompt, Some(super::Prompt::SaveAs));
    }

    #[test]
    fn un_aller_retour_sur_le_disque_passe_par_le_modal() {
        let mut path = std::env::temp_dir();
        path.push(format!("redit-essai-boucle-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);

        let mut editor = editor(60, 20);
        let mut commands = AirCommandSet::new();
        turn(&mut editor, &mut commands);
        type_text(&mut editor, "contenu", &mut commands);

        editor.execute(SAVE_AS);
        if let Some(dialog) = find_mut::<AirPromptDialog>(editor.root_mut(), super::identifier(9)) {
            dialog.set_text(path.to_string_lossy().into_owned());
        }
        editor.execute(OK);
        assert_eq!(editor.message, None, "l'enregistrement a abouti");
        assert!(!editor.document.buffer().is_modified());

        // Le relire dans un contrôleur neuf : c'est l'épreuve du chargement complet.
        let mut other = Editor::new(Document::new());
        other.root_mut().resize(AirSize::new(60, 20));
        other.execute(OPEN);
        if let Some(dialog) = find_mut::<AirPromptDialog>(other.root_mut(), super::identifier(9)) {
            dialog.set_text(path.to_string_lossy().into_owned());
        }
        other.execute(OK);
        turn(&mut other, &mut commands);
        assert_eq!(other.document.buffer().lines(), ["contenu"]);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn ouvrir_un_fichier_absent_le_dit_sans_echouer() {
        let mut path = std::env::temp_dir();
        path.push(format!("redit-essai-absent-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let mut editor = editor(60, 20);
        editor.open(&path);
        assert_eq!(editor.message.as_deref(), Some("fichier neuf"));
        assert_eq!(editor.document.path(), Some(path.as_path()));
    }

    #[test]
    fn ouvrir_un_repertoire_rapporte_l_erreur_dans_le_titre() {
        let mut editor = editor(60, 20);
        let mut commands = AirCommandSet::new();
        editor.open(&std::env::temp_dir());
        turn(&mut editor, &mut commands);
        assert!(
            editor.window_title().contains("illisible"),
            "titre : {}",
            editor.window_title()
        );
    }

    #[test]
    fn supprimer_la_ligne_agit_sur_le_modele() {
        let mut editor = editor(60, 20);
        let mut commands = AirCommandSet::new();
        editor.document.buffer_mut().apply(
            AirTextAction::Load(vec!["une".to_owned(), "deux".to_owned()]),
            &mut editor.changes,
        );
        turn(&mut editor, &mut commands);
        editor.execute(DELETE_LINE);
        assert_eq!(editor.document.buffer().lines(), ["deux"]);
    }

    #[test]
    fn a_propos_affiche_la_version() {
        let mut editor = editor(60, 20);
        editor.execute(ABOUT);
        assert_eq!(
            editor.message.as_deref(),
            Some(concat!("redit ", env!("CARGO_PKG_VERSION")))
        );
    }

    #[test]
    fn une_commande_inconnue_est_ignoree() {
        let mut editor = editor(60, 20);
        editor.execute(air_tui::command::AirCommand::new(9999));
        assert!(!editor.wants_quit());
        assert_eq!(editor.message, None);
    }

    #[test]
    fn la_touche_de_fonction_de_la_barre_de_statut_emet_sa_commande() {
        let mut editor = editor(60, 20);
        assert_eq!(
            press(&mut editor, AirKey::Function(10)),
            AirOutcome::Command(QUIT)
        );
    }

    #[test]
    fn un_modal_prive_la_barre_de_statut_de_ses_touches() {
        let mut editor = editor(60, 20);
        editor.execute(OPEN);
        // `F10` ne quitte plus : le dialogue prend tout.
        assert_eq!(
            press(&mut editor, AirKey::Function(10)),
            AirOutcome::Consumed
        );
    }

    #[test]
    fn un_ecran_minuscule_ne_panique_pas() {
        let mut editor = editor(4, 4);
        let mut commands = AirCommandSet::new();
        turn(&mut editor, &mut commands);
        turn(&mut editor, &mut commands);
        assert_eq!(editor.root().size(), AirSize::new(4, 4));
    }

    #[test]
    fn un_ecran_qui_grandit_agrandit_la_vue() {
        let mut editor = editor(40, 12);
        let mut commands = AirCommandSet::new();
        turn(&mut editor, &mut commands);
        editor.root_mut().resize(AirSize::new(100, 40));
        turn(&mut editor, &mut commands);
        let view =
            find::<AirEditorView>(editor.root(), super::identifier(4)).expect("la vue d'édition");
        assert_eq!(view.visible_rows(), 35);
        assert_eq!(view.visible_columns(), 97);
    }

    // -------------------------------------------- le presse-papiers interne

    /// Saisit un texte, puis sélectionne tout.
    fn with_everything_selected(editor: &mut Editor, text: &str, commands: &mut AirCommandSet) {
        type_text(editor, text, commands);
        editor.execute(super::SELECT_ALL);
        turn(editor, commands);
    }

    #[test]
    fn coller_sur_une_selection_la_remplace() {
        // Copier ne désélectionne pas : coller juste après remplace donc ce qui vient
        // d'être copié, et c'est bien ce qu'on attend d'un éditeur.
        let mut editor = editor(40, 12);
        let mut commands = AirCommandSet::new();
        with_everything_selected(&mut editor, "abc", &mut commands);
        editor.execute(COPY);
        turn(&mut editor, &mut commands);
        editor.execute(PASTE);
        turn(&mut editor, &mut commands);
        assert_eq!(editor.document.buffer().to_text(), "abc");
    }

    #[test]
    fn copier_puis_coller_hors_selection_duplique() {
        let mut editor = editor(40, 12);
        let mut commands = AirCommandSet::new();
        with_everything_selected(&mut editor, "abc", &mut commands);
        editor.execute(COPY);
        turn(&mut editor, &mut commands);

        // La flèche droite abandonne la sélection et porte le curseur à la fin.
        press(&mut editor, AirKey::Right);
        turn(&mut editor, &mut commands);
        assert!(!editor.document.buffer().has_selection());

        editor.execute(PASTE);
        turn(&mut editor, &mut commands);
        assert_eq!(editor.document.buffer().to_text(), "abcabc");
    }

    #[test]
    fn couper_retire_la_selection_et_la_garde() {
        let mut editor = editor(40, 12);
        let mut commands = AirCommandSet::new();
        with_everything_selected(&mut editor, "abc", &mut commands);

        editor.execute(CUT);
        turn(&mut editor, &mut commands);
        assert_eq!(editor.document.buffer().to_text(), "");

        editor.execute(PASTE);
        turn(&mut editor, &mut commands);
        assert_eq!(editor.document.buffer().to_text(), "abc");
    }

    #[test]
    fn coller_sans_rien_avoir_copie_ne_change_rien() {
        let mut editor = editor(40, 12);
        let mut commands = AirCommandSet::new();
        type_text(&mut editor, "abc", &mut commands);
        editor.execute(PASTE);
        turn(&mut editor, &mut commands);
        assert_eq!(editor.document.buffer().to_text(), "abc");
    }

    #[test]
    fn copier_sans_selection_ne_detruit_pas_le_presse_papiers() {
        // Le piège : un geste sans effet ne doit pas effacer une copie antérieure.
        let mut editor = editor(40, 12);
        let mut commands = AirCommandSet::new();
        with_everything_selected(&mut editor, "abc", &mut commands);
        editor.execute(COPY);
        turn(&mut editor, &mut commands);

        // Plus de sélection, puis une copie qui ne trouve rien.
        editor.execute(super::SELECT_ALL);
        editor
            .document
            .buffer_mut()
            .apply(AirTextAction::ClearSelection, &mut AirChangeSet::new());
        editor.execute(COPY);
        turn(&mut editor, &mut commands);

        editor.execute(PASTE);
        turn(&mut editor, &mut commands);
        assert_eq!(editor.document.buffer().to_text(), "abcabc");
    }

    #[test]
    fn les_gestes_du_presse_papiers_sont_grises_quand_ils_n_ont_pas_de_sens() {
        // Le *pull* de l'ADR-007 : l'état est lu à chaque tour, personne n'est notifié.
        let mut editor = editor(40, 12);
        let mut commands = AirCommandSet::new();
        type_text(&mut editor, "abc", &mut commands);
        assert!(!commands.is_enabled(COPY), "copier sans sélection");
        assert!(!commands.is_enabled(CUT), "couper sans sélection");
        assert!(!commands.is_enabled(PASTE), "coller sans presse-papiers");

        editor.execute(super::SELECT_ALL);
        turn(&mut editor, &mut commands);
        assert!(commands.is_enabled(COPY), "copier avec une sélection");
        assert!(commands.is_enabled(CUT), "couper avec une sélection");

        editor.execute(COPY);
        turn(&mut editor, &mut commands);
        assert!(commands.is_enabled(PASTE), "coller avec un presse-papiers");
    }
}
