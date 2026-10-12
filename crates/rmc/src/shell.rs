// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Le contrôleur de `rmc` — deux panneaux, et ce qui les accorde.
//!
//! # Les quatre étapes, deux fois
//!
//! Le tour de boucle de l'ADR-007 est celui de `redit`, appliqué à deux modèles au lieu
//! d'un :
//!
//! ```text
//! 1. entrée → intention   les listes accumulent des AirListRequest
//! 2. intention → mutation chaque Directory applique les siennes
//! 3. faits → réaction     les générations décident quelle liste est repoussée
//! 4. rendu différentiel
//! ```
//!
//! Les deux panneaux ne se parlent pas. Ils ne partagent que l'écran et le focus, et
//! c'est ce qui permet d'en ajouter un troisième sans rien changer ailleurs.
//!
//! # Ce que la coquille prouve, et ce qu'elle ne prouve pas
//!
//! Elle prouve qu'`AirScreen` porte un chrome **commutable** : la même boucle, le même
//! bureau et les mêmes barres servent un éditeur à une fenêtre et un gestionnaire à deux
//! panneaux, et aucune des deux applications ne monte sa propre disposition.
//!
//! Elle ne prouve pas que `rmc` est Midnight Commander : il n'y a ni ligne de commande,
//! ni copie, ni déplacement, ni visualiseur, ni éditeur intégré. **`rmc` n'exécute aucun
//! programme** — `Entrée` sur un fichier ne fait rien, et c'est écrit dans le mode
//! d'emploi plutôt que remplacé par une devinette sur l'éditeur de l'utilisateur.

use std::path::{Path, PathBuf};

use air_tui::application::{AirDesktop, AirScreen};
use air_tui::command::{AirCommand, AirCommandSet, QUIT};
use air_tui::geometry::{AirRectangle, AirSize};
use air_tui::input::event::AirKey;
use air_tui::model::{AirChangeSet, AirGeneration, AirModel};
use air_tui::view::{AirView, AirViewId, AirViewState, find_mut};
use air_tui::widgets::{
    AirListBox, AirListItem, AirListRequest, AirMenu, AirMenuBar, AirMenuItem, AirOrientation,
    AirScrollBar, AirStatusItem, AirStatusLine, AirWindow,
};

use crate::directory::{Directory, DirectoryAction, DirectoryChange, EntryKind};

/// Changer de panneau.
pub const SWITCH_PANEL: AirCommand = AirCommand::new(300);
/// Relire le panneau actif.
pub const RELOAD: AirCommand = AirCommand::new(301);
/// Remonter d'un niveau dans le panneau actif.
pub const GO_TO_PARENT: AirCommand = AirCommand::new(302);
/// Changer le format d'affichage du panneau actif.
pub const SWITCH_LISTING_FORMAT: AirCommand = AirCommand::new(303);

/// Le nombre de colonnes d'un panneau en affichage bref.
///
/// `mc` en accepte de une à neuf — c'est son manuel qui le dit —, et son dialogue de
/// format les demande. `rmc` n'a pas ce dialogue : deux colonnes dans un panneau qui fait
/// la moitié de l'écran, c'est ce qui se lit sans tronquer la plupart des noms.
const BRIEF_COLUMNS: usize = 2;

/// Comment un panneau affiche ses entrées.
///
/// `mc` cycle entre **quatre** formats — complet, bref, long, défini par l'utilisateur.
/// `rmc` n'en a que deux, et les deux autres ne sont pas des variantes à ajouter ici :
/// « long » affiche droits, propriétaire et date, qu'`rmc` ne lit pas, et « utilisateur »
/// demande un langage de format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListingFormat {
    /// Le nom, et sa taille contre le bord droit. Une colonne.
    Full,
    /// Le nom seul, sur plusieurs colonnes.
    ///
    /// Les tailles disparaissent parce que la colonne est trop étroite pour les deux, et
    /// c'est ce que `mc` fait aussi : « the brief view shows only the file name ».
    Brief,
}

impl ListingFormat {
    /// Le suivant dans le cycle.
    const fn next(self) -> Self {
        match self {
            Self::Full => Self::Brief,
            Self::Brief => Self::Full,
        }
    }

    /// Le nombre de colonnes que ce format demande.
    const fn column_count(self) -> usize {
        match self {
            Self::Full => 1,
            Self::Brief => BRIEF_COLUMNS,
        }
    }
}

/// La dimension sous laquelle deux panneaux ne tiennent plus.
///
/// Deux fenêtres bordées de quatre colonnes utiles chacune, et trois lignes : en dessous,
/// la disposition garde ses limites plutôt que de produire des rectangles vides.
const MINIMUM_DESKTOP: AirSize = AirSize::new(16, 3);

/// Lequel des deux panneaux.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// Celui de gauche.
    Left,
    /// Celui de droite.
    Right,
}

impl Side {
    /// L'autre.
    #[must_use]
    pub const fn other(self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }

    /// L'indice du côté, pour les tableaux de deux.
    const fn index(self) -> usize {
        match self {
            Self::Left => 0,
            Self::Right => 1,
        }
    }
}

/// Les identifiants d'un panneau.
#[derive(Debug, Clone, Copy)]
struct PanelViews {
    /// La fenêtre bordée, dont le titre porte le chemin.
    window: AirViewId,
    /// La liste des entrées.
    list: AirViewId,
    /// L'ascenseur, à droite de la liste.
    bar: AirViewId,
}

/// Le contrôleur de `rmc`.
pub struct Shell {
    /// Les deux modèles, gauche puis droite.
    panels: [Directory; 2],
    /// Le panneau qui reçoit les touches.
    active: Side,
    /// Le journal des changements, réutilisé d'un tour à l'autre.
    changes: AirChangeSet<DirectoryChange>,
    /// Les demandes relevées d'une liste, réutilisées d'un tour à l'autre.
    requests: Vec<AirListRequest>,
    /// L'arbre de vues.
    root: AirScreen,
    /// Les identifiants des deux panneaux.
    views: [PanelViews; 2],
    /// Le format d'affichage de chaque panneau, gauche puis droite.
    listing: [ListingFormat; 2],
    /// La génération que chaque liste reflète déjà.
    ///
    /// C'est le mécanisme 2 de l'ADR-007 : une liste dont le modèle n'a pas changé n'est
    /// pas repoussée, et le coût ordinaire de l'étape 3 reste une comparaison d'entiers.
    reflected: [Option<AirGeneration>; 2],
    /// Le nom sur lequel poser le curseur dès que la liste aura le nouveau contenu.
    ///
    /// C'est une **intention**, pas une action : au moment où une commande la produit, la
    /// liste reflète encore le répertoire précédent, et y chercher un nom du suivant ne
    /// trouverait rien. L'étape 3 l'applique après avoir poussé le reflet.
    requested_focus: [Option<String>; 2],
    /// Vrai quand l'utilisateur a demandé à quitter.
    quit: bool,
}

/// Construit un identifiant, avec repli sur le premier valide.
///
/// Les identifiants de `rmc` sont des constantes de un à huit : le repli est inatteignable,
/// et il vaut mieux qu'un `expect` sur une constante.
fn identifier(value: u32) -> AirViewId {
    AirViewId::new(value).unwrap_or(AirViewId::FIRST)
}

impl Shell {
    /// Un contrôleur sur deux répertoires, avec son arbre monté.
    ///
    /// Rien n'est lu du disque ici : [`Self::reload_all`] le fait, et c'est elle qui
    /// rapporte les échecs une fois qu'il y a un écran pour les montrer.
    #[must_use]
    pub fn new(left: PathBuf, right: PathBuf) -> Self {
        let views = [
            PanelViews {
                window: identifier(3),
                list: identifier(4),
                bar: identifier(5),
            },
            PanelViews {
                window: identifier(6),
                list: identifier(7),
                bar: identifier(8),
            },
        ];
        let mut root = AirScreen::new(AirViewState::new(
            identifier(1),
            AirRectangle::from_parts(0, 0, 1, 1),
        ));
        root.set_desktop(Box::new(build_desktop(&views)));
        root.set_status_line(Box::new(build_status_line()));
        root.set_menu_bar(Box::new(build_menu_bar()));
        Self {
            panels: [Directory::new(left), Directory::new(right)],
            active: Side::Left,
            listing: [ListingFormat::Full, ListingFormat::Full],
            changes: AirChangeSet::with_capacity(8),
            requests: Vec::with_capacity(8),
            root,
            views,
            reflected: [None, None],
            requested_focus: [None, None],
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

    /// Un panneau, par son côté.
    #[must_use]
    pub fn panel(&self, side: Side) -> &Directory {
        // `index` ne rend que 0 ou 1, et le tableau en porte deux : le repli est
        // inatteignable, et il évite un `expect` dans un chemin appelé à chaque tour.
        self.panels.get(side.index()).unwrap_or(&self.panels[0])
    }

    /// Le format d'affichage d'un panneau.
    #[must_use]
    pub fn listing_format(&self, side: Side) -> ListingFormat {
        self.listing
            .get(side.index())
            .copied()
            .unwrap_or(ListingFormat::Full)
    }

    /// Lit les deux répertoires.
    pub fn reload_all(&mut self) {
        for side in [Side::Left, Side::Right] {
            self.apply(side, DirectoryAction::Reload);
        }
    }

    /// Applique une action à un panneau.
    fn apply(&mut self, side: Side, action: DirectoryAction) {
        let changes = &mut self.changes;
        if let Some(panel) = self.panels.get_mut(side.index()) {
            panel.apply(action, changes);
        }
    }

    /// Étape 3 : pousse le reflet des modèles dans les listes, et met le chrome à jour.
    pub fn synchronize(&mut self, commands: &mut AirCommandSet) {
        self.layout();
        self.focus_active();
        for side in [Side::Left, Side::Right] {
            self.synchronize_panel(side);
        }

        // Le *pull* de l'ADR-007 : remonter n'a pas de sens à la racine. Griser la
        // commande grise d'un coup l'item de menu et, s'il y en avait un, l'item de barre
        // de statut — sans que personne soit notifié.
        if self.panel(self.active).path().parent().is_some() {
            commands.enable(GO_TO_PARENT);
        } else {
            commands.disable(GO_TO_PARENT);
        }
    }

    /// Pousse un panneau dans sa liste, et renseigne son ascenseur.
    fn synchronize_panel(&mut self, side: Side) {
        let Some(views) = self.views.get(side.index()).copied() else {
            return;
        };
        let generation = self.panel(side).generation();
        let fresh = self
            .reflected
            .get(side.index())
            .copied()
            .flatten()
            .is_none_or(|seen| seen != generation);
        if fresh {
            let format = self.listing_format(side);
            let items: Vec<AirListItem> = self
                .panel(side)
                .entries()
                .iter()
                .map(|entry| {
                    let item = AirListItem::new(entry.name.clone());
                    // En affichage bref, la précision est **absente** et non tronquée :
                    // une colonne de dix-huit cellules ne porte pas un nom et une taille,
                    // et c'est le nom qui compte.
                    let item = match format {
                        ListingFormat::Full => item.with_detail(detail_of(entry.kind, entry.size)),
                        ListingFormat::Brief => item,
                    };
                    if entry.marked { item.marked() } else { item }
                })
                .collect();
            if let Some(list) = find_mut::<AirListBox>(&mut self.root, views.list) {
                // Les colonnes avant le contenu : poser les items d'abord ramènerait le
                // défilement dans une grille qui n'est pas encore celle du format.
                list.set_column_count(format.column_count());
                list.set_items(items);
            }
            if let Some(slot) = self.reflected.get_mut(side.index()) {
                *slot = Some(generation);
            }
        }

        let (top_item, visible_items, item_count) =
            match find_mut::<AirListBox>(&mut self.root, views.list) {
                Some(list) => (list.top_item(), list.visible_items(), list.item_count()),
                None => return,
            };
        if let Some(bar) = find_mut::<AirScrollBar>(&mut self.root, views.bar) {
            // La **surface** et non la hauteur : un panneau bref montre deux fois plus
            // d'entrées que de lignes, et un ascenseur qui l'ignorerait annoncerait un
            // reste à parcourir qui n'existe pas.
            bar.set_range(top_item, visible_items, item_count);
        }
        let title = self.panel_title(side);
        if let Some(window) = find_mut::<AirWindow>(&mut self.root, views.window) {
            window.set_title(title);
        }

        // Le curseur demandé se pose maintenant : la liste porte le bon contenu, donc le
        // nom s'y trouve — ou ne s'y trouve plus, et le curseur reste où il est.
        let wanted = self
            .requested_focus
            .get_mut(side.index())
            .and_then(Option::take);
        if let Some(name) = wanted
            && let Some(list) = find_mut::<AirListBox>(&mut self.root, views.list)
            && let Some(index) = list.items().iter().position(|item| item.text == name)
        {
            list.focus_item(index);
        }
    }

    /// Le titre d'un panneau : son chemin, et ce qui y est marqué.
    fn panel_title(&self, side: Side) -> String {
        let panel = self.panel(side);
        let path = panel.path().display().to_string();
        if let Some(message) = panel.error() {
            // Un répertoire illisible garde ce qu'on y voyait : le titre est le seul
            // endroit de l'écran qui parle de ce panneau, donc c'est là que la raison va.
            return format!("{path} — {message}");
        }
        let marked = panel.marked_count();
        if marked == 0 {
            return path;
        }
        format!(
            "{path} [{marked} marqué{} — {} o]",
            if marked > 1 { "s" } else { "" },
            panel.marked_size()
        )
    }

    /// Étape 2 : applique aux modèles ce que les listes ont demandé.
    pub fn apply_pending(&mut self) {
        self.changes.clear();
        for side in [Side::Left, Side::Right] {
            let Some(views) = self.views.get(side.index()).copied() else {
                continue;
            };
            // Le `Vec` est emprunté au champ puis rendu : appliquer les demandes en le
            // parcourant emprunterait `self` deux fois, et c'est exactement ce que
            // l'ADR-007 évite en faisant passer les intentions par une file. Le prêt
            // garde l'allocation d'un tour à l'autre.
            let mut requests = std::mem::take(&mut self.requests);
            requests.clear();
            if let Some(list) = find_mut::<AirListBox>(&mut self.root, views.list) {
                requests.extend(list.drain_requests());
            }
            for request in requests.drain(..) {
                match request {
                    AirListRequest::Activate(index) => {
                        self.apply(side, DirectoryAction::Activate(index));
                    }
                    AirListRequest::ToggleMark(index) => {
                        self.apply(side, DirectoryAction::ToggleMark(index));
                    }
                }
            }
            self.requests = requests;

            // Descendre dans un répertoire remet le curseur en tête : c'est ce que fait
            // `mc`, et garder la position d'un autre répertoire désignerait un fichier au
            // hasard.
            if self.changes.contains(&DirectoryChange::Entered)
                && let Some(list) = find_mut::<AirListBox>(&mut self.root, views.list)
            {
                list.focus_item(0);
                list.scroll_to(0);
            }

            // L'ascenseur **demande** une position ; c'est ici qu'on décide de la
            // satisfaire, parce que seul le propriétaire du contenu connaît la borne.
            let requested = find_mut::<AirScrollBar>(&mut self.root, views.bar)
                .and_then(AirScrollBar::take_request);
            if let Some(position) = requested
                && let Some(list) = find_mut::<AirListBox>(&mut self.root, views.list)
            {
                list.scroll_to(position);
            }
        }
    }

    /// Exécute une commande rendue par l'application.
    pub fn execute(&mut self, command: AirCommand) {
        match command {
            QUIT => self.quit = true,
            SWITCH_PANEL => self.active = self.active.other(),
            RELOAD => self.apply(self.active, DirectoryAction::Reload),
            SWITCH_LISTING_FORMAT => {
                let index = self.active.index();
                if let Some(slot) = self.listing.get_mut(index) {
                    *slot = slot.next();
                }
                // Le format est une décision de **vue**, et le modèle n'a pas changé :
                // sa génération est la même, donc l'étape 3 ne repousserait rien. C'est
                // au contrôleur d'oublier ce qu'il croit avoir reflété.
                if let Some(slot) = self.reflected.get_mut(index) {
                    *slot = None;
                }
            }
            GO_TO_PARENT => {
                let name = crate::directory::display_name(self.panel(self.active).path());
                self.apply(self.active, DirectoryAction::GoToParent);
                // Remonter pose le curseur sur le répertoire d'où l'on vient : c'est ce
                // que fait `mc`, et c'est ce qui rend un aller-retour sans surprise.
                if let Some(slot) = self.requested_focus.get_mut(self.active.index()) {
                    *slot = Some(name);
                }
            }
            _ => {}
        }
    }

    /// Donne le focus au panneau actif, et le retire à l'autre.
    ///
    /// Les deux chaînes comptent : la fenêtre pour que le cadre actif se voie, la liste
    /// pour qu'elle reçoive les touches **et** qu'elle montre son curseur. Une liste
    /// inactive qui garderait son focus afficherait deux curseurs à l'écran, et on ne
    /// saurait plus lequel des deux panneaux répond.
    fn focus_active(&mut self) {
        let active = self.active;
        for side in [Side::Left, Side::Right] {
            let Some(views) = self.views.get(side.index()).copied() else {
                continue;
            };
            let wanted = side == active;
            if let Some(window) = find_mut::<AirWindow>(&mut self.root, views.window) {
                window.state_mut().set_focused(wanted);
            }
            if let Some(list) = find_mut::<AirListBox>(&mut self.root, views.list) {
                list.state_mut().set_focused(wanted);
            }
        }
    }

    /// Place les deux fenêtres sur le bureau, et leurs contrôles dans les fenêtres.
    ///
    /// Recalculée à chaque tour plutôt qu'au redimensionnement : c'est une poignée
    /// d'opérations entières, et un état « est-ce que la dimension a changé » de plus
    /// serait un état de plus à tenir juste.
    fn layout(&mut self) {
        let Some(area) = self
            .root
            .desktop()
            .map(|desktop| desktop.state().bounds().size)
        else {
            return;
        };
        if area.columns < MINIMUM_DESKTOP.columns || area.rows < MINIMUM_DESKTOP.rows {
            // Trop petit pour deux panneaux. Les vues gardent leurs limites : mieux vaut
            // un contenu découpé que des rectangles ramenés à zéro à chaque tour.
            return;
        }
        let left_columns = area.columns / 2;
        let right_columns = area.columns.saturating_sub(left_columns);
        let geometry = [
            AirRectangle::from_parts(0, 0, left_columns, area.rows),
            AirRectangle::from_parts(left_columns, 0, right_columns, area.rows),
        ];
        for side in [Side::Left, Side::Right] {
            let Some(views) = self.views.get(side.index()).copied() else {
                continue;
            };
            let Some(bounds) = geometry.get(side.index()).copied() else {
                continue;
            };
            let Some(window) = find_mut::<AirWindow>(&mut self.root, views.window) else {
                continue;
            };
            window.state_mut().set_bounds(bounds);
            // La fenêtre est bordée : son contenu perd une cellule de chaque côté, et
            // l'ascenseur prend la dernière colonne de ce qui reste.
            let content = AirSize::new(
                bounds.size.columns.saturating_sub(2),
                bounds.size.rows.saturating_sub(2),
            );
            let list_columns = content.columns.saturating_sub(1);
            let group = window.group_mut();
            if let Some(view) = group.child_mut(views.list) {
                view.state_mut().set_bounds(AirRectangle::from_parts(
                    0,
                    0,
                    list_columns,
                    content.rows,
                ));
            }
            if let Some(view) = group.child_mut(views.bar) {
                view.state_mut().set_bounds(AirRectangle::from_parts(
                    list_columns,
                    0,
                    1,
                    content.rows,
                ));
            }
        }
    }
}

/// La précision écrite à droite d'une entrée.
///
/// `mc` écrit « UP--DIR » et « SUB-DIR » là où le français n'a pas de mot de sept lettres :
/// les abréviations sont les siennes, traduites, et elles tiennent dans la largeur d'un
/// panneau étroit.
fn detail_of(kind: EntryKind, size: u64) -> String {
    match kind {
        EntryKind::Parent => "HAUT".to_owned(),
        EntryKind::Directory => "RÉP".to_owned(),
        EntryKind::Symlink => "LIEN".to_owned(),
        EntryKind::File => size.to_string(),
    }
}

/// Le bureau, avec ses deux panneaux.
fn build_desktop(views: &[PanelViews; 2]) -> AirDesktop {
    let mut desktop = AirDesktop::new(AirViewState::new(
        identifier(2),
        AirRectangle::from_parts(0, 0, 1, 1),
    ));
    for panel in views {
        let mut window = AirWindow::new(AirViewState::new(
            panel.window,
            AirRectangle::from_parts(0, 0, 1, 1),
        ));
        let mut list = AirListBox::new(AirViewState::new(
            panel.list,
            AirRectangle::from_parts(0, 0, 1, 1),
        ));
        // Le focus est posé ici pour que l'arbre soit cohérent avant le premier tour ;
        // `focus_active` le reprend ensuite à chaque tour.
        list.state_mut().set_focused(true);
        window.insert(Box::new(list));
        window.insert(Box::new(AirScrollBar::new(
            AirViewState::new(panel.bar, AirRectangle::from_parts(0, 0, 1, 1)),
            AirOrientation::Vertical,
        )));
        desktop.insert(Box::new(window));
    }
    desktop
}

/// La barre de menu de `rmc`.
fn build_menu_bar() -> AirMenuBar {
    AirMenuBar::new(AirViewState::new(
        identifier(9),
        AirRectangle::from_parts(0, 0, 1, 1),
    ))
    .with_menu(
        AirMenu::new("~F~ichier")
            .with_item(AirMenuItem::new("~R~elire", RELOAD))
            .with_item(AirMenuItem::separator())
            .with_item(AirMenuItem::new("~Q~uitter", QUIT)),
    )
    .with_menu(
        AirMenu::new("~P~anneau")
            .with_item(AirMenuItem::new("~C~hanger de panneau", SWITCH_PANEL))
            .with_item(AirMenuItem::new("Re~m~onter", GO_TO_PARENT))
            .with_item(AirMenuItem::new(
                "~F~ormat d'affichage",
                SWITCH_LISTING_FORMAT,
            )),
    )
}

/// La barre de touches de `rmc`.
///
/// Elle ne publie que ce qui fonctionne. `mc` en affiche dix ; en annoncer dix dont sept
/// ne font rien serait exactement le défaut que le NO GO n° 2 du dépôt interdit.
fn build_status_line() -> AirStatusLine {
    AirStatusLine::new(AirViewState::new(
        identifier(10),
        AirRectangle::from_parts(0, 0, 1, 1),
    ))
    .with_item(AirStatusItem::new(
        "~Tab~ Panneau",
        AirKey::Tab,
        SWITCH_PANEL,
    ))
    .with_item(AirStatusItem::new(
        "~F5~ Relire",
        AirKey::Function(5),
        RELOAD,
    ))
    .with_item(AirStatusItem::new(
        "~F10~ Quitter",
        AirKey::Function(10),
        QUIT,
    ))
}

/// Le répertoire de départ d'un panneau, quand la ligne de commande n'en donne pas.
///
/// Le répertoire courant, ou la racine quand il n'est pas lisible — un processus dont le
/// répertoire courant a été supprimé n'a pas de `getcwd`, et l'écran doit quand même
/// s'ouvrir sur quelque chose.
#[must_use]
pub fn default_directory() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| Path::new("/").to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::{
        BRIEF_COLUMNS, GO_TO_PARENT, ListingFormat, RELOAD, SWITCH_LISTING_FORMAT, SWITCH_PANEL,
        Shell, Side, build_menu_bar, detail_of, identifier,
    };
    use crate::directory::{EntryKind, scratch::Scratch};
    use air_tui::command::{AirCommandSet, QUIT};
    use air_tui::geometry::AirSize;
    use air_tui::input::event::{AirInputEvent, AirKey, AirKeyEvent};
    use air_tui::view::{AirContext, AirEvent, AirOutcome, AirView, dispatch_event, find};
    use air_tui::widgets::{AirListBox, AirScrollBar, AirWindow};
    use std::path::Path;

    /// Un contrôleur sur deux chemins, à l'écran donné, après un premier tour.
    fn shell(left: &Path, right: &Path, columns: u16, rows: u16) -> (Shell, AirCommandSet) {
        let mut shell = Shell::new(left.to_path_buf(), right.to_path_buf());
        shell.root_mut().resize(AirSize::new(columns, rows));
        shell.reload_all();
        let mut commands = AirCommandSet::new();
        shell.synchronize(&mut commands);
        (shell, commands)
    }

    /// Un tour de boucle complet, sans terminal.
    fn turn(shell: &mut Shell, commands: &mut AirCommandSet) {
        shell.apply_pending();
        shell.synchronize(commands);
    }

    /// Envoie une frappe dans l'arbre, et exécute la commande qu'elle produit.
    ///
    /// C'est ce que fait la vraie boucle : `pump` collecte les commandes rendues par la
    /// distribution, et le contrôleur les exécute après avoir appliqué les intentions.
    fn press(shell: &mut Shell, key: AirKey) -> AirOutcome {
        let mut commands = AirCommandSet::new();
        let outcome = {
            let mut context = AirContext {
                commands: &mut commands,
            };
            let mut event = AirEvent::new(AirInputEvent::Key(AirKeyEvent::press(key)));
            dispatch_event(shell.root_mut(), &mut event, &mut context)
        };
        if let AirOutcome::Command(command) = outcome {
            shell.execute(command);
        }
        outcome
    }

    /// La liste d'un côté.
    fn list_of(shell: &Shell, side: Side) -> &AirListBox {
        let identifier = if side == Side::Left {
            identifier(4)
        } else {
            identifier(7)
        };
        find::<AirListBox>(shell.root(), identifier).expect("la liste du panneau")
    }

    #[test]
    fn un_contexte_neuf_monte_les_deux_panneaux_entiers() {
        let scratch = Scratch::new(&["un"], &[("a.txt", "")]);
        let (shell, _) = shell(scratch.path(), scratch.path(), 80, 25);
        let root = shell.root();
        assert_eq!(root.size(), AirSize::new(80, 25));
        // Les six vues que le contrôleur retrouve à chaque tour sont là, avec leur type.
        for value in [3, 6] {
            assert!(find::<AirWindow>(root, identifier(value)).is_some());
        }
        for value in [4, 7] {
            assert!(find::<AirListBox>(root, identifier(value)).is_some());
        }
        for value in [5, 8] {
            assert!(find::<AirScrollBar>(root, identifier(value)).is_some());
        }
    }

    #[test]
    fn les_deux_panneaux_partagent_la_largeur_et_reservent_leur_ascenseur() {
        let scratch = Scratch::new(&[], &[]);
        let (shell, _) = shell(scratch.path(), scratch.path(), 80, 25);
        let left = find::<AirWindow>(shell.root(), identifier(3)).expect("fenêtre gauche");
        let right = find::<AirWindow>(shell.root(), identifier(6)).expect("fenêtre droite");
        assert_eq!(left.state().bounds().size.columns, 40);
        assert_eq!(right.state().bounds().size.columns, 40);
        assert_eq!(right.state().bounds().origin.column.value(), 40);
        // Quatre-vingts colonnes en deux, moins deux de cadre, moins l'ascenseur : 37.
        let list = list_of(&shell, Side::Left);
        assert_eq!(list.state().bounds().size.columns, 37);
        let bar = find::<AirScrollBar>(shell.root(), identifier(5)).expect("ascenseur gauche");
        assert_eq!(bar.state().bounds().origin.column.value(), 37);
        // Vingt-cinq lignes, moins les deux barres, moins le cadre : vingt-et-une.
        assert_eq!(list.visible_rows(), 21);
    }

    #[test]
    fn une_largeur_impaire_ne_perd_pas_de_colonne() {
        let scratch = Scratch::new(&[], &[]);
        let (shell, _) = shell(scratch.path(), scratch.path(), 81, 25);
        let left = find::<AirWindow>(shell.root(), identifier(3)).expect("fenêtre gauche");
        let right = find::<AirWindow>(shell.root(), identifier(6)).expect("fenêtre droite");
        assert_eq!(left.state().bounds().size.columns, 40);
        assert_eq!(right.state().bounds().size.columns, 41);
    }

    #[test]
    fn un_ecran_trop_petit_garde_ses_limites() {
        // Mieux vaut un contenu découpé que des rectangles ramenés à zéro à chaque tour.
        let scratch = Scratch::new(&[], &[]);
        let (shell, _) = shell(scratch.path(), scratch.path(), 10, 3);
        let left = find::<AirWindow>(shell.root(), identifier(3)).expect("fenêtre gauche");
        assert_eq!(left.state().bounds().size.columns, 1);
    }

    #[test]
    fn le_panneau_de_gauche_est_actif_et_la_tabulation_echange() {
        let scratch = Scratch::new(&[], &[("a.txt", "")]);
        let (mut shell, mut commands) = shell(scratch.path(), scratch.path(), 80, 25);
        assert_eq!(shell.active, Side::Left);
        assert!(list_of(&shell, Side::Left).state().is_focused());
        assert!(!list_of(&shell, Side::Right).state().is_focused());

        press(&mut shell, AirKey::Tab);
        turn(&mut shell, &mut commands);
        assert_eq!(shell.active, Side::Right);
        assert!(!list_of(&shell, Side::Left).state().is_focused());
        assert!(list_of(&shell, Side::Right).state().is_focused());
        // Et le cadre suit : c'est ce qui se voit à l'écran.
        let right = find::<AirWindow>(shell.root(), identifier(6)).expect("fenêtre droite");
        assert!(right.state().is_focused());
    }

    #[test]
    fn les_deux_panneaux_sont_independants() {
        let scratch = Scratch::new(&["dedans"], &[]);
        let (mut shell, mut commands) = shell(scratch.path(), scratch.path(), 80, 25);
        let index = list_of(&shell, Side::Left)
            .items()
            .iter()
            .position(|item| item.text == "dedans")
            .expect("le répertoire est listé");
        if let Some(list) = air_tui::view::find_mut::<AirListBox>(shell.root_mut(), identifier(4)) {
            list.focus_item(index);
        }
        press(&mut shell, AirKey::Enter);
        turn(&mut shell, &mut commands);
        assert_eq!(
            shell.panel(Side::Left).path(),
            scratch.path().join("dedans")
        );
        assert_eq!(
            shell.panel(Side::Right).path(),
            scratch.path(),
            "le panneau de droite n'a pas bougé"
        );
    }

    #[test]
    fn descendre_remet_le_curseur_en_tete() {
        // Garder la position d'un autre répertoire désignerait un fichier au hasard.
        let scratch = Scratch::new(&["dedans"], &[("a", ""), ("b", ""), ("c", "")]);
        let (mut shell, mut commands) = shell(scratch.path(), scratch.path(), 80, 25);
        press(&mut shell, AirKey::End);
        turn(&mut shell, &mut commands);
        assert!(list_of(&shell, Side::Left).focused_item() > 0);
        // Le curseur est en bas ; on remonte sur `dedans` et on y descend.
        let index = list_of(&shell, Side::Left)
            .items()
            .iter()
            .position(|item| item.text == "dedans")
            .expect("listé");
        if let Some(list) = air_tui::view::find_mut::<AirListBox>(shell.root_mut(), identifier(4)) {
            list.focus_item(index);
        }
        press(&mut shell, AirKey::Enter);
        turn(&mut shell, &mut commands);
        assert_eq!(list_of(&shell, Side::Left).focused_item(), 0);
        assert_eq!(list_of(&shell, Side::Left).top_item(), 0);
    }

    #[test]
    fn remonter_pose_le_curseur_sur_le_repertoire_d_ou_l_on_vient() {
        let scratch = Scratch::new(&["aaa", "bbb", "ccc"], &[]);
        let inside = scratch.path().join("bbb");
        let (mut shell, mut commands) = shell(&inside, scratch.path(), 80, 25);
        shell.execute(GO_TO_PARENT);
        turn(&mut shell, &mut commands);
        assert_eq!(shell.panel(Side::Left).path(), scratch.path());
        let list = list_of(&shell, Side::Left);
        assert_eq!(
            list.focused().map(|item| item.text.as_str()),
            Some("bbb"),
            "l'aller-retour revient où il était"
        );
    }

    #[test]
    fn le_format_bref_met_le_panneau_en_colonnes_et_retire_les_tailles() {
        let scratch = Scratch::new(&["dossier"], &[("a.txt", "abc")]);
        let (mut shell, mut commands) = shell(scratch.path(), scratch.path(), 80, 25);
        let list = list_of(&shell, Side::Left);
        assert_eq!(list.column_count(), 1, "le format complet est le défaut");
        assert!(
            list.items().iter().all(|item| !item.detail.is_empty()),
            "le format complet écrit une précision pour chaque entrée"
        );

        shell.execute(SWITCH_LISTING_FORMAT);
        turn(&mut shell, &mut commands);
        assert_eq!(shell.listing_format(Side::Left), ListingFormat::Brief);
        let list = list_of(&shell, Side::Left);
        assert_eq!(list.column_count(), BRIEF_COLUMNS);
        assert!(
            list.items().iter().all(|item| item.detail.is_empty()),
            "l'affichage bref ne montre que les noms"
        );
    }

    #[test]
    fn le_format_est_propre_au_panneau_actif_et_revient() {
        let scratch = Scratch::new(&[], &[("a.txt", "abc")]);
        let (mut shell, mut commands) = shell(scratch.path(), scratch.path(), 80, 25);
        shell.execute(SWITCH_LISTING_FORMAT);
        turn(&mut shell, &mut commands);
        assert_eq!(shell.listing_format(Side::Left), ListingFormat::Brief);
        assert_eq!(
            shell.listing_format(Side::Right),
            ListingFormat::Full,
            "l'autre panneau n'a rien demandé"
        );
        assert_eq!(list_of(&shell, Side::Right).column_count(), 1);

        // Le cycle n'a que deux formats : le second appui revient au premier.
        shell.execute(SWITCH_LISTING_FORMAT);
        turn(&mut shell, &mut commands);
        assert_eq!(shell.listing_format(Side::Left), ListingFormat::Full);
        let list = list_of(&shell, Side::Left);
        assert_eq!(list.column_count(), 1);
        assert!(
            list.items().iter().all(|item| !item.detail.is_empty()),
            "les tailles reviennent avec le format complet"
        );
    }

    #[test]
    fn l_ascenseur_d_un_panneau_bref_mesure_la_surface() {
        // Assez d'entrées pour que la mesure ne soit pas ramenée au total : l'ascenseur
        // borne `visible` par `total`, et deux fichiers donneraient deux dans les deux
        // formats — un essai qui ne mesurerait rien.
        let names: Vec<String> = (0..50).map(|index| format!("fichier-{index:02}")).collect();
        let files: Vec<(&str, &str)> = names.iter().map(|name| (name.as_str(), "x")).collect();
        let scratch = Scratch::new(&[], &files);
        let (mut shell, mut commands) = shell(scratch.path(), scratch.path(), 80, 25);
        let bar = find::<AirScrollBar>(shell.root(), identifier(5)).expect("ascenseur gauche");
        assert_eq!(bar.visible(), 21, "vingt-et-une lignes en format complet");

        shell.execute(SWITCH_LISTING_FORMAT);
        turn(&mut shell, &mut commands);
        let bar = find::<AirScrollBar>(shell.root(), identifier(5)).expect("ascenseur gauche");
        // Vingt-et-une lignes, deux colonnes : quarante-deux entrées visibles d'un coup.
        assert_eq!(bar.visible(), 42);
    }

    #[test]
    fn le_menu_du_panneau_publie_le_format_d_affichage() {
        // La commande n'a pas de touche : le framework n'a pas de table d'accélérateurs,
        // et le menu est donc le seul chemin. Un item absent la rendrait inatteignable.
        let bar = build_menu_bar();
        let labels: Vec<&str> = bar
            .menus()
            .iter()
            .flat_map(|menu| {
                menu.items()
                    .iter()
                    .map(air_tui::widgets::AirMenuItem::label)
            })
            .collect();
        assert!(
            labels.contains(&"~F~ormat d'affichage"),
            "le menu ne publie pas le format : {labels:?}"
        );
    }

    #[test]
    fn remonter_est_grise_a_la_racine() {
        // Le *pull* de l'ADR-007 : une seule source, et le menu comme la barre y lisent.
        let scratch = Scratch::new(&[], &[]);
        let (_, commands) = shell(Path::new("/"), scratch.path(), 80, 25);
        assert!(!commands.is_enabled(GO_TO_PARENT));
        let (_, commands) = shell(scratch.path(), Path::new("/"), 80, 25);
        assert!(commands.is_enabled(GO_TO_PARENT));
    }

    #[test]
    fn inser_marque_et_le_titre_le_dit() {
        let scratch = Scratch::new(&[], &[("trois", "abc")]);
        let (mut shell, mut commands) = shell(scratch.path(), scratch.path(), 80, 25);
        let index = list_of(&shell, Side::Left)
            .items()
            .iter()
            .position(|item| item.text == "trois")
            .expect("listé");
        if let Some(list) = air_tui::view::find_mut::<AirListBox>(shell.root_mut(), identifier(4)) {
            list.focus_item(index);
        }
        press(&mut shell, AirKey::Insert);
        turn(&mut shell, &mut commands);
        assert_eq!(shell.panel(Side::Left).marked_count(), 1);
        let window = find::<AirWindow>(shell.root(), identifier(3)).expect("fenêtre gauche");
        let title = window.title().unwrap_or_default().to_owned();
        assert!(
            title.contains("1 marqué") && title.contains("3 o"),
            "le titre ne porte pas la marque : {title}"
        );
        // Et la liste l'affiche : le reflet est repoussé parce que la génération a bougé.
        let item = list_of(&shell, Side::Left)
            .items()
            .iter()
            .find(|item| item.text == "trois")
            .expect("listé");
        assert!(item.marked);
    }

    #[test]
    fn le_titre_porte_le_pluriel_quand_il_y_a_plusieurs_marques() {
        let scratch = Scratch::new(&[], &[("un", "a"), ("deux", "bb")]);
        let (mut shell, mut commands) = shell(scratch.path(), scratch.path(), 80, 25);
        press(&mut shell, AirKey::Insert);
        press(&mut shell, AirKey::Insert);
        press(&mut shell, AirKey::Insert);
        turn(&mut shell, &mut commands);
        assert_eq!(shell.panel(Side::Left).marked_count(), 2);
        let window = find::<AirWindow>(shell.root(), identifier(3)).expect("fenêtre gauche");
        let title = window.title().unwrap_or_default().to_owned();
        assert!(title.contains("2 marqués"), "titre : {title}");
    }

    #[test]
    fn le_titre_porte_la_raison_d_un_echec_de_lecture() {
        let scratch = Scratch::new(&[], &[]);
        let absent = scratch.path().join("absent");
        let (shell, _) = shell(&absent, scratch.path(), 80, 25);
        let window = find::<AirWindow>(shell.root(), identifier(3)).expect("fenêtre gauche");
        let title = window.title().unwrap_or_default().to_owned();
        assert!(title.contains('—'), "le titre ne dit pas l'échec : {title}");
    }

    #[test]
    fn la_liste_n_est_repoussee_que_quand_la_generation_bouge() {
        // Le mécanisme 2 de l'ADR-007 : le coût ordinaire de l'étape 3 est une
        // comparaison d'entiers, pas une copie de la liste.
        let scratch = Scratch::new(&[], &[("a", "")]);
        let (mut shell, mut commands) = shell(scratch.path(), scratch.path(), 80, 25);
        let generation = shell.panel(Side::Left).generation();
        assert_eq!(shell.reflected[0], Some(generation));
        // Un fichier créé en dehors de `rmc` n'est pas vu sans relecture : c'est voulu.
        std::fs::write(scratch.path().join("neuf"), "").expect("fichier");
        turn(&mut shell, &mut commands);
        assert_eq!(list_of(&shell, Side::Left).item_count(), 2);
        shell.execute(RELOAD);
        turn(&mut shell, &mut commands);
        assert_eq!(list_of(&shell, Side::Left).item_count(), 3);
        assert_ne!(shell.panel(Side::Left).generation(), generation);
    }

    #[test]
    fn l_ascenseur_suit_la_liste_et_la_defile_en_retour() {
        let scratch = Scratch::new(&[], &[]);
        for index in 0..60 {
            std::fs::write(scratch.path().join(format!("fichier-{index:02}")), "")
                .expect("fichier");
        }
        let (mut shell, mut commands) = shell(scratch.path(), scratch.path(), 80, 12);
        press(&mut shell, AirKey::End);
        turn(&mut shell, &mut commands);
        let list = list_of(&shell, Side::Left);
        let top = list.top_item();
        assert!(top > 0, "la fin de la liste a fait défiler");
        let bar = find::<AirScrollBar>(shell.root(), identifier(5)).expect("ascenseur");
        assert_eq!(bar.position(), top);
        assert_eq!(bar.total(), 61, "soixante fichiers et le parent");
    }

    #[test]
    fn la_touche_de_fonction_de_la_barre_relit_le_panneau() {
        let scratch = Scratch::new(&[], &[("a", "")]);
        let (mut shell, mut commands) = shell(scratch.path(), scratch.path(), 80, 25);
        std::fs::write(scratch.path().join("neuf"), "").expect("fichier");
        // `F5` est publiée par la barre de statut, qui est en post-traitement : la liste
        // ne la voit pas passer.
        press(&mut shell, AirKey::Function(5));
        turn(&mut shell, &mut commands);
        assert_eq!(list_of(&shell, Side::Left).item_count(), 3);
    }

    #[test]
    fn la_touche_de_fonction_de_sortie_leve_le_drapeau() {
        let scratch = Scratch::new(&[], &[]);
        let (mut shell, _) = shell(scratch.path(), scratch.path(), 80, 25);
        assert!(!shell.wants_quit());
        press(&mut shell, AirKey::Function(10));
        assert!(shell.wants_quit());
    }

    #[test]
    fn la_commande_de_sortie_leve_le_drapeau() {
        let scratch = Scratch::new(&[], &[]);
        let (mut shell, _) = shell(scratch.path(), scratch.path(), 80, 25);
        shell.execute(QUIT);
        assert!(shell.wants_quit());
    }

    #[test]
    fn changer_de_panneau_par_la_commande_et_revenir() {
        let scratch = Scratch::new(&[], &[]);
        let (mut shell, _) = shell(scratch.path(), scratch.path(), 80, 25);
        shell.execute(SWITCH_PANEL);
        assert_eq!(shell.active, Side::Right);
        shell.execute(SWITCH_PANEL);
        assert_eq!(shell.active, Side::Left);
        assert_eq!(Side::Left.other(), Side::Right);
        assert_eq!(Side::Right.other(), Side::Left);
    }

    #[test]
    fn la_precision_de_droite_dit_ce_qu_est_l_entree() {
        assert_eq!(detail_of(EntryKind::Parent, 0), "HAUT");
        assert_eq!(detail_of(EntryKind::Directory, 0), "RÉP");
        assert_eq!(detail_of(EntryKind::Symlink, 0), "LIEN");
        assert_eq!(detail_of(EntryKind::File, 4096), "4096");
    }

    #[test]
    fn le_repertoire_de_depart_existe() {
        // Un processus dont le répertoire courant a été supprimé n'a pas de `getcwd` :
        // l'écran doit quand même s'ouvrir sur quelque chose.
        let path = super::default_directory();
        assert!(path.is_absolute());
    }
}
