// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! L'arbre de vues — le modèle d'objets, le dessin et le routage des événements.
//!
//! # Composition, pas héritage
//!
//! Turbo Vision bâtissait une hiérarchie de classes à partir de `TView`. Ici, [`AirView`]
//! est un **trait** et [`AirViewState`] une structure que chaque vue **contient**
//! (ADR-011). Une vue n'hérite de rien : elle expose son état et dit comment se dessiner.
//!
//! # Bordure par défaut
//!
//! Toute vue a une bordure, et la retire par [`AirViewState::set_border`] (ADR-010). C'est
//! l'inverse de Turbo Vision, où le cadre était une sous-vue qu'un `TWindow` insérait, et
//! le choix se paie d'une conséquence qu'il faut connaître : **la zone de contenu n'est
//! pas les limites de la vue**. Une vue bordée perd une cellule de chaque côté, et c'est
//! [`AirPainter`] qu'elle reçoit pour son contenu qui le lui impose — elle ne peut pas
//! écrire sur son propre cadre.
//!
//! Les petits contrôles posent `border = false` dans leur constructeur : un bouton encadré
//! sur trois cellules de haut n'a plus de place pour son texte.
//!
//! # La bordure est dessinée par le thème
//!
//! Une vue ne choisit pas ses caractères de cadre, elle choisit d'en avoir un. Les glyphes
//! viennent de [`crate::theme::AirGlyphSet`] et les couleurs d'un rôle de la palette
//! (ADR-008) : changer de thème redessine tous les cadres sans toucher une vue.

pub mod group;
pub mod painter;

use core::any::Any;
use core::num::NonZeroU32;

pub use group::AirGroup;
pub use painter::AirPainter;

use crate::command::{AirCommand, AirCommandSet};
use crate::geometry::{AirColumn, AirPoint, AirRectangle, AirRow, AirSize};
use crate::input::event::{AirInputEvent, AirKeyState};
use crate::theme::{AirColorPair, AirFrameGlyphs, AirRole, AirTheme};

/// L'identifiant d'une vue.
///
/// Un entier non nul plutôt qu'un pointeur : une vue détruite ne laisse pas un
/// identifiant qui semble valide, et le modèle MVC de l'ADR-007 s'y réfère sans jamais
/// tenir de référence arrière.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AirViewId(NonZeroU32);

impl AirViewId {
    /// Le premier identifiant valide.
    ///
    /// Une application qui numérote ses vues part de là et incrémente ; c'est aussi la
    /// valeur de repli d'une suite qui saturerait, cas qu'un compteur de trente-deux bits
    /// n'atteint pas.
    pub const FIRST: Self = match Self::new(1) {
        Some(first) => first,
        None => panic!("un n'est pas nul"),
    };

    /// Construit un identifiant, ou `None` pour zéro.
    #[must_use]
    pub const fn new(value: u32) -> Option<Self> {
        match NonZeroU32::new(value) {
            Some(non_zero) => Some(Self(non_zero)),
            None => None,
        }
    }

    /// La valeur brute.
    #[must_use]
    pub const fn value(self) -> u32 {
        self.0.get()
    }
}

/// Le style de bordure d'une vue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AirBorderStyle {
    /// Trait simple — les fenêtres de Turbo Vision.
    #[default]
    Single,
    /// Trait double — ses dialogues.
    Double,
}

/// La phase de distribution d'un événement aux enfants d'une vue.
///
/// Les trois phases de Turbo Vision, conservées telles quelles : ce sont elles qui font
/// fonctionner les menus, les raccourcis et la barre de statut sans que chacun ait à
/// connaître les autres.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AirPhase {
    /// Avant la vue qui a le focus — pour qui doit voir un événement d'abord.
    PreProcess,
    /// À la vue qui a le focus.
    Focused,
    /// Après la vue qui a le focus — pour les raccourcis globaux, qui ne doivent pas
    /// voler une frappe destinée à un champ de saisie.
    PostProcess,
}

/// Ce qu'une vue a fait d'un événement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AirOutcome {
    /// La vue n'était pas concernée.
    Ignored,
    /// La vue a traité l'événement.
    Consumed,
    /// La vue demande l'exécution d'une commande.
    Command(AirCommand),
}

/// Un événement en cours de distribution.
///
/// La consommation est explicite — c'est le `clearEvent` de Turbo Vision : tant que
/// l'événement n'est pas consommé, la distribution continue.
#[derive(Debug, Clone)]
pub struct AirEvent {
    /// La charge utile, retirée dès que l'événement est consommé.
    payload: Option<AirInputEvent>,
    /// La phase en cours, lue par les vues qui s'en soucient.
    phase: AirPhase,
}

impl AirEvent {
    /// Un événement à distribuer.
    #[must_use]
    pub const fn new(input: AirInputEvent) -> Self {
        Self {
            payload: Some(input),
            phase: AirPhase::Focused,
        }
    }

    /// La charge utile, si l'événement n'a pas été consommé.
    #[must_use]
    pub const fn input(&self) -> Option<&AirInputEvent> {
        self.payload.as_ref()
    }

    /// La phase en cours.
    #[must_use]
    pub const fn phase(&self) -> AirPhase {
        self.phase
    }

    /// Déclare l'événement traité : la distribution s'arrête.
    pub fn consume(&mut self) {
        self.payload = None;
    }

    /// Vrai si l'événement a été consommé.
    #[must_use]
    pub const fn is_consumed(&self) -> bool {
        self.payload.is_none()
    }

    /// Vrai s'il s'agit d'un événement de souris, qui se route par la position et non par
    /// le focus.
    #[must_use]
    pub const fn is_mouse(&self) -> bool {
        matches!(self.payload, Some(AirInputEvent::Mouse(_)))
    }

    /// La position de l'événement, s'il en a une.
    #[must_use]
    pub const fn mouse_position(&self) -> Option<AirPoint> {
        match &self.payload {
            Some(AirInputEvent::Mouse(mouse)) => Some(mouse.position),
            _ => None,
        }
    }

    /// Déplace la position de l'événement de souris vers les coordonnées d'une zone.
    ///
    /// La traduction se fait **à la descente** de l'arbre, et se défait à la remontée :
    /// une vue reçoit toujours des coordonnées qui sont les siennes, sans avoir à savoir
    /// où elle se trouve. Déplacer plutôt que cloner évite une allocation par vue
    /// traversée, et un collage encadré porte une chaîne.
    ///
    /// Visible dans la crate : les widgets qui composent eux-mêmes — la barre de menu et
    /// son déroulant, le dialogue et ses contrôles — descendent sans passer par
    /// `children`, et doivent donc traduire comme la traversée le fait.
    pub(crate) fn shift_into(&mut self, origin: AirPoint) {
        if let Some(AirInputEvent::Mouse(mouse)) = self.payload.as_mut() {
            mouse.position = AirPoint::new(
                AirColumn::new(
                    mouse
                        .position
                        .column
                        .value()
                        .saturating_sub(origin.column.value()),
                ),
                AirRow::new(
                    mouse
                        .position
                        .row
                        .value()
                        .saturating_sub(origin.row.value()),
                ),
            );
        }
    }

    /// Défait un [`Self::shift_into`].
    pub(crate) fn shift_out_of(&mut self, origin: AirPoint) {
        if let Some(AirInputEvent::Mouse(mouse)) = self.payload.as_mut() {
            mouse.position = AirPoint::new(
                AirColumn::new(
                    mouse
                        .position
                        .column
                        .value()
                        .saturating_add(origin.column.value()),
                ),
                AirRow::new(
                    mouse
                        .position
                        .row
                        .value()
                        .saturating_add(origin.row.value()),
                ),
            );
        }
    }
}

/// Ce qu'une vue peut consulter pendant qu'elle se dessine.
///
/// Le jeu de commandes en fait partie, et ce n'est pas un détail : l'ADR-007 fonde le
/// grisage sur du *pull* — « chaque vue interroge le jeu au moment où elle se dessine ».
/// Sans le jeu dans le chemin de dessin, cette phrase serait fausse et un item de menu ne
/// pourrait pas se griser tout seul.
#[derive(Debug, Clone, Copy)]
pub struct AirDrawContext<'draw> {
    /// Le thème en vigueur.
    pub theme: &'draw AirTheme,
    /// Le jeu d'activation des commandes, en lecture seule.
    pub commands: &'draw AirCommandSet,
}

impl<'draw> AirDrawContext<'draw> {
    /// Un contexte de dessin.
    #[must_use]
    pub const fn new(theme: &'draw AirTheme, commands: &'draw AirCommandSet) -> Self {
        Self { theme, commands }
    }

    /// Vrai si la commande est active.
    ///
    /// Raccourci du geste que fait tout widget grisable, pour qu'il ne traverse pas la
    /// structure à la main.
    #[must_use]
    pub fn is_command_enabled(&self, command: AirCommand) -> bool {
        self.commands.is_enabled(command)
    }
}

/// Ce qu'une vue peut consulter et modifier pendant le traitement d'un événement.
#[derive(Debug)]
pub struct AirContext<'commands> {
    /// Le jeu d'activation des commandes.
    pub commands: &'commands mut AirCommandSet,
}

/// L'état commun à toute vue.
///
/// Contenue par composition, jamais héritée (ADR-011).
// Six booléens, et c'est la bonne forme : chacun est un état indépendant que l'appelant
// lit et écrit par son nom. Les regrouper en masque de bits rendrait `state.is_focused()`
// illisible et ferait écrire des comparaisons de bits dans chaque widget — ce que le
// Principe 7 d'Air refuse.
#[allow(
    clippy::struct_excessive_bools,
    reason = "un booléen par état indépendant"
)]
#[derive(Debug, Clone, Copy)]
pub struct AirViewState {
    /// L'identifiant de la vue.
    identifier: AirViewId,
    /// Les limites, **relatives à la zone de contenu du propriétaire**.
    bounds: AirRectangle,
    /// Vrai si la vue porte une bordure. Vrai par défaut (ADR-010).
    border: bool,
    /// Le style de la bordure.
    border_style: AirBorderStyle,
    /// Vrai si la vue est dessinée et reçoit des événements.
    visible: bool,
    /// Vrai si la vue détient le focus parmi ses sœurs.
    focused: bool,
    /// Vrai si la vue réagit aux événements.
    enabled: bool,
    /// Vrai si la vue voit les événements avant celle qui a le focus.
    pre_process: bool,
    /// Vrai si la vue voit les événements après celle qui a le focus.
    post_process: bool,
    /// Vrai si la vue projette une ombre portée.
    ///
    /// Faux par défaut : une ombre sur chaque vue couvrirait l'écran de gris. Ce n'est
    /// **pas** cette vue qui la dessine — elle tombe hors de ses limites, et son
    /// propriétaire la peint pour elle (ADR-015).
    casts_shadow: bool,
}

impl AirViewState {
    /// L'état d'une vue neuve, bordée, visible et active.
    #[must_use]
    pub const fn new(identifier: AirViewId, bounds: AirRectangle) -> Self {
        Self {
            identifier,
            bounds,
            border: true,
            border_style: AirBorderStyle::Single,
            casts_shadow: false,
            visible: true,
            focused: false,
            enabled: true,
            pre_process: false,
            post_process: false,
        }
    }

    /// L'identifiant de la vue.
    #[must_use]
    pub const fn identifier(&self) -> AirViewId {
        self.identifier
    }

    /// Les limites de la vue, relatives à la zone de contenu de son propriétaire.
    #[must_use]
    pub const fn bounds(&self) -> AirRectangle {
        self.bounds
    }

    /// Déplace ou redimensionne la vue.
    pub const fn set_bounds(&mut self, bounds: AirRectangle) {
        self.bounds = bounds;
    }

    /// La zone disponible pour le contenu : les limites, moins une cellule de chaque côté
    /// quand la bordure est présente.
    ///
    /// Rend `None` quand la bordure ne laisse rien — une vue bordée de deux cellules de
    /// large n'a pas d'intérieur, et l'appelant doit le savoir (ADR-010).
    #[must_use]
    pub fn content_bounds(&self) -> Option<AirRectangle> {
        if self.border {
            self.bounds.inset(1)
        } else {
            Some(self.bounds)
        }
    }

    /// Vrai si la vue porte une bordure.
    #[must_use]
    pub const fn has_border(&self) -> bool {
        self.border
    }

    /// Met ou retire la bordure.
    pub const fn set_border(&mut self, border: bool) {
        self.border = border;
    }

    /// Le style de bordure.
    #[must_use]
    pub const fn border_style(&self) -> AirBorderStyle {
        self.border_style
    }

    /// Vrai si la vue projette une ombre portée.
    #[must_use]
    pub const fn casts_shadow(&self) -> bool {
        self.casts_shadow
    }

    /// Décide si la vue projette une ombre portée.
    ///
    /// La vue ne la dessine pas : elle tombe hors de ses limites, et c'est son
    /// propriétaire qui la peint (ADR-015). Le thème décide de sa taille et la palette de
    /// sa couleur, donc une vue qui en projette n'en projette pas dans un thème qui n'en
    /// a pas.
    pub const fn set_casts_shadow(&mut self, casts_shadow: bool) {
        self.casts_shadow = casts_shadow;
    }

    /// Choisit le style de bordure.
    pub const fn set_border_style(&mut self, style: AirBorderStyle) {
        self.border_style = style;
    }

    /// Vrai si la vue est visible.
    #[must_use]
    pub const fn is_visible(&self) -> bool {
        self.visible
    }

    /// Montre ou cache la vue.
    pub const fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    /// Vrai si la vue détient le focus parmi ses sœurs.
    #[must_use]
    pub const fn is_focused(&self) -> bool {
        self.focused
    }

    /// Donne ou retire le focus.
    pub const fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Vrai si la vue réagit aux événements.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Active ou désactive la vue.
    pub const fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Vrai si la vue voit les événements avant celle qui a le focus.
    #[must_use]
    pub const fn is_pre_process(&self) -> bool {
        self.pre_process
    }

    /// Fait voir à la vue les événements avant celle qui a le focus.
    pub const fn set_pre_process(&mut self, pre_process: bool) {
        self.pre_process = pre_process;
    }

    /// Vrai si la vue voit les événements après celle qui a le focus.
    #[must_use]
    pub const fn is_post_process(&self) -> bool {
        self.post_process
    }

    /// Fait voir à la vue les événements après celle qui a le focus.
    pub const fn set_post_process(&mut self, post_process: bool) {
        self.post_process = post_process;
    }
}

/// Ce que toute vue sait faire.
///
/// Le trait est **object-safe** : un propriétaire tient des `Box<dyn AirView>`, donc
/// l'arbre ne porte aucun paramètre de type et une application peut y mêler ses propres
/// vues.
///
/// # Pourquoi `Any`
///
/// L'effacement de type a un prix exact : une fois la vue d'édition posée dans l'arbre,
/// plus personne ne peut lui demander de se synchroniser sur le modèle. Un contrôleur MVC
/// a besoin de retrouver **ses** vues avec leur type, et les deux autres solutions se
/// paient plus cher — un paramètre de type sur tous les conteneurs se propage jusqu'à
/// l'application, et une méthode par type de vue dans le trait ferait du cœur le
/// catalogue de ses widgets.
///
/// Le lien d'héritage `Any` permet à [`find`] et [`find_mut`] de rendre une vue avec son
/// type, sans coût à l'exécution autre qu'une comparaison d'identifiant de type, et sans
/// une ligne de plus dans les vues. Il impose `'static` aux implémentations, ce qu'elles
/// sont toutes : une vue ne contient pas d'emprunt.
pub trait AirView: Any {
    /// L'état commun, en lecture.
    fn state(&self) -> &AirViewState;

    /// L'état commun, en écriture.
    fn state_mut(&mut self) -> &mut AirViewState;

    /// Dessine le contenu, en coordonnées de la **zone de contenu**.
    ///
    /// Le peintre reçu est déjà découpé à l'intérieur de la bordure : une vue ne peut pas
    /// écrire sur son propre cadre, même en essayant.
    fn draw_content(&self, painter: &mut AirPainter<'_>, context: &AirDrawContext<'_>);

    /// Traite un événement. Par défaut, la vue n'est concernée par rien.
    fn handle_event(&mut self, event: &mut AirEvent, context: &mut AirContext<'_>) -> AirOutcome {
        let _ = (event, context);
        AirOutcome::Ignored
    }

    /// Le titre à inscrire sur la bordure, s'il y en a un.
    ///
    /// Dessiné par la traversée et non par la vue, pour la même raison que la bordure : le
    /// titre est sur le cadre, et une vue ne peut pas écrire sur son propre cadre.
    fn title(&self) -> Option<&str> {
        None
    }

    /// Inscrit des décorations sur la **bordure**, après le cadre et le titre.
    ///
    /// Le peintre reçu couvre les limites entières de la vue, bordure comprise : c'est la
    /// seule exception à la règle « une vue n'écrit pas sur son cadre », et elle est
    /// explicite. Une case de fermeture est sur le cadre par définition ; la lui interdire
    /// obligerait à la dessiner depuis le propriétaire, qui n'a aucune raison de la
    /// connaître.
    fn draw_frame_decorations(&self, painter: &mut AirPainter<'_>, context: &AirDrawContext<'_>) {
        let _ = (painter, context);
    }

    /// La position du curseur dans la **zone de contenu**, s'il y en a un.
    ///
    /// Seule une vue **focalisée** voit son curseur retenu : c'est la traversée qui
    /// applique cette règle, pas la vue. Un champ de saisie peut donc répondre sans
    /// savoir s'il a le focus, et deux champs ne peuvent pas revendiquer le curseur en
    /// même temps.
    fn cursor(&self) -> Option<AirPoint> {
        None
    }

    /// Redimensionne la vue pour occuper un écran entier.
    ///
    /// Par défaut, la vue prend ces limites à partir de l'origine. Un conteneur qui porte
    /// une **disposition** — [`crate::application::AirScreen`] et ses barres — la
    /// redéfinit pour replacer ses pièces.
    ///
    /// Existe pour que [`crate::application::AirApplication`] puisse suivre un
    /// redimensionnement de terminal sans connaître le type de la racine qu'on lui confie.
    fn resize(&mut self, size: AirSize) {
        self.state_mut()
            .set_bounds(AirRectangle::new(AirPoint::ORIGIN, size));
    }

    /// Les vues contenues, de l'arrière vers l'avant. Vide par défaut.
    fn children(&self) -> &[Box<dyn AirView>] {
        &[]
    }

    /// Les vues contenues, en écriture.
    fn children_mut(&mut self) -> &mut [Box<dyn AirView>] {
        &mut []
    }
}

/// Dessine une vue et sa descendance, et rend la position **absolue** du curseur.
///
/// `painter` doit être déjà borné aux limites de `view`. L'ordre est celui de Turbo
/// Vision : la bordure, le contenu, puis les enfants de l'arrière vers l'avant — ce qui
/// fait qu'une fenêtre au premier plan recouvre celles d'en dessous sans rien savoir
/// d'elles.
///
/// Le curseur rendu est celui de la vue focalisée **la plus profonde** : la descente
/// retient le dernier trouvé. Une vue non focalisée n'en publie pas, et un curseur qui
/// n'a pas passé le découpage n'en est pas un — il serait allé s'afficher sur la vue qui
/// recouvre.
pub fn draw_view(
    view: &dyn AirView,
    painter: &mut AirPainter<'_>,
    context: &AirDrawContext<'_>,
) -> Option<AirPoint> {
    if !view.state().is_visible() {
        return None;
    }
    if view.state().has_border() {
        draw_border(view.state(), painter, context.theme);
        if let Some(title) = view.title() {
            draw_title(painter, context.theme, title);
        }
        view.draw_frame_decorations(painter, context);
    }
    let own_size = painter.size();
    let whole = AirRectangle::new(AirPoint::ORIGIN, own_size);
    let interior = if view.state().has_border() {
        whole.inset(1)
    } else {
        Some(whole)
    };
    let Some(interior) = interior else {
        // Bordée et trop petite pour avoir un intérieur : le cadre est dessiné, le
        // contenu n'a nulle part où aller.
        return None;
    };
    let mut interior_painter = painter.child(interior)?;
    view.draw_content(&mut interior_painter, context);
    let mut cursor = if view.state().is_focused() {
        view.cursor()
            .and_then(|point| interior_painter.absolute_position(point))
    } else {
        None
    };
    for child in view.children() {
        // L'ombre d'abord, l'enfant ensuite, et dans l'ordre arrière-vers-avant de la
        // boucle : l'ombre d'une fenêtre de premier plan tombe donc sur celles d'en
        // dessous, et le corps d'une fenêtre de premier plan recouvre l'ombre qu'une
        // autre y aurait projetée. Une passe globale des ombres ne saurait pas distinguer
        // les deux (ADR-015).
        draw_child_shadow(&mut interior_painter, child.as_ref(), context);
        if let Some(mut child_painter) = interior_painter.child(child.state().bounds())
            && let Some(found) = draw_view(child.as_ref(), &mut child_painter, context)
        {
            cursor = Some(found);
        }
    }
    cursor
}

/// Peint l'ombre d'un enfant, dans les limites du **propriétaire**.
///
/// C'est la décision de l'ADR-015 : une vue ne peint pas hors de ses limites, et ce qui
/// dépasse d'elle est peint par le propriétaire de la surface où cela dépasse. Le peintre
/// reçu est donc celui du parent, et son découpage s'applique tel quel — une fenêtre
/// contre le bord du bureau ne projette qu'un morceau d'ombre, ce qui est juste.
///
/// L'ombre **recolore** ce qui est dessous en gardant le caractère : c'est ce qui lui donne
/// l'aspect d'une ombre sur le motif du bureau plutôt que d'un rectangle collé à la
/// fenêtre.
fn draw_child_shadow(
    painter: &mut AirPainter<'_>,
    child: &dyn AirView,
    context: &AirDrawContext<'_>,
) {
    if !child.state().is_visible() || !child.state().casts_shadow() {
        return;
    }
    let Some(shadow) = context.theme.decorations.shadow else {
        return;
    };
    let colors = context.theme.palette.color_pair(AirRole::WindowShadow);
    let bounds = child.state().bounds();
    // La zone d'ombre est la vue **décalée**, moins la vue elle-même : les cellules qui
    // restent dessous vont être recouvertes, et les recolorer serait du travail perdu.
    let shadowed = AirRectangle::new(
        AirPoint::new(
            AirColumn::new(bounds.left().saturating_add(shadow.columns)),
            AirRow::new(bounds.top().saturating_add(shadow.rows)),
        ),
        bounds.size,
    );
    let mut row = shadowed.top();
    while row < shadowed.bottom_exclusive() {
        let mut column = shadowed.left();
        while column < shadowed.right_exclusive() {
            let point = AirPoint::new(AirColumn::new(column), AirRow::new(row));
            if !bounds.contains(point) {
                shade(painter, point, colors);
            }
            column = column.saturating_add(1);
        }
        row = row.saturating_add(1);
    }
}

/// Recolore une cellule aux couleurs de l'ombre, en gardant son caractère.
fn shade(painter: &mut AirPainter<'_>, point: AirPoint, colors: AirColorPair) {
    let character = painter.cell(point).map_or(' ', |cell| cell.character);
    painter.set_cell(point, colors.cell(character));
}

/// Trace la bordure d'une vue avec les glyphes et les couleurs du thème.
fn draw_border(state: &AirViewState, painter: &mut AirPainter<'_>, theme: &AirTheme) {
    let size = painter.size();
    if size.columns < 2 || size.rows < 2 {
        // Pas la place d'un cadre : en dessiner un morceau serait pire que rien.
        return;
    }
    let glyphs = match state.border_style() {
        AirBorderStyle::Single => theme.glyphs.window_frame,
        AirBorderStyle::Double => theme.glyphs.dialog_frame,
    };
    let role = if state.is_focused() {
        AirRole::WindowFrameActive
    } else {
        AirRole::WindowFrameInactive
    };
    let colors = theme.palette.color_pair(role);
    let right = size.columns.saturating_sub(1);
    let bottom = size.rows.saturating_sub(1);

    let mut put = |column: u16, row: u16, character: char| {
        painter.set_cell(
            AirPoint::new(AirColumn::new(column), AirRow::new(row)),
            colors.cell(character),
        );
    };

    let mut column = 1;
    while column < right {
        put(column, 0, glyphs.horizontal);
        put(column, bottom, glyphs.horizontal);
        column = column.saturating_add(1);
    }
    let mut row = 1;
    while row < bottom {
        put(0, row, glyphs.vertical);
        put(right, row, glyphs.vertical);
        row = row.saturating_add(1);
    }
    put(0, 0, glyphs.top_left);
    put(right, 0, glyphs.top_right);
    put(0, bottom, glyphs.bottom_left);
    put(right, bottom, glyphs.bottom_right);
}

/// Inscrit un titre centré sur le bord supérieur d'un cadre.
///
/// Le titre est entouré d'espaces pour le détacher du trait, et il est **tronqué** plutôt
/// que débordé : un titre trop long qui mangerait les angles rendrait le cadre illisible.
fn draw_title(painter: &mut AirPainter<'_>, theme: &AirTheme, title: &str) {
    let size = painter.size();
    // Deux angles, et au moins une cellule de trait de chaque côté.
    let available = size.columns.saturating_sub(4);
    if available == 0 || title.is_empty() {
        return;
    }
    let colors = theme.palette.color_pair(AirRole::WindowTitle);
    let width = u16::try_from(crate::unicode::text_width(title)).unwrap_or(u16::MAX);
    let decorated = width.saturating_add(2).min(available);
    let free = available.saturating_sub(decorated);
    let start = 2_u16.saturating_add(free / 2);
    let mut column = AirColumn::new(start);
    for text in [" ", title, " "] {
        column = painter.draw_text_with_colors(AirPoint::new(column, AirRow::FIRST), text, colors);
    }
}

/// Les glyphes de cadre d'un style, dans un thème.
///
/// Exposé pour qu'un widget qui dessine son propre cadre — un séparateur, un onglet —
/// emploie les mêmes caractères que la bordure du framework.
#[must_use]
pub const fn frame_glyphs(theme: &AirTheme, style: AirBorderStyle) -> AirFrameGlyphs {
    match style {
        AirBorderStyle::Single => theme.glyphs.window_frame,
        AirBorderStyle::Double => theme.glyphs.dialog_frame,
    }
}

/// Distribue un événement à une vue et à sa descendance.
///
/// L'ordre, et pourquoi il est celui-là :
///
/// 1. **la vue elle-même**, pour qu'un conteneur puisse intercepter — une fenêtre modale
///    doit pouvoir retenir une frappe avant que ses enfants la voient ;
/// 2. **ses enfants**, par la position pour la souris, par les trois phases sinon.
///
/// # Les relâchements n'entrent pas dans l'arbre
///
/// Un relâchement de touche est **écarté ici**, une fois, avant la descente. Le protocole
/// clavier Kitty les rapporte dès que le type d'événement est demandé (`\x1b[>23u`), et
/// une vue qui les traiterait agirait deux fois par frappe : une flèche bas descendrait de
/// deux lignes, un `Inser` marquerait puis démarquerait.
///
/// Le filtre est à cet endroit et non dans chaque widget, pour deux raisons. La règle est
/// la même pour tous — douze widgets qui la répètent sont douze endroits où elle peut
/// manquer, et c'est ainsi que le jalon M7b a livré onze widgets exposés. Et
/// l'information n'est pas détruite : une boucle d'application tient l'[`AirInputEvent`]
/// avant de le distribuer, donc qui a besoin des relâchements les a encore.
///
/// Une **répétition** passe, elle, et c'est voulu : maintenir une flèche doit répéter le
/// déplacement. Filtrer « tout ce qui n'est pas un enfoncement » serait le défaut
/// symétrique, moins visible et plus pénible.
pub fn dispatch_event(
    view: &mut dyn AirView,
    event: &mut AirEvent,
    context: &mut AirContext<'_>,
) -> AirOutcome {
    if is_key_release(event) {
        return AirOutcome::Ignored;
    }
    dispatch_into(view, event, context)
}

/// Vrai si l'événement est un relâchement de touche.
fn is_key_release(event: &AirEvent) -> bool {
    matches!(
        event.input(),
        Some(AirInputEvent::Key(key)) if key.state == AirKeyState::Release
    )
}

/// Le corps récursif de [`dispatch_event`], sans le filtre des relâchements.
///
/// Séparé pour que le filtre soit évalué une fois par événement et non une fois par vue
/// traversée.
pub(crate) fn dispatch_into(
    view: &mut dyn AirView,
    event: &mut AirEvent,
    context: &mut AirContext<'_>,
) -> AirOutcome {
    if !view.state().is_visible() || !view.state().is_enabled() {
        return AirOutcome::Ignored;
    }
    let own = view.handle_event(event, context);
    if event.is_consumed() || own != AirOutcome::Ignored {
        return own;
    }
    if event.is_mouse() {
        return dispatch_positional(view, event, context);
    }
    dispatch_phases(view, event, context)
}

/// Distribue un événement de souris à l'enfant qui est sous le pointeur.
///
/// Du premier plan vers l'arrière : c'est la fenêtre du dessus qui reçoit le clic, et
/// c'est le seul ordre qui corresponde à ce que l'utilisateur voit.
fn dispatch_positional(
    view: &mut dyn AirView,
    event: &mut AirEvent,
    context: &mut AirContext<'_>,
) -> AirOutcome {
    let Some(position) = event.mouse_position() else {
        return AirOutcome::Ignored;
    };
    let Some(inner) = view.state().content_bounds() else {
        return AirOutcome::Ignored;
    };
    // La position arrive dans les coordonnées de cette vue ; les limites des enfants sont
    // relatives à la zone de contenu, décalée d'une cellule quand la vue est bordée.
    let content_origin = AirPoint::new(
        AirColumn::new(inner.left().saturating_sub(view.state().bounds().left())),
        AirRow::new(inner.top().saturating_sub(view.state().bounds().top())),
    );
    let hit = {
        let children = view.children();
        let mut found = None;
        for (index, child) in children.iter().enumerate().rev() {
            if !child.state().is_visible() {
                continue;
            }
            let child_bounds = child.state().bounds().translated(content_origin);
            if child_bounds.contains(position) {
                found = Some((index, child_bounds.origin));
                break;
            }
        }
        found
    };
    let Some((index, origin)) = hit else {
        return AirOutcome::Ignored;
    };
    event.shift_into(origin);
    let outcome = match view.children_mut().get_mut(index) {
        Some(child) => dispatch_into(child.as_mut(), event, context),
        // Inatteignable : l'indice vient d'un parcours de la même collection.
        None => AirOutcome::Ignored,
    };
    event.shift_out_of(origin);
    outcome
}

/// Distribue un événement aux enfants selon les trois phases.
fn dispatch_phases(
    view: &mut dyn AirView,
    event: &mut AirEvent,
    context: &mut AirContext<'_>,
) -> AirOutcome {
    for phase in [
        AirPhase::PreProcess,
        AirPhase::Focused,
        AirPhase::PostProcess,
    ] {
        event.phase = phase;
        let targets: Vec<usize> = match phase {
            AirPhase::PreProcess => indices_where(view, AirViewState::is_pre_process),
            AirPhase::Focused => indices_where(view, AirViewState::is_focused),
            AirPhase::PostProcess => indices_where(view, AirViewState::is_post_process),
        };
        for index in targets {
            if event.is_consumed() {
                return AirOutcome::Consumed;
            }
            let outcome = match view.children_mut().get_mut(index) {
                Some(child) => dispatch_into(child.as_mut(), event, context),
                // Inatteignable : les indices viennent de la même collection.
                None => AirOutcome::Ignored,
            };
            if outcome != AirOutcome::Ignored {
                return outcome;
            }
        }
    }
    AirOutcome::Ignored
}

/// Les indices des enfants visibles et actifs qui satisfont un prédicat, du premier plan
/// vers l'arrière.
fn indices_where(view: &dyn AirView, predicate: fn(&AirViewState) -> bool) -> Vec<usize> {
    view.children()
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, child)| {
            let state = child.state();
            state.is_visible() && state.is_enabled() && predicate(state)
        })
        .map(|(index, _)| index)
        .collect()
}

/// La vue d'identifiant donné dans un arbre, rendue avec son type d'origine.
///
/// Rend `None` quand l'identifiant est absent **ou** quand la vue trouvée n'est pas du
/// type demandé : se tromper de type est une erreur de programme qui mérite une absence,
/// pas une panique.
///
/// La recherche est en profondeur, dans l'ordre de l'arbre. Un contrôleur qui s'en sert à
/// chaque tour de boucle garde les identifiants de ses vues et paie un parcours de
/// quelques dizaines de nœuds, ce qui est sans commune mesure avec le rendu.
#[must_use]
pub fn find<ViewType: AirView>(view: &dyn AirView, identifier: AirViewId) -> Option<&ViewType> {
    if view.state().identifier() == identifier {
        return (view as &dyn Any).downcast_ref::<ViewType>();
    }
    for child in view.children() {
        if let Some(found) = find::<ViewType>(child.as_ref(), identifier) {
            return Some(found);
        }
    }
    None
}

/// La vue d'identifiant donné dans un arbre, en écriture et avec son type d'origine.
pub fn find_mut<ViewType: AirView>(
    view: &mut dyn AirView,
    identifier: AirViewId,
) -> Option<&mut ViewType> {
    if view.state().identifier() == identifier {
        return (view as &mut dyn Any).downcast_mut::<ViewType>();
    }
    for child in view.children_mut() {
        if let Some(found) = find_mut::<ViewType>(child.as_mut(), identifier) {
            return Some(found);
        }
    }
    None
}

/// La dimension minimale d'une vue bordée pour qu'elle ait un intérieur.
///
/// Trois cellules dans chaque direction : deux pour le cadre, une au moins pour le
/// contenu. Exposé pour qu'un widget refuse une dimension absurde plutôt que de la
/// découvrir à l'affichage.
#[must_use]
pub const fn minimum_bordered_size() -> AirSize {
    AirSize::new(3, 3)
}

#[cfg(test)]
mod tests {
    use core::cell::RefCell;
    use std::rc::Rc;

    use super::{
        AirBorderStyle, AirContext, AirDrawContext, AirEvent, AirGroup, AirOutcome, AirPainter,
        AirPhase, AirView, AirViewId, AirViewState, dispatch_event, draw_view, find, find_mut,
        frame_glyphs, minimum_bordered_size,
    };
    use crate::cell::{AirAttributes, AirCell, AirColor};
    use crate::command::{AirCommandSet, SAVE};
    use crate::geometry::{AirColumn, AirPoint, AirRectangle, AirRow, AirSize};
    use crate::input::event::{
        AirInputEvent, AirKey, AirKeyEvent, AirKeyState, AirModifiers, AirMouseButton,
        AirMouseEvent, AirMouseEventKind,
    };
    use crate::surface::AirSurface;
    use crate::theme::{AirRole, AirTheme};
    use crate::widgets::{AirPanel, AirWindow};

    fn identifier(value: u32) -> AirViewId {
        AirViewId::new(value).expect("un identifiant non nul")
    }

    fn state(value: u32, bounds: AirRectangle) -> AirViewState {
        AirViewState::new(identifier(value), bounds)
    }

    fn point(column: u16, row: u16) -> AirPoint {
        AirPoint::new(AirColumn::new(column), AirRow::new(row))
    }

    // ------------------------------------------------- l'ombre portée (ADR-015)

    /// Un bureau d'une seule fenêtre, et la surface où il est peint.
    ///
    /// Le bureau est **sans bordure** : sans cela, la fenêtre serait décalée d'une cellule
    /// et chaque attente devrait compter ce décalage.
    fn desktop_with_window(size: AirSize, window_bounds: AirRectangle) -> (AirSurface, AirTheme) {
        let theme = AirTheme::turbo_vision();
        let mut surface = AirSurface::new(size);
        let mut root = AirPanel::new(state_without_border(
            1,
            AirRectangle::new(AirPoint::ORIGIN, size),
        ));
        root.insert(Box::new(AirWindow::new(state(2, window_bounds))));
        {
            let mut painter = AirPainter::new(&mut surface);
            draw_view(
                &root,
                &mut painter,
                &AirDrawContext::new(&theme, &AirCommandSet::new()),
            );
        }
        (surface, theme)
    }

    /// Un état sans bordure, pour un conteneur qui ne doit pas décaler ses enfants.
    fn state_without_border(value: u32, bounds: AirRectangle) -> AirViewState {
        let mut state = state(value, bounds);
        state.set_border(false);
        state
    }

    #[test]
    fn une_fenetre_projette_une_ombre_en_bas_et_a_droite() {
        // Deux colonnes à droite et une ligne en dessous : l'ADR-008 nomme les deux
        // colonnes dans son contexte.
        let (surface, theme) =
            desktop_with_window(AirSize::new(14, 8), AirRectangle::from_parts(2, 1, 6, 4));
        let shadow = theme.palette.color_pair(AirRole::WindowShadow);
        // À droite de la fenêtre, sur une ligne qu'elle occupe.
        for column in 8..10 {
            assert_eq!(
                surface.cell(point(column, 2)).map(|cell| cell.background),
                Some(shadow.background),
                "colonne {column} : la bande de droite doit être ombrée"
            );
        }
        // Sous la fenêtre, décalée de deux colonnes.
        for column in 4..10 {
            assert_eq!(
                surface.cell(point(column, 5)).map(|cell| cell.background),
                Some(shadow.background),
                "colonne {column} : la bande du bas doit être ombrée"
            );
        }
    }

    #[test]
    fn l_ombre_ne_couvre_pas_la_fenetre() {
        // Elle est la vue décalée **moins** la vue : recolorer ce que la fenêtre va
        // recouvrir serait du travail perdu, et le prouver interdit la régression.
        let (surface, theme) =
            desktop_with_window(AirSize::new(14, 8), AirRectangle::from_parts(2, 1, 6, 4));
        let shadow = theme.palette.color_pair(AirRole::WindowShadow);
        let frame = theme.palette.color_pair(AirRole::WindowFrameInactive);
        assert_eq!(
            surface.cell(point(2, 1)).map(|cell| cell.background),
            Some(frame.background),
            "l'angle de la fenêtre doit porter le cadre, pas l'ombre"
        );
        assert_ne!(
            shadow.background, frame.background,
            "l'essai doit discriminer"
        );
    }

    #[test]
    fn l_ombre_garde_le_caractere_qu_elle_assombrit() {
        // C'est ce qui fait une ombre plutôt qu'un rectangle, et le vrai bureau est le
        // seul conteneur qui le montre : il remplit son fond du `░` de Turbo Vision, là
        // où un panneau le remplit d'espaces.
        let theme = AirTheme::turbo_vision();
        let size = AirSize::new(14, 8);
        let mut surface = AirSurface::new(size);
        let mut desktop = crate::application::AirDesktop::new(state(
            1,
            AirRectangle::new(AirPoint::ORIGIN, size),
        ));
        desktop.insert(Box::new(AirWindow::new(state(
            2,
            AirRectangle::from_parts(2, 1, 6, 4),
        ))));
        {
            let mut painter = AirPainter::new(&mut surface);
            draw_view(
                &desktop,
                &mut painter,
                &AirDrawContext::new(&theme, &AirCommandSet::new()),
            );
        }
        let shaded = surface
            .cell(point(4, 5))
            .copied()
            .expect("une cellule ombrée");
        assert_eq!(
            shaded.character, theme.glyphs.desktop_fill,
            "le motif du bureau doit survivre à l'ombre"
        );
        assert_eq!(
            shaded.background,
            theme.palette.color_pair(AirRole::WindowShadow).background
        );
    }

    #[test]
    fn un_theme_sans_ombre_n_en_peint_aucune() {
        // L'ombre s'éteint d'un seul endroit, et aucune vue n'a à le savoir.
        let theme = AirTheme {
            decorations: AirTheme::turbo_vision().decorations.without_shadow(),
            ..AirTheme::turbo_vision()
        };
        let size = AirSize::new(14, 8);
        let mut surface = AirSurface::new(size);
        let mut root = AirPanel::new(state_without_border(
            1,
            AirRectangle::new(AirPoint::ORIGIN, size),
        ));
        root.insert(Box::new(AirWindow::new(state(
            2,
            AirRectangle::from_parts(2, 1, 6, 4),
        ))));
        {
            let mut painter = AirPainter::new(&mut surface);
            draw_view(
                &root,
                &mut painter,
                &AirDrawContext::new(&theme, &AirCommandSet::new()),
            );
        }
        let shadow = theme.palette.color_pair(AirRole::WindowShadow);
        assert_ne!(
            surface.cell(point(4, 5)).map(|cell| cell.background),
            Some(shadow.background)
        );
    }

    #[test]
    fn une_vue_qui_ne_projette_pas_d_ombre_n_en_projette_pas() {
        // `casts_shadow` est faux par défaut : une ombre sur chaque vue couvrirait
        // l'écran de gris.
        let theme = AirTheme::turbo_vision();
        let size = AirSize::new(14, 8);
        let mut surface = AirSurface::new(size);
        let mut root = AirPanel::new(state_without_border(
            1,
            AirRectangle::new(AirPoint::ORIGIN, size),
        ));
        root.insert(Box::new(AirPanel::new(state(
            2,
            AirRectangle::from_parts(2, 1, 6, 4),
        ))));
        {
            let mut painter = AirPainter::new(&mut surface);
            draw_view(
                &root,
                &mut painter,
                &AirDrawContext::new(&theme, &AirCommandSet::new()),
            );
        }
        let shadow = theme.palette.color_pair(AirRole::WindowShadow);
        assert_ne!(
            surface.cell(point(4, 5)).map(|cell| cell.background),
            Some(shadow.background),
            "un panneau ne projette pas d'ombre"
        );
    }

    #[test]
    fn l_ombre_est_decoupee_comme_tout_le_reste() {
        // Une fenêtre contre le bord du bureau n'en projette qu'un morceau. Ce n'est pas
        // un cas à traiter : c'est le découpage du peintre, que l'ADR-015 ne touche pas.
        let (surface, theme) =
            desktop_with_window(AirSize::new(10, 6), AirRectangle::from_parts(2, 1, 8, 5));
        let shadow = theme.palette.color_pair(AirRole::WindowShadow);
        // La bande de droite tomberait aux colonnes 10 et 11, hors de la surface.
        assert_eq!(surface.cell(point(10, 2)), None, "hors de la surface");
        // La bande du bas tomberait à la ligne 6, hors de la surface elle aussi.
        assert_eq!(surface.cell(point(4, 6)), None);
        // Et rien n'a paniqué : la fenêtre est bien dessinée.
        assert_ne!(
            surface.cell(point(2, 1)).map(|cell| cell.background),
            Some(shadow.background)
        );
    }

    #[test]
    fn l_ombre_d_une_fenetre_de_premier_plan_tombe_sur_celle_d_en_dessous() {
        // L'ordre à deux temps : les enfants sont dessinés de l'arrière vers l'avant, et
        // l'ombre de chacun juste avant lui. Une passe globale des ombres ne saurait pas
        // distinguer « derrière » de « devant » (ADR-015).
        let theme = AirTheme::turbo_vision();
        let size = AirSize::new(20, 10);
        let mut surface = AirSurface::new(size);
        let mut root = AirPanel::new(state_without_border(
            1,
            AirRectangle::new(AirPoint::ORIGIN, size),
        ));
        // Celle d'arrière-plan d'abord, celle de premier plan ensuite.
        root.insert(Box::new(AirWindow::new(state(
            2,
            AirRectangle::from_parts(0, 0, 10, 6),
        ))));
        root.insert(Box::new(AirWindow::new(state(
            3,
            AirRectangle::from_parts(6, 3, 10, 6),
        ))));
        {
            let mut painter = AirPainter::new(&mut surface);
            draw_view(
                &root,
                &mut painter,
                &AirDrawContext::new(&theme, &AirCommandSet::new()),
            );
        }
        let shadow = theme.palette.color_pair(AirRole::WindowShadow);
        // (8, 4) est dans la fenêtre de premier plan : son corps l'emporte.
        assert_ne!(
            surface.cell(point(8, 4)).map(|cell| cell.background),
            Some(shadow.background),
            "le corps du premier plan recouvre tout"
        );
        // (8, 9) est sous la fenêtre de premier plan, hors de celle d'arrière-plan.
        assert_eq!(
            surface.cell(point(8, 9)).map(|cell| cell.background),
            Some(shadow.background),
            "l'ombre du premier plan tombe sur le bureau"
        );
    }

    fn key_event(character: char) -> AirEvent {
        AirEvent::new(AirInputEvent::Key(AirKeyEvent::press(AirKey::Character(
            character,
        ))))
    }

    fn mouse_event(column: u16, row: u16) -> AirEvent {
        AirEvent::new(AirInputEvent::Mouse(AirMouseEvent {
            position: point(column, row),
            kind: AirMouseEventKind::Pressed(AirMouseButton::Left),
            modifiers: AirModifiers::NONE,
        }))
    }

    /// Une vue de test : elle note ce qu'elle a vu et peut consommer.
    struct Spy {
        state: AirViewState,
        /// Le caractère dont elle remplit son contenu.
        fill: char,
        /// Les phases où elle a été appelée, dans l'ordre.
        seen: Vec<AirPhase>,
        /// Les positions de souris reçues, dans ses propres coordonnées.
        positions: Vec<AirPoint>,
        /// Vrai si elle consomme ce qu'elle reçoit.
        consumes: bool,
        /// Ce qu'elle rend.
        outcome: AirOutcome,
        /// Le curseur qu'elle réclame, en coordonnées de son contenu.
        cursor: Option<AirPoint>,
    }

    impl Spy {
        fn new(value: u32, bounds: AirRectangle, fill: char) -> Self {
            Self {
                state: state(value, bounds),
                fill,
                seen: Vec::new(),
                positions: Vec::new(),
                consumes: false,
                outcome: AirOutcome::Ignored,
                cursor: None,
            }
        }

        fn without_border(mut self) -> Self {
            self.state.set_border(false);
            self
        }

        fn with_cursor(mut self, column: u16, row: u16) -> Self {
            self.cursor = Some(AirPoint::new(AirColumn::new(column), AirRow::new(row)));
            self
        }
    }

    impl AirView for Spy {
        fn state(&self) -> &AirViewState {
            &self.state
        }

        fn state_mut(&mut self) -> &mut AirViewState {
            &mut self.state
        }

        fn cursor(&self) -> Option<AirPoint> {
            self.cursor
        }

        fn draw_content(&self, painter: &mut AirPainter<'_>, _context: &AirDrawContext<'_>) {
            let size = painter.size();
            painter.fill_rectangle(
                AirRectangle::new(AirPoint::ORIGIN, size),
                AirCell::new(self.fill, AirColor::WHITE, AirColor::BLUE),
            );
        }

        fn handle_event(
            &mut self,
            event: &mut AirEvent,
            _context: &mut AirContext<'_>,
        ) -> AirOutcome {
            self.seen.push(event.phase());
            if let Some(position) = event.mouse_position() {
                self.positions.push(position);
            }
            if self.consumes {
                event.consume();
                return AirOutcome::Consumed;
            }
            self.outcome
        }
    }

    fn rendered(surface: &AirSurface, size: AirSize) -> Vec<String> {
        (0..size.rows)
            .map(|row| {
                (0..size.columns)
                    .map(|column| {
                        surface
                            .cell(point(column, row))
                            .map_or(' ', |found| found.character)
                    })
                    .collect()
            })
            .collect()
    }

    // ----------------------------------------------------- identifiants

    #[test]
    fn un_identifiant_nul_n_existe_pas() {
        // Une vue détruite ne doit pas laisser un identifiant qui semble valide.
        assert_eq!(AirViewId::new(0), None);
        assert_eq!(identifier(7).value(), 7);
        assert_eq!(AirViewId::FIRST.value(), 1);
    }

    // ------------------------------------------------------------ état

    #[test]
    fn une_vue_neuve_est_bordee_visible_et_active() {
        let fresh = state(1, AirRectangle::from_parts(0, 0, 10, 5));
        assert!(fresh.has_border(), "ADR-010 : bordée par défaut");
        assert!(fresh.is_visible());
        assert!(fresh.is_enabled());
        assert!(!fresh.is_focused());
        assert!(!fresh.is_pre_process());
        assert!(!fresh.is_post_process());
        assert_eq!(fresh.border_style(), AirBorderStyle::Single);
    }

    #[test]
    fn la_zone_de_contenu_d_une_vue_bordee_perd_une_cellule_de_chaque_cote() {
        let bordered = state(1, AirRectangle::from_parts(2, 3, 10, 6));
        assert_eq!(
            bordered.content_bounds(),
            Some(AirRectangle::from_parts(3, 4, 8, 4))
        );
    }

    #[test]
    fn la_zone_de_contenu_d_une_vue_sans_bordure_est_ses_limites() {
        let mut plain = state(1, AirRectangle::from_parts(2, 3, 10, 6));
        plain.set_border(false);
        assert_eq!(plain.content_bounds(), Some(plain.bounds()));
    }

    #[test]
    fn une_vue_bordee_trop_petite_n_a_pas_d_interieur() {
        // L'appelant doit le savoir plutôt que de recevoir un rectangle vide (ADR-010).
        let cramped = state(1, AirRectangle::from_parts(0, 0, 2, 2));
        assert_eq!(cramped.content_bounds(), None);
        assert_eq!(minimum_bordered_size(), AirSize::new(3, 3));
    }

    #[test]
    fn les_accesseurs_d_etat_ecrivent_ce_qu_ils_lisent() {
        let mut mutable = state(1, AirRectangle::from_parts(0, 0, 4, 4));
        mutable.set_bounds(AirRectangle::from_parts(1, 1, 6, 6));
        assert_eq!(mutable.bounds(), AirRectangle::from_parts(1, 1, 6, 6));
        mutable.set_border_style(AirBorderStyle::Double);
        assert_eq!(mutable.border_style(), AirBorderStyle::Double);
        mutable.set_visible(false);
        assert!(!mutable.is_visible());
        mutable.set_focused(true);
        assert!(mutable.is_focused());
        mutable.set_enabled(false);
        assert!(!mutable.is_enabled());
        mutable.set_pre_process(true);
        assert!(mutable.is_pre_process());
        mutable.set_post_process(true);
        assert!(mutable.is_post_process());
        assert_eq!(mutable.identifier(), identifier(1));
    }

    // -------------------------------------------------------- événement

    #[test]
    fn un_evenement_consomme_n_a_plus_de_charge() {
        let mut event = key_event('a');
        assert!(!event.is_consumed());
        assert!(event.input().is_some());
        event.consume();
        assert!(event.is_consumed());
        assert!(event.input().is_none());
    }

    #[test]
    fn un_evenement_de_souris_est_reconnu_et_porte_sa_position() {
        let mouse = mouse_event(4, 5);
        assert!(mouse.is_mouse());
        assert_eq!(mouse.mouse_position(), Some(point(4, 5)));
        let key = key_event('a');
        assert!(!key.is_mouse());
        assert_eq!(key.mouse_position(), None);
    }

    #[test]
    fn la_phase_initiale_est_celle_du_focus() {
        assert_eq!(key_event('a').phase(), AirPhase::Focused);
    }

    // ------------------------------------------------------- dessin

    /// Dessine une vue sur une surface neuve et rend le curseur absolu.
    fn cursor_of(view: &dyn AirView, size: AirSize) -> Option<AirPoint> {
        let theme = AirTheme::turbo_vision();
        let mut surface = AirSurface::new(size);
        let mut painter = AirPainter::new(&mut surface);
        draw_view(
            view,
            &mut painter,
            &AirDrawContext::new(&theme, &AirCommandSet::new()),
        )
    }

    #[test]
    fn une_vue_se_retrouve_dans_l_arbre_avec_son_type() {
        // C'est ce dont un contrôleur MVC a besoin : la vue est dans l'arbre, et il doit
        // pouvoir lui parler avec son type.
        let mut root = AirGroup::new(AirViewState::new(
            identifier(1),
            AirRectangle::from_parts(0, 0, 20, 10),
        ));
        let mut inner = AirGroup::new(AirViewState::new(
            identifier(2),
            AirRectangle::from_parts(0, 0, 10, 5),
        ));
        inner.insert(Box::new(Spy::new(
            3,
            AirRectangle::from_parts(0, 0, 4, 2),
            'x',
        )));
        root.insert(Box::new(inner));

        assert!(find::<Spy>(&root, identifier(3)).is_some());
        assert!(find_mut::<Spy>(&mut root, identifier(3)).is_some());
        assert!(find::<AirGroup>(&root, identifier(2)).is_some());
        assert!(find::<AirGroup>(&root, identifier(1)).is_some());
    }

    #[test]
    fn un_identifiant_absent_ou_un_type_errone_ne_rend_rien() {
        let mut root = AirGroup::new(AirViewState::new(
            identifier(1),
            AirRectangle::from_parts(0, 0, 20, 10),
        ));
        root.insert(Box::new(Spy::new(
            2,
            AirRectangle::from_parts(0, 0, 4, 2),
            'x',
        )));
        assert!(find::<Spy>(&root, identifier(99)).is_none());
        // Le bon identifiant, le mauvais type : une absence, pas une panique.
        assert!(find::<AirGroup>(&root, identifier(2)).is_none());
        assert!(find_mut::<AirGroup>(&mut root, identifier(2)).is_none());
    }

    #[test]
    fn seule_une_vue_focalisee_publie_son_curseur() {
        // La règle est appliquée par la traversée : la vue répond toujours, et deux vues
        // ne peuvent donc pas revendiquer le curseur en même temps.
        let size = AirSize::new(10, 4);
        let mut view =
            Spy::new(1, AirRectangle::new(AirPoint::ORIGIN, size), '.').with_cursor(2, 1);
        assert_eq!(cursor_of(&view, size), None);
        view.state.set_focused(true);
        // La vue est bordée : son contenu commence en (1, 1), le curseur est donc en (3, 2).
        assert_eq!(cursor_of(&view, size), Some(point(3, 2)));
    }

    #[test]
    fn le_curseur_retenu_est_celui_du_descendant_le_plus_profond() {
        let size = AirSize::new(20, 10);
        let mut group = AirGroup::new(state(1, AirRectangle::new(AirPoint::ORIGIN, size)));
        group.state_mut().set_focused(true);
        let mut child = Spy::new(2, AirRectangle::from_parts(3, 2, 8, 3), '.');
        child.state.set_focused(true);
        child.cursor = Some(point(1, 0));
        group.insert(Box::new(child));
        // Le groupe est bordé — contenu en (1, 1) ; l'enfant est bordé aussi — contenu en
        // (1, 1) de lui-même. Soit 1 + 3 + 1 + 1 en colonne et 1 + 2 + 1 + 0 en ligne.
        assert_eq!(cursor_of(&group, size), Some(point(6, 4)));
    }

    #[test]
    fn un_curseur_hors_du_decoupage_n_est_pas_rendu() {
        // Sinon il irait clignoter sur la vue qui recouvre.
        let size = AirSize::new(10, 4);
        let mut view =
            Spy::new(1, AirRectangle::new(AirPoint::ORIGIN, size), '.').with_cursor(50, 0);
        view.state.set_focused(true);
        assert_eq!(cursor_of(&view, size), None);
    }

    #[test]
    fn une_vue_invisible_ne_publie_pas_de_curseur() {
        let size = AirSize::new(10, 4);
        let mut view =
            Spy::new(1, AirRectangle::new(AirPoint::ORIGIN, size), '.').with_cursor(2, 1);
        view.state.set_focused(true);
        view.state.set_visible(false);
        assert_eq!(cursor_of(&view, size), None);
    }

    #[test]
    fn une_vue_bordee_dessine_son_cadre_avec_les_glyphes_du_theme() {
        let theme = AirTheme::turbo_vision();
        let size = AirSize::new(6, 4);
        let mut surface = AirSurface::new(size);
        let view = Spy::new(1, AirRectangle::new(AirPoint::ORIGIN, size), '.');
        {
            let mut painter = AirPainter::new(&mut surface);
            draw_view(
                &view,
                &mut painter,
                &AirDrawContext::new(&theme, &AirCommandSet::new()),
            );
        }
        assert_eq!(
            rendered(&surface, size),
            vec![
                "┌────┐".to_owned(),
                "│....│".to_owned(),
                "│....│".to_owned(),
                "└────┘".to_owned(),
            ]
        );
    }

    #[test]
    fn un_contenu_trop_long_ne_peut_pas_ecraser_la_bordure() {
        // Le défaut du jalon M0, désormais impossible : le contenu reçoit un peintre
        // découpé à l'intérieur du cadre.
        let theme = AirTheme::turbo_vision();
        let size = AirSize::new(6, 3);
        let mut surface = AirSurface::new(size);
        let view = Spy::new(1, AirRectangle::new(AirPoint::ORIGIN, size), '#');
        {
            let mut painter = AirPainter::new(&mut surface);
            draw_view(
                &view,
                &mut painter,
                &AirDrawContext::new(&theme, &AirCommandSet::new()),
            );
        }
        let lines = rendered(&surface, size);
        assert_eq!(lines.get(1).map(String::as_str), Some("│####│"));
    }

    #[test]
    fn une_vue_sans_bordure_dessine_jusqu_au_bord() {
        let theme = AirTheme::turbo_vision();
        let size = AirSize::new(4, 2);
        let mut surface = AirSurface::new(size);
        let view = Spy::new(1, AirRectangle::new(AirPoint::ORIGIN, size), '*').without_border();
        {
            let mut painter = AirPainter::new(&mut surface);
            draw_view(
                &view,
                &mut painter,
                &AirDrawContext::new(&theme, &AirCommandSet::new()),
            );
        }
        assert_eq!(
            rendered(&surface, size),
            vec!["****".to_owned(), "****".to_owned()]
        );
    }

    #[test]
    fn une_vue_invisible_ne_dessine_rien() {
        let theme = AirTheme::turbo_vision();
        let size = AirSize::new(4, 2);
        let mut surface = AirSurface::new(size);
        let mut view = Spy::new(1, AirRectangle::new(AirPoint::ORIGIN, size), '*');
        view.state.set_visible(false);
        {
            let mut painter = AirPainter::new(&mut surface);
            draw_view(
                &view,
                &mut painter,
                &AirDrawContext::new(&theme, &AirCommandSet::new()),
            );
        }
        assert_eq!(
            rendered(&surface, size),
            vec!["    ".to_owned(), "    ".to_owned()]
        );
    }

    #[test]
    fn une_vue_bordee_trop_petite_dessine_le_cadre_et_pas_de_contenu() {
        let theme = AirTheme::turbo_vision();
        let size = AirSize::new(2, 2);
        let mut surface = AirSurface::new(size);
        let view = Spy::new(1, AirRectangle::new(AirPoint::ORIGIN, size), '#');
        {
            let mut painter = AirPainter::new(&mut surface);
            draw_view(
                &view,
                &mut painter,
                &AirDrawContext::new(&theme, &AirCommandSet::new()),
            );
        }
        assert_eq!(
            rendered(&surface, size),
            vec!["┌┐".to_owned(), "└┘".to_owned()]
        );
    }

    #[test]
    fn une_vue_d_une_seule_cellule_ne_dessine_pas_de_cadre_partiel() {
        let theme = AirTheme::turbo_vision();
        let size = AirSize::new(1, 1);
        let mut surface = AirSurface::new(size);
        let view = Spy::new(1, AirRectangle::new(AirPoint::ORIGIN, size), '#');
        {
            let mut painter = AirPainter::new(&mut surface);
            draw_view(
                &view,
                &mut painter,
                &AirDrawContext::new(&theme, &AirCommandSet::new()),
            );
        }
        assert_eq!(rendered(&surface, size), vec![" ".to_owned()]);
    }

    #[test]
    fn les_enfants_sont_places_dans_la_zone_de_contenu() {
        // Un enfant en (0, 0) se trouve juste à l'intérieur de la bordure, pas dessus.
        let theme = AirTheme::turbo_vision();
        let size = AirSize::new(6, 4);
        let mut surface = AirSurface::new(size);
        let mut group = AirGroup::new(state(1, AirRectangle::new(AirPoint::ORIGIN, size)));
        group.insert(Box::new(
            Spy::new(2, AirRectangle::from_parts(0, 0, 2, 1), 'x').without_border(),
        ));
        {
            let mut painter = AirPainter::new(&mut surface);
            draw_view(
                &group,
                &mut painter,
                &AirDrawContext::new(&theme, &AirCommandSet::new()),
            );
        }
        let lines = rendered(&surface, size);
        assert_eq!(lines.get(1).map(String::as_str), Some("│xx  │"));
    }

    #[test]
    fn un_enfant_ne_peut_pas_sortir_de_son_proprietaire() {
        let theme = AirTheme::turbo_vision();
        let size = AirSize::new(6, 4);
        let mut surface = AirSurface::new(size);
        let mut group = AirGroup::new(state(1, AirRectangle::new(AirPoint::ORIGIN, size)));
        // Un enfant plus large que la zone de contenu.
        group.insert(Box::new(
            Spy::new(2, AirRectangle::from_parts(0, 0, 20, 1), 'x').without_border(),
        ));
        {
            let mut painter = AirPainter::new(&mut surface);
            draw_view(
                &group,
                &mut painter,
                &AirDrawContext::new(&theme, &AirCommandSet::new()),
            );
        }
        let lines = rendered(&surface, size);
        assert_eq!(lines.get(1).map(String::as_str), Some("│xxxx│"));
    }

    #[test]
    fn un_titre_est_centre_sur_le_bord_superieur() {
        struct Titled(AirViewState);
        impl AirView for Titled {
            fn state(&self) -> &AirViewState {
                &self.0
            }
            fn state_mut(&mut self) -> &mut AirViewState {
                &mut self.0
            }
            fn draw_content(&self, _painter: &mut AirPainter<'_>, _context: &AirDrawContext<'_>) {}
            fn title(&self) -> Option<&str> {
                Some("Air")
            }
        }
        let theme = AirTheme::turbo_vision();
        let size = AirSize::new(12, 3);
        let mut surface = AirSurface::new(size);
        let view = Titled(state(1, AirRectangle::new(AirPoint::ORIGIN, size)));
        {
            let mut painter = AirPainter::new(&mut surface);
            draw_view(
                &view,
                &mut painter,
                &AirDrawContext::new(&theme, &AirCommandSet::new()),
            );
        }
        let lines = rendered(&surface, size);
        assert_eq!(lines.first().map(String::as_str), Some("┌── Air ───┐"));
    }

    #[test]
    fn les_glyphes_de_cadre_suivent_le_style() {
        let theme = AirTheme::turbo_vision();
        assert_eq!(frame_glyphs(&theme, AirBorderStyle::Single).top_left, '┌');
        assert_eq!(frame_glyphs(&theme, AirBorderStyle::Double).top_left, '╔');
    }

    #[test]
    fn un_cadre_double_est_dessine_quand_le_style_le_demande() {
        let theme = AirTheme::turbo_vision();
        let size = AirSize::new(4, 3);
        let mut surface = AirSurface::new(size);
        let mut view = Spy::new(1, AirRectangle::new(AirPoint::ORIGIN, size), ' ');
        view.state.set_border_style(AirBorderStyle::Double);
        {
            let mut painter = AirPainter::new(&mut surface);
            draw_view(
                &view,
                &mut painter,
                &AirDrawContext::new(&theme, &AirCommandSet::new()),
            );
        }
        let lines = rendered(&surface, size);
        assert_eq!(lines.first().map(String::as_str), Some("╔══╗"));
    }

    // --------------------------------------------------- distribution

    fn dispatch(view: &mut dyn AirView, event: &mut AirEvent) -> AirOutcome {
        let mut commands = AirCommandSet::new();
        let mut context = AirContext {
            commands: &mut commands,
        };
        dispatch_event(view, event, &mut context)
    }

    fn key_event_in_state(state: AirKeyState) -> AirEvent {
        AirEvent::new(AirInputEvent::Key(AirKeyEvent {
            logical: AirKey::Down,
            modifiers: AirModifiers::NONE,
            state,
            physical: None,
            base_layout: None,
        }))
    }

    #[test]
    fn un_relachement_de_touche_n_entre_pas_dans_l_arbre() {
        // Ni la vue, ni ses enfants : le filtre est avant la descente. La vue espionne
        // consomme tout ce qu'elle reçoit, donc son silence est la preuve.
        let mut group = AirGroup::new(state(1, AirRectangle::from_parts(0, 0, 10, 10)));
        let mut child = Spy::new(2, AirRectangle::from_parts(0, 0, 4, 4), '.');
        child.state.set_focused(true);
        child.consumes = true;
        group.insert(Box::new(child));

        let mut event = key_event_in_state(AirKeyState::Release);
        assert_eq!(dispatch(&mut group, &mut event), AirOutcome::Ignored);
        assert!(!event.is_consumed(), "le relâchement a été consommé");
    }

    #[test]
    fn une_repetition_de_touche_entre_dans_l_arbre() {
        // Le pendant du précédent. Maintenir une flèche doit répéter le déplacement :
        // écarter « tout ce qui n'est pas un enfoncement » serait le défaut symétrique.
        let mut group = AirGroup::new(state(1, AirRectangle::from_parts(0, 0, 10, 10)));
        let mut child = Spy::new(2, AirRectangle::from_parts(0, 0, 4, 4), '.');
        child.state.set_focused(true);
        child.consumes = true;
        group.insert(Box::new(child));

        let mut event = key_event_in_state(AirKeyState::Repeat);
        assert_eq!(dispatch(&mut group, &mut event), AirOutcome::Consumed);
        assert!(
            event.is_consumed(),
            "la répétition n'est pas parvenue à la vue"
        );
    }

    #[test]
    fn une_vue_invisible_ou_desactivee_ne_recoit_rien() {
        let mut hidden = Spy::new(1, AirRectangle::from_parts(0, 0, 4, 4), '.');
        hidden.state.set_visible(false);
        assert_eq!(
            dispatch(&mut hidden, &mut key_event('a')),
            AirOutcome::Ignored
        );
        assert_eq!(hidden.seen, Vec::new());

        let mut disabled = Spy::new(2, AirRectangle::from_parts(0, 0, 4, 4), '.');
        disabled.state.set_enabled(false);
        assert_eq!(
            dispatch(&mut disabled, &mut key_event('a')),
            AirOutcome::Ignored
        );
        assert_eq!(disabled.seen, Vec::new());
    }

    #[test]
    fn une_vue_voit_l_evenement_avant_ses_enfants() {
        // C'est ce qui permet à un conteneur modal de retenir une frappe.
        let mut group = AirGroup::new(state(1, AirRectangle::from_parts(0, 0, 10, 10)));
        let mut child = Spy::new(2, AirRectangle::from_parts(0, 0, 4, 4), '.');
        child.state.set_focused(true);
        group.insert(Box::new(child));
        let mut event = key_event('a');
        assert_eq!(dispatch(&mut group, &mut event), AirOutcome::Ignored);
        // Le groupe ignore, l'enfant a donc été consulté.
        assert!(!event.is_consumed());
    }

    #[test]
    fn les_trois_phases_sont_parcourues_dans_l_ordre() {
        let mut group = AirGroup::new(state(1, AirRectangle::from_parts(0, 0, 20, 20)));
        let mut before = Spy::new(2, AirRectangle::from_parts(0, 0, 4, 4), '.');
        before.state.set_pre_process(true);
        let mut focused = Spy::new(3, AirRectangle::from_parts(0, 5, 4, 4), '.');
        focused.state.set_focused(true);
        let mut after = Spy::new(4, AirRectangle::from_parts(0, 10, 4, 4), '.');
        after.state.set_post_process(true);
        group.insert(Box::new(before));
        group.insert(Box::new(focused));
        group.insert(Box::new(after));

        let mut event = key_event('a');
        assert_eq!(dispatch(&mut group, &mut event), AirOutcome::Ignored);

        let phases: Vec<AirPhase> = group
            .children()
            .iter()
            .filter_map(|child| {
                let identifier = child.state().identifier().value();
                match identifier {
                    2 => Some(AirPhase::PreProcess),
                    3 => Some(AirPhase::Focused),
                    4 => Some(AirPhase::PostProcess),
                    _ => None,
                }
            })
            .collect();
        assert_eq!(
            phases,
            vec![
                AirPhase::PreProcess,
                AirPhase::Focused,
                AirPhase::PostProcess
            ]
        );
    }

    #[test]
    fn une_vue_de_pre_traitement_peut_retenir_un_evenement_avant_le_focus() {
        // Le cas d'usage réel : une barre de menu ouverte voit `Échap` avant le champ de
        // saisie qui a le focus.
        let mut group = AirGroup::new(state(1, AirRectangle::from_parts(0, 0, 20, 20)));
        let mut menu = Spy::new(2, AirRectangle::from_parts(0, 0, 4, 1), '.');
        menu.state.set_pre_process(true);
        menu.consumes = true;
        let mut field = Spy::new(3, AirRectangle::from_parts(0, 5, 4, 1), '.');
        field.state.set_focused(true);
        group.insert(Box::new(menu));
        group.insert(Box::new(field));

        let mut event = key_event('a');
        assert_eq!(dispatch(&mut group, &mut event), AirOutcome::Consumed);
        assert!(event.is_consumed());
    }

    #[test]
    fn une_commande_rendue_par_un_enfant_remonte() {
        let mut group = AirGroup::new(state(1, AirRectangle::from_parts(0, 0, 20, 20)));
        let mut child = Spy::new(2, AirRectangle::from_parts(0, 0, 4, 4), '.');
        child.state.set_focused(true);
        child.outcome = AirOutcome::Command(SAVE);
        group.insert(Box::new(child));
        assert_eq!(
            dispatch(&mut group, &mut key_event('s')),
            AirOutcome::Command(SAVE)
        );
    }

    #[test]
    fn un_clic_est_traduit_dans_les_coordonnees_de_l_enfant_puis_restaure() {
        struct Recorder {
            state: AirViewState,
            seen: Rc<RefCell<Vec<AirPoint>>>,
        }

        impl AirView for Recorder {
            fn state(&self) -> &AirViewState {
                &self.state
            }
            fn state_mut(&mut self) -> &mut AirViewState {
                &mut self.state
            }
            fn draw_content(&self, _painter: &mut AirPainter<'_>, _context: &AirDrawContext<'_>) {}
            fn handle_event(
                &mut self,
                event: &mut AirEvent,
                _context: &mut AirContext<'_>,
            ) -> AirOutcome {
                if let Some(position) = event.mouse_position() {
                    self.seen.borrow_mut().push(position);
                }
                AirOutcome::Ignored
            }
        }

        // C'est tout l'intérêt du modèle relatif au propriétaire : l'enfant reçoit des
        // coordonnées qui sont les siennes, sans savoir où il se trouve, et la vue parente
        // retrouve les siennes à la remontée.
        let seen = Rc::new(RefCell::new(Vec::new()));
        let mut group = AirGroup::new(state(1, AirRectangle::from_parts(0, 0, 20, 20)));
        group.insert(Box::new(Recorder {
            state: state(2, AirRectangle::from_parts(5, 5, 4, 4)),
            seen: Rc::clone(&seen),
        }));

        // Le groupe est bordé : sa zone de contenu commence en (1, 1), donc l'enfant
        // occupe (6, 6) à (9, 9) dans les coordonnées du groupe.
        let mut event = mouse_event(7, 8);
        dispatch(&mut group, &mut event);

        assert_eq!(
            seen.borrow().first().copied(),
            Some(point(1, 2)),
            "l'enfant doit voir (7,8) - (6,6)"
        );
        assert_eq!(
            event.mouse_position(),
            Some(point(7, 8)),
            "la position doit être restaurée à la remontée"
        );
    }

    #[test]
    fn un_clic_hors_de_tout_enfant_est_ignore() {
        let mut group = AirGroup::new(state(1, AirRectangle::from_parts(0, 0, 20, 20)));
        group.insert(Box::new(Spy::new(
            2,
            AirRectangle::from_parts(5, 5, 2, 2),
            '.',
        )));
        let mut event = mouse_event(1, 1);
        assert_eq!(dispatch(&mut group, &mut event), AirOutcome::Ignored);
    }

    #[test]
    fn un_clic_va_a_la_vue_du_premier_plan() {
        // Deux enfants superposés : c'est celui du dessus qui reçoit, comme à l'écran.
        let mut group = AirGroup::new(state(1, AirRectangle::from_parts(0, 0, 20, 20)));
        let mut behind = Spy::new(2, AirRectangle::from_parts(0, 0, 10, 10), '.');
        behind.consumes = true;
        let mut front = Spy::new(3, AirRectangle::from_parts(0, 0, 10, 10), '.');
        front.outcome = AirOutcome::Command(SAVE);
        group.insert(Box::new(behind));
        group.insert(Box::new(front));
        let mut event = mouse_event(3, 3);
        // Si l'arrière avait reçu le clic, il l'aurait consommé.
        assert_eq!(dispatch(&mut group, &mut event), AirOutcome::Command(SAVE));
    }

    #[test]
    fn un_clic_sur_une_vue_sans_interieur_est_ignore() {
        let mut group = AirGroup::new(state(1, AirRectangle::from_parts(0, 0, 2, 2)));
        group.insert(Box::new(Spy::new(
            2,
            AirRectangle::from_parts(0, 0, 1, 1),
            '.',
        )));
        assert_eq!(
            dispatch(&mut group, &mut mouse_event(0, 0)),
            AirOutcome::Ignored
        );
    }

    #[test]
    fn un_enfant_invisible_ne_recoit_pas_les_clics() {
        let mut group = AirGroup::new(state(1, AirRectangle::from_parts(0, 0, 20, 20)));
        let mut hidden = Spy::new(2, AirRectangle::from_parts(0, 0, 10, 10), '.');
        hidden.state.set_visible(false);
        hidden.consumes = true;
        group.insert(Box::new(hidden));
        assert_eq!(
            dispatch(&mut group, &mut mouse_event(3, 3)),
            AirOutcome::Ignored
        );
    }

    #[test]
    fn un_evenement_sans_position_ne_route_pas_par_la_position() {
        // Un clavier en phase de focus sans enfant focalisé n'atteint personne.
        let mut group = AirGroup::new(state(1, AirRectangle::from_parts(0, 0, 10, 10)));
        group.insert(Box::new(Spy::new(
            2,
            AirRectangle::from_parts(0, 0, 4, 4),
            '.',
        )));
        assert_eq!(
            dispatch(&mut group, &mut key_event('a')),
            AirOutcome::Ignored
        );
    }

    #[test]
    fn le_peintre_et_le_theme_passent_jusqu_aux_petits_enfants() {
        let theme = AirTheme::turbo_vision();
        let size = AirSize::new(10, 6);
        let mut surface = AirSurface::new(size);
        let mut outer = AirGroup::new(state(1, AirRectangle::new(AirPoint::ORIGIN, size)));
        let mut inner = AirGroup::new(state(2, AirRectangle::from_parts(0, 0, 6, 4)));
        inner.insert(Box::new(
            Spy::new(3, AirRectangle::from_parts(0, 0, 2, 1), 'z').without_border(),
        ));
        outer.insert(Box::new(inner));
        {
            let mut painter = AirPainter::new(&mut surface);
            draw_view(
                &outer,
                &mut painter,
                &AirDrawContext::new(&theme, &AirCommandSet::new()),
            );
        }
        // Bordure externe (1,1), bordure interne (2,2), enfant en (2,2).
        assert_eq!(
            surface.cell(point(2, 2)).map(|found| found.character),
            Some('z')
        );
    }

    #[test]
    fn les_attributs_de_dessin_ne_fuient_pas_entre_vues() {
        let theme = AirTheme::turbo_vision();
        let size = AirSize::new(6, 3);
        let mut surface = AirSurface::new(size);
        let mut group = AirGroup::new(state(1, AirRectangle::new(AirPoint::ORIGIN, size)));
        group.state_mut().set_border(false);
        group.insert(Box::new(
            Spy::new(2, AirRectangle::from_parts(0, 0, 3, 1), 'a').without_border(),
        ));
        {
            let mut painter = AirPainter::new(&mut surface);
            draw_view(
                &group,
                &mut painter,
                &AirDrawContext::new(&theme, &AirCommandSet::new()),
            );
            let mut direct = AirPainter::new(&mut surface);
            direct.draw_text(
                point(3, 0),
                "b",
                AirColor::RED,
                AirColor::CYAN,
                AirAttributes::NONE,
            );
        }
        assert_eq!(
            surface.cell(point(0, 0)).map(|found| found.foreground),
            Some(AirColor::WHITE)
        );
        assert_eq!(
            surface.cell(point(3, 0)).map(|found| found.foreground),
            Some(AirColor::RED)
        );
    }
}
