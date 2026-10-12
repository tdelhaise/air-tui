// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! La vue d'édition — le reflet d'un [`AirTextBuffer`] dans une fenêtre.
//!
//! # Elle ne tient pas le modèle
//!
//! C'est le point entier de l'ADR-007. Cette vue ne porte **aucune** référence vers le
//! tampon : ni `&`, ni `Rc`, ni pointeur. Elle en garde un **reflet** — les quelques
//! lignes visibles et la position du curseur — que le contrôleur lui pousse par
//! [`AirEditorView::synchronize`] à l'étape 3 du tour de boucle.
//!
//! Deux conséquences qui valent le détour :
//!
//! - une frappe ne peut pas muter le tampon depuis la vue. La vue **accumule des
//!   intentions** ([`AirTextAction`]) que le contrôleur relève par
//!   [`AirEditorView::drain_actions`] et applique au modèle. La réentrance est impossible,
//!   et il n'y a jamais deux emprunts mutables vivants ;
//! - deux vues du même tampon défilent indépendamment, puisque le défilement est ici et
//!   non dans le modèle.
//!
//! # Les compteurs de génération font le travail
//!
//! [`AirEditorView::synchronize`] est appelée à chaque tour de boucle, et la plupart du
//! temps elle ne fait **rien** : elle compare les deux générations du tampon à celles
//! qu'elle a déjà reflétées, et sort. C'est une comparaison de deux entiers, pas un
//! parcours du texte — c'est le mécanisme 2 de l'ADR-007, et c'est ce qui permet d'appeler
//! la synchronisation sans se demander si c'est utile.
//!
//! # Le défilement appartient à la vue, et le curseur au modèle
//!
//! Le curseur revient dans la fenêtre quand il **bouge** ; la molette et l'ascenseur
//! déplacent la fenêtre sans le ramener. Les deux gestes sont distincts, et confondre les
//! deux donnerait soit un curseur qu'on perd en tapant, soit une molette qui ne sert à
//! rien.
//!
//! # La sélection, et où elle vit
//!
//! Faite au jalon M7d. La sélection est **dans le tampon**, pas ici : le texte est au
//! modèle, et son étendue sélectionnée en fait partie. La vue n'en garde que ce qu'elle
//! dessine — l'intervalle de colonnes de chaque ligne visible, capturé au moment du
//! reflet, seul instant où elle voit le tampon. Les bornes sont des décalages en octets,
//! et les convertir en colonnes demande le début de la ligne, que le reflet ne garde pas.
//!
//! Le copier-coller **sort par le canal des commandes** et non par celui des intentions :
//! le presse-papiers n'appartient pas à cette vue, et c'est le contrôleur qui le tient. La
//! recherche — `Ctrl`+`F`, et `Ctrl`+`L` pour la suivante — sort par le même canal et pour
//! la même raison : il faut poser une question à l'utilisateur, ce qu'une vue d'édition ne
//! sait pas faire (ADR-016).
//!
//! Le double clic sélectionne le segment sous le pointeur, le triple la ligne entière.
//! La vue ne détecte **pas** le geste : la boucle d'application le synthétise et le lui
//! livre après l'enfoncement, donc le curseur est déjà posé quand la sélection s'étend.
//!
//! # Ce qui n'est pas là
//!
//! La **tabulation** : insérer un `\t` demande de choisir des taquets et de les appliquer
//! au rendu comme au calcul de colonne. Tant que ce choix n'est pas fait, `Tab` n'est pas
//! consommée — elle reste disponible pour le parcours du focus, plutôt que d'insérer un
//! caractère dont l'affichage serait faux.

use crate::command::AirCommand;
use crate::geometry::{AirColumn, AirPoint, AirRectangle, AirRow, AirSize};
use crate::input::event::{AirInputEvent, AirKey, AirMouseButton, AirMouseEventKind};
use crate::model::{AirGeneration, AirTextAction, AirTextBuffer, AirTextMotion, AirTextPosition};
use crate::theme::{AirColorPair, AirRole};
use crate::unicode::{grapheme_width, graphemes, text_width, width_between};
use crate::view::painter::AirPainter;
use crate::view::{AirContext, AirDrawContext, AirEvent, AirOutcome, AirView, AirViewState};

/// Une ligne telle qu'elle sera posée à l'écran.
#[derive(Debug, Clone, PartialEq, Eq)]
struct VisibleLine {
    /// La colonne de contenu où poser le texte.
    ///
    /// Non nulle quand le bord gauche coupe une grappe large : la place de sa moitié
    /// visible reste vide, parce qu'on ne dessine pas la moitié d'un glyphe.
    column: u16,
    /// Le texte à poser, déjà borné à la largeur de la fenêtre.
    text: String,
    /// L'intervalle de colonnes d'affichage sélectionné sur cette ligne.
    ///
    /// En colonnes **absolues** dans la ligne complète, et non dans la fenêtre : la
    /// conversion est au dessin, qui connaît déjà le défilement horizontal. Calculé au
    /// moment du reflet, seul instant où la vue voit le tampon — les bornes de la
    /// sélection sont des décalages en octets, et les convertir en colonnes demande le
    /// début de la ligne, que le reflet ne garde pas.
    selected: Option<(usize, usize)>,
}

impl VisibleLine {
    /// Une ligne dont rien n'est visible.
    const EMPTY: Self = Self {
        column: 0,
        text: String::new(),
        selected: None,
    };
}

/// La tranche d'une ligne visible dans une fenêtre horizontale.
///
/// Rend la colonne de contenu où poser le texte, et le texte. Une grappe que le bord
/// gauche coupe n'est pas rendue, et une grappe qui ne tient pas à droite est écartée par
/// la même règle.
fn visible_slice(line: &str, left_column: usize, columns: usize) -> VisibleLine {
    let mut offset: usize = 0;
    let mut walked: usize = 0;
    let mut start: Option<(usize, usize)> = None;
    for cluster in graphemes(line) {
        if walked >= left_column {
            start = Some((offset, walked.saturating_sub(left_column)));
            break;
        }
        walked = walked.saturating_add(grapheme_width(cluster));
        offset = offset.saturating_add(cluster.len());
    }
    let Some((begin, padding)) = start else {
        // La ligne finit avant le bord gauche : rien n'en est visible.
        return VisibleLine::EMPTY;
    };
    let Some(tail) = line.get(begin..) else {
        return VisibleLine::EMPTY;
    };
    let mut end: usize = begin;
    let mut used: usize = padding;
    for cluster in graphemes(tail) {
        let next = used.saturating_add(grapheme_width(cluster));
        if next > columns {
            break;
        }
        used = next;
        end = end.saturating_add(cluster.len());
    }
    VisibleLine {
        column: u16::try_from(padding).unwrap_or(u16::MAX),
        text: line.get(begin..end).unwrap_or("").to_owned(),
        selected: None,
    }
}

/// L'intervalle de colonnes d'affichage qu'une sélection couvre sur une ligne.
///
/// Rend `None` quand la ligne est hors de la sélection, ou quand l'intervalle est vide —
/// ce qui arrive sur la dernière ligne d'une sélection qui s'arrête à son tout début.
fn selected_columns(
    line: &str,
    index: usize,
    start: AirTextPosition,
    end: AirTextPosition,
) -> Option<(usize, usize)> {
    if index < start.line || index > end.line {
        return None;
    }
    let first = if index == start.line {
        width_between(line, 0, start.offset)
    } else {
        0
    };
    let last = if index == end.line {
        width_between(line, 0, end.offset)
    } else {
        // Une colonne de plus que la ligne : c'est le saut de ligne, et il est bien dans
        // la sélection. Sans lui, une ligne vide prise au milieu d'une sélection ne
        // porterait aucune marque et le surlignage paraîtrait troué.
        text_width(line).saturating_add(1)
    };
    if last <= first {
        return None;
    }
    Some((first, last))
}

/// La vue d'édition d'un tampon de texte.
#[derive(Debug)]
pub struct AirEditorView {
    /// L'état commun.
    state: AirViewState,
    /// Les lignes visibles, telles qu'elles seront posées.
    visible_lines: Vec<VisibleLine>,
    /// La première ligne du tampon affichée.
    top_line: usize,
    /// La première colonne d'affichage affichée.
    left_column: usize,
    /// Le nombre total de lignes du tampon, au dernier reflet.
    total_lines: usize,
    /// La position du curseur dans le tampon, au dernier reflet.
    cursor_position: AirTextPosition,
    /// La colonne d'affichage du curseur, au dernier reflet.
    cursor_display_column: usize,
    /// La génération de texte déjà reflétée.
    seen_text_generation: AirGeneration,
    /// La génération de curseur déjà reflétée.
    seen_cursor_generation: AirGeneration,
    /// La génération de sélection déjà reflétée.
    seen_selection_generation: AirGeneration,
    /// La dimension de contenu du dernier reflet, ou `None` si rien n'a été reflété.
    synchronized_size: Option<AirSize>,
    /// Vrai quand le défilement a changé sans que le tampon bouge.
    scrolled: bool,
    /// Les intentions produites par les gestes, en attente du contrôleur.
    pending_actions: Vec<AirTextAction>,
}

impl AirEditorView {
    /// Une vue d'édition vide, sans bordure.
    ///
    /// La bordure est retirée : la vue est le contenu d'une fenêtre, qui porte déjà la
    /// sienne, et deux cadres emboîtés mangeraient deux lignes de texte.
    #[must_use]
    pub fn new(mut state: AirViewState) -> Self {
        state.set_border(false);
        Self {
            state,
            visible_lines: Vec::new(),
            top_line: 0,
            left_column: 0,
            total_lines: 1,
            cursor_position: AirTextPosition::default(),
            cursor_display_column: 0,
            seen_text_generation: AirGeneration::new(),
            seen_cursor_generation: AirGeneration::new(),
            seen_selection_generation: AirGeneration::new(),
            synchronized_size: None,
            scrolled: false,
            pending_actions: Vec::new(),
        }
    }

    /// La première ligne affichée.
    #[must_use]
    pub const fn top_line(&self) -> usize {
        self.top_line
    }

    /// La première colonne d'affichage affichée.
    #[must_use]
    pub const fn left_column(&self) -> usize {
        self.left_column
    }

    /// Le nombre total de lignes du tampon, au dernier reflet.
    #[must_use]
    pub const fn total_lines(&self) -> usize {
        self.total_lines
    }

    /// La dimension de la zone de contenu.
    #[must_use]
    pub fn content_size(&self) -> AirSize {
        self.state
            .content_bounds()
            .map_or(AirSize::new(0, 0), |bounds| bounds.size)
    }

    /// Le nombre de lignes affichables.
    #[must_use]
    pub fn visible_rows(&self) -> usize {
        usize::from(self.content_size().rows)
    }

    /// Le nombre de colonnes affichables.
    #[must_use]
    pub fn visible_columns(&self) -> usize {
        usize::from(self.content_size().columns)
    }

    /// Déplace la fenêtre sur une ligne, **sans** toucher au curseur.
    ///
    /// C'est ce que font la molette et l'ascenseur. Le curseur n'est ramené dans la
    /// fenêtre que lorsqu'il bouge — sinon la molette ne servirait à rien.
    pub fn scroll_to_line(&mut self, line: usize) {
        let bounded = line.min(self.total_lines.saturating_sub(1));
        if bounded != self.top_line {
            self.top_line = bounded;
            self.scrolled = true;
        }
    }

    /// Déplace la fenêtre sur une colonne, sans toucher au curseur.
    pub fn scroll_to_column(&mut self, column: usize) {
        if column != self.left_column {
            self.left_column = column;
            self.scrolled = true;
        }
    }

    /// Les intentions accumulées, retirées de la vue.
    ///
    /// La capacité du tampon d'intentions est conservée : le chemin chaud n'alloue pas.
    pub fn drain_actions(&mut self) -> std::vec::Drain<'_, AirTextAction> {
        self.pending_actions.drain(..)
    }

    /// Vrai si des intentions attendent d'être relevées.
    #[must_use]
    pub fn has_pending_actions(&self) -> bool {
        !self.pending_actions.is_empty()
    }

    /// Met le reflet à jour depuis le tampon.
    ///
    /// Ne fait rien quand ni le texte, ni le curseur, ni la dimension, ni le défilement
    /// n'ont changé : quatre comparaisons, aucune allocation, aucun parcours de texte.
    pub fn synchronize(&mut self, buffer: &AirTextBuffer) {
        let size = self.content_size();
        let cursor_moved = self.seen_cursor_generation != buffer.cursor_generation();
        let text_changed = self.seen_text_generation != buffer.text_generation();
        let selection_changed = self.seen_selection_generation != buffer.selection_generation();
        let resized = self.synchronized_size != Some(size);
        if !cursor_moved && !text_changed && !selection_changed && !resized && !self.scrolled {
            return;
        }

        self.cursor_position = buffer.cursor();
        self.cursor_display_column = buffer.cursor_column();
        self.total_lines = buffer.line_count();
        if cursor_moved || resized {
            self.reveal_cursor(size);
        }
        self.copy_visible_lines(buffer, size);

        self.seen_text_generation = buffer.text_generation();
        self.seen_cursor_generation = buffer.cursor_generation();
        self.seen_selection_generation = buffer.selection_generation();
        self.synchronized_size = Some(size);
        self.scrolled = false;
    }

    /// Ramène la fenêtre sur le curseur.
    fn reveal_cursor(&mut self, size: AirSize) {
        let rows = usize::from(size.rows);
        let columns = usize::from(size.columns);
        if rows == 0 || columns == 0 {
            return;
        }
        let line = self.cursor_position.line;
        if line < self.top_line {
            self.top_line = line;
        } else if line >= self.top_line.saturating_add(rows) {
            self.top_line = line.saturating_sub(rows.saturating_sub(1));
        }
        let column = self.cursor_display_column;
        if column < self.left_column {
            self.left_column = column;
        } else if column >= self.left_column.saturating_add(columns) {
            self.left_column = column.saturating_sub(columns.saturating_sub(1));
        }
    }

    /// Copie les lignes visibles du tampon, bornées à la fenêtre.
    ///
    /// Ce que la vue garde est borné par la **dimension de la fenêtre**, pas par la
    /// longueur des lignes : une ligne d'un mégaoctet ne coûte que ses colonnes visibles.
    fn copy_visible_lines(&mut self, buffer: &AirTextBuffer, size: AirSize) {
        let rows = usize::from(size.rows);
        let columns = usize::from(size.columns);
        self.visible_lines.clear();
        if rows == 0 || columns == 0 {
            return;
        }
        let selection = buffer.selection();
        for row in 0..rows {
            let index = self.top_line.saturating_add(row);
            // Au delà de la dernière ligne : une ligne vide, et non une absence. Le
            // dessin la remplit comme les autres, donc le fond reste uniforme.
            let mut visible = match buffer.line(index) {
                Some(line) => visible_slice(line, self.left_column, columns),
                None => VisibleLine::EMPTY,
            };
            if let Some((start, end)) = selection {
                let line = buffer.line(index).unwrap_or("");
                visible.selected = selected_columns(line, index, start, end);
            }
            self.visible_lines.push(visible);
        }
    }

    /// Le déplacement que désigne une position de souris, dans les coordonnées du contenu.
    fn motion_at(&self, position: AirPoint) -> AirTextMotion {
        AirTextMotion::ToColumn {
            line: self
                .top_line
                .saturating_add(usize::from(position.row.value())),
            column: self
                .left_column
                .saturating_add(usize::from(position.column.value())),
        }
    }

    /// Dessine une ligne visible, surlignage compris.
    ///
    /// Le cas sans sélection garde un **seul** appel de pose : c'est le cas courant, et le
    /// dessin grappe par grappe n'a de raison d'être que là où la couleur change en cours
    /// de ligne.
    fn draw_line(
        &self,
        painter: &mut AirPainter<'_>,
        row: u16,
        line: &VisibleLine,
        colors: AirColorPair,
        marked: AirColorPair,
    ) {
        let Some((first, last)) = line.selected else {
            if !line.text.is_empty() {
                painter.draw_text_with_colors(
                    AirPoint::new(AirColumn::new(line.column), AirRow::new(row)),
                    &line.text,
                    colors,
                );
            }
            return;
        };
        let size = painter.size();
        // Le fond d'abord, sur tout l'intervalle visible : c'est lui qui montre le saut de
        // ligne sélectionné et les lignes vides prises au milieu d'une sélection, qui ne
        // portent aucun caractère à colorer.
        let window_first = first.saturating_sub(self.left_column);
        let window_last = last
            .saturating_sub(self.left_column)
            .min(usize::from(size.columns));
        if last > self.left_column && window_last > window_first {
            let width = window_last.saturating_sub(window_first);
            painter.fill_rectangle(
                AirRectangle::from_parts(
                    u16::try_from(window_first).unwrap_or(u16::MAX),
                    row,
                    u16::try_from(width).unwrap_or(u16::MAX),
                    1,
                ),
                marked.cell(' '),
            );
        }
        // Puis le texte, grappe par grappe : la couleur change en cours de ligne, et la
        // colonne d'une grappe large avance de deux.
        let mut column = self.left_column.saturating_add(usize::from(line.column));
        for cluster in graphemes(&line.text) {
            let inside = column >= first && column < last;
            let pair = if inside { marked } else { colors };
            let at = column.saturating_sub(self.left_column);
            if let Ok(at) = u16::try_from(at) {
                painter.draw_text_with_colors(
                    AirPoint::new(AirColumn::new(at), AirRow::new(row)),
                    cluster,
                    pair,
                );
            }
            column = column.saturating_add(grapheme_width(cluster));
        }
    }

    /// La commande qu'une frappe demande, s'il y en a une.
    ///
    /// Cinq gestes sortent par le canal des **commandes** et non par celui des
    /// intentions, et ils ont une chose en commun : ce qu'il leur faut n'appartient pas à
    /// cette vue. Le presse-papiers est au contrôleur, et une question posée à
    /// l'utilisateur l'est aussi.
    ///
    /// `Ctrl`+`C`, `Ctrl`+`X` et `Ctrl`+`V` sont les gestes que tout le monde connaît.
    /// Turbo Vision employait `Ctrl`+`Inser` et `Maj`+`Suppr`, que les terminaux ne
    /// transmettent pas tous — ils demandent le protocole Kitty pour être distingués de
    /// `Inser` et `Suppr` nus.
    ///
    /// `Ctrl`+`F` ouvre la recherche et `Ctrl`+`L` reprend la dernière, qui est le geste
    /// de Borland — `Ctrl`+`L` y répète la recherche. `F3`, que Turbo Vision emploie pour
    /// la même chose dans son IDE, est pris par « Ouvrir » dans `redit` ; `Ctrl`+`G` et
    /// `Ctrl`+`N` sont les gestes d'autres éditeurs, et aucun des deux n'a de source dans
    /// ce que ce dépôt transcrit.
    fn controller_command(key: &crate::input::event::AirKeyEvent) -> Option<AirCommand> {
        if !key.modifiers.control {
            return None;
        }
        match key.logical {
            AirKey::Character('c' | 'C') => Some(crate::command::COPY),
            AirKey::Character('x' | 'X') => Some(crate::command::CUT),
            AirKey::Character('v' | 'V') => Some(crate::command::PASTE),
            AirKey::Character('f' | 'F') => Some(crate::command::FIND),
            AirKey::Character('l' | 'L') => Some(crate::command::FIND_NEXT),
            _ => None,
        }
    }

    /// Le déplacement correspondant à une touche, s'il y en a un.
    ///
    /// Séparé de l'édition parce que **`Maj` ne décide qu'une chose** : étendre la
    /// sélection ou l'effacer. Une table unique qui mêlerait les deux obligerait à
    /// répéter cette décision sur chaque flèche.
    const fn motion_for_key(key: AirKey, control: bool, page: usize) -> Option<AirTextMotion> {
        match key {
            AirKey::Left if control => Some(AirTextMotion::PreviousWord),
            AirKey::Right if control => Some(AirTextMotion::NextWord),
            AirKey::Left => Some(AirTextMotion::Left),
            AirKey::Right => Some(AirTextMotion::Right),
            AirKey::Up => Some(AirTextMotion::Up),
            AirKey::Down => Some(AirTextMotion::Down),
            AirKey::Home if control => Some(AirTextMotion::DocumentStart),
            AirKey::End if control => Some(AirTextMotion::DocumentEnd),
            AirKey::Home => Some(AirTextMotion::LineStart),
            AirKey::End => Some(AirTextMotion::LineEnd),
            AirKey::PageUp => Some(AirTextMotion::UpByLines(page)),
            AirKey::PageDown => Some(AirTextMotion::DownByLines(page)),
            _ => None,
        }
    }

    /// L'intention correspondant à une frappe, s'il y en a une.
    fn action_for_key(&self, key: &crate::input::event::AirKeyEvent) -> Option<AirTextAction> {
        let control = key.modifiers.control;
        let page = self.visible_rows().max(1);
        if let Some(motion) = Self::motion_for_key(key.logical, control, page) {
            return Some(if key.modifiers.shift {
                AirTextAction::select_by(motion)
            } else {
                AirTextAction::move_by(motion)
            });
        }
        match key.logical {
            // Une frappe avec `Alt` appartient aux raccourcis ; la barre de menu en décide.
            AirKey::Character(_) if key.modifiers.alternate => None,
            // `Ctrl`+`Y` supprime la ligne : c'est le geste de Turbo Vision, et il n'a pas
            // d'autre équivalent au clavier.
            AirKey::Character('y' | 'Y') if control => Some(AirTextAction::DeleteLine),
            AirKey::Character('a' | 'A') if control => Some(AirTextAction::SelectAll),
            // L'annulation sort par le canal des **intentions** et non des commandes :
            // l'historique appartient au modèle, que cette vue adresse déjà (ADR-014).
            AirKey::Character('z' | 'Z') if control => Some(AirTextAction::Undo),
            // `Ctrl`+`R` et non `Ctrl`+`Y`, pris par la suppression de ligne de Turbo
            // Vision, ni `Ctrl`+`Maj`+`Z`, qu'un terminal sans protocole Kitty ne
            // distingue pas de `Ctrl`+`Z` : le rétablissement marcherait sur certains
            // terminaux et pas sur d'autres, ce qui est pire que de ne pas l'offrir.
            AirKey::Character('r' | 'R') if control => Some(AirTextAction::Redo),
            AirKey::Character(_) if control => None,
            AirKey::Character(character) => Some(AirTextAction::InsertCharacter(character)),
            AirKey::Enter => Some(AirTextAction::InsertLineBreak),
            AirKey::Backspace => Some(AirTextAction::DeleteBackward),
            AirKey::Delete => Some(AirTextAction::DeleteForward),
            _ => None,
        }
    }
}

impl AirView for AirEditorView {
    fn state(&self) -> &AirViewState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut AirViewState {
        &mut self.state
    }

    fn draw_content(&self, painter: &mut AirPainter<'_>, context: &AirDrawContext<'_>) {
        let colors = context.theme.palette.color_pair(AirRole::WindowText);
        let marked = context.theme.palette.color_pair(AirRole::EditorSelected);
        let size = painter.size();
        painter.fill_rectangle(AirRectangle::new(AirPoint::ORIGIN, size), colors.cell(' '));
        for (row, line) in self.visible_lines.iter().enumerate() {
            let Ok(row) = u16::try_from(row) else {
                break;
            };
            if row >= size.rows {
                break;
            }
            self.draw_line(painter, row, line, colors, marked);
        }
    }

    fn cursor(&self) -> Option<AirPoint> {
        if !self.state.is_enabled() {
            return None;
        }
        let size = self.content_size();
        let row = self.cursor_position.line.checked_sub(self.top_line)?;
        let column = self.cursor_display_column.checked_sub(self.left_column)?;
        let row = u16::try_from(row).ok()?;
        let column = u16::try_from(column).ok()?;
        if row >= size.rows || column >= size.columns {
            return None;
        }
        Some(AirPoint::new(AirColumn::new(column), AirRow::new(row)))
    }

    fn handle_event(&mut self, event: &mut AirEvent, context: &mut AirContext<'_>) -> AirOutcome {
        let _ = context;
        if !self.state.is_enabled() {
            return AirOutcome::Ignored;
        }
        let Some(input) = event.input() else {
            return AirOutcome::Ignored;
        };
        match input {
            AirInputEvent::Mouse(mouse) => {
                let position = mouse.position;
                match mouse.kind {
                    AirMouseEventKind::Pressed(AirMouseButton::Left) => {
                        // `Maj` et un clic étendent la sélection depuis son ancre, comme
                        // `Maj` et une flèche : c'est le même geste par un autre organe.
                        let motion = self.motion_at(position);
                        self.pending_actions.push(if mouse.modifiers.shift {
                            AirTextAction::select_by(motion)
                        } else {
                            AirTextAction::move_by(motion)
                        });
                    }
                    // Le glisser n'a besoin d'aucun état dans la vue : l'appui a effacé la
                    // sélection, donc le premier déplacement pose l'ancre là où le bouton
                    // est tombé. Garder le point d'appui ici le dédoublerait avec l'ancre
                    // que le tampon tient déjà.
                    AirMouseEventKind::Moved(Some(AirMouseButton::Left)) => {
                        let motion = self.motion_at(position);
                        self.pending_actions.push(AirTextAction::select_by(motion));
                    }
                    // Le double clic arrive **après** son enfoncement, qui a déjà poussé
                    // le déplacement : les deux intentions sont appliquées dans l'ordre au
                    // même tour, donc le curseur est posé avant que le segment soit pris.
                    // Sans cet ordre, la sélection porterait sur le mot d'avant.
                    AirMouseEventKind::DoubleClicked(AirMouseButton::Left) => {
                        self.pending_actions.push(AirTextAction::SelectSegment);
                    }
                    AirMouseEventKind::TripleClicked(AirMouseButton::Left) => {
                        self.pending_actions.push(AirTextAction::SelectLine);
                    }
                    AirMouseEventKind::WheelUp => {
                        self.scroll_to_line(self.top_line.saturating_sub(3));
                    }
                    AirMouseEventKind::WheelDown => {
                        self.scroll_to_line(self.top_line.saturating_add(3));
                    }
                    _ => return AirOutcome::Ignored,
                }
                event.consume();
                AirOutcome::Consumed
            }
            AirInputEvent::Key(key) => {
                // Les gestes dont l'effet n'appartient pas à cette vue sortent par le
                // canal des commandes et non par celui des intentions : le presse-papiers
                // est au contrôleur, et la question d'une recherche l'est aussi.
                if let Some(command) = Self::controller_command(key) {
                    event.consume();
                    return AirOutcome::Command(command);
                }
                let Some(action) = self.action_for_key(key) else {
                    return AirOutcome::Ignored;
                };
                self.pending_actions.push(action);
                event.consume();
                AirOutcome::Consumed
            }
            // Le collage encadré du terminal. Le décodeur le produit depuis le jalon M1 et
            // personne ne le consommait : la séquence était décodée, puis jetée.
            AirInputEvent::Paste(pasted) => {
                self.pending_actions
                    .push(AirTextAction::InsertText(pasted.clone()));
                event.consume();
                AirOutcome::Consumed
            }
            _ => AirOutcome::Ignored,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AirEditorView, visible_slice};
    use crate::cell::AirColor;
    use crate::command::AirCommandSet;
    use crate::geometry::{AirColumn, AirPoint, AirRectangle, AirRow, AirSize};
    use crate::input::event::{
        AirInputEvent, AirKey, AirKeyEvent, AirModifiers, AirMouseButton, AirMouseEvent,
        AirMouseEventKind,
    };
    use crate::model::{AirChangeSet, AirModel, AirTextAction, AirTextBuffer, AirTextPosition};
    use crate::surface::AirSurface;
    use crate::theme::{AirRole, AirTheme};
    use crate::view::painter::AirPainter;
    use crate::view::{
        AirContext, AirDrawContext, AirEvent, AirOutcome, AirView, AirViewId, AirViewState,
        draw_view,
    };

    fn editor(columns: u16, rows: u16) -> AirEditorView {
        let mut view = AirEditorView::new(AirViewState::new(
            AirViewId::FIRST,
            AirRectangle::from_parts(0, 0, columns, rows),
        ));
        view.state_mut().set_focused(true);
        view
    }

    /// Un tour de boucle complet : les intentions de la vue, puis la synchronisation.
    fn turn(view: &mut AirEditorView, buffer: &mut AirTextBuffer) {
        let mut changes = AirChangeSet::new();
        let actions: Vec<AirTextAction> = view.drain_actions().collect();
        for action in actions {
            buffer.apply(action, &mut changes);
        }
        view.synchronize(buffer);
    }

    fn press(view: &mut AirEditorView, key: AirKey) -> AirOutcome {
        press_with(view, key, AirModifiers::default())
    }

    fn press_with(view: &mut AirEditorView, key: AirKey, modifiers: AirModifiers) -> AirOutcome {
        let mut commands = AirCommandSet::new();
        let mut context = AirContext {
            commands: &mut commands,
        };
        let mut event = AirEvent::new(AirInputEvent::Key(AirKeyEvent {
            modifiers,
            ..AirKeyEvent::press(key)
        }));
        view.handle_event(&mut event, &mut context)
    }

    #[test]
    fn controle_z_annule_et_controle_r_retablit() {
        // Les deux sortent par le canal des **intentions** : l'historique appartient au
        // modèle, que cette vue adresse déjà. `Ctrl`+`Y` reste la suppression de ligne de
        // Turbo Vision, et `Ctrl`+`Maj`+`Z` serait indistinguable de `Ctrl`+`Z` sur un
        // terminal sans protocole Kitty (ADR-014).
        let mut view = editor(20, 4);
        let control = AirModifiers::NONE.with_control();
        assert_eq!(
            press_with(&mut view, AirKey::Character('z'), control),
            AirOutcome::Consumed
        );
        assert_eq!(
            press_with(&mut view, AirKey::Character('r'), control),
            AirOutcome::Consumed
        );
        assert_eq!(
            view.drain_actions().collect::<Vec<_>>(),
            vec![AirTextAction::Undo, AirTextAction::Redo]
        );
    }

    #[test]
    fn la_majuscule_de_la_touche_ne_change_pas_l_intention() {
        // `Ctrl`+`Maj`+`Z` arrive en majuscule sur un terminal qui transmet la casse : il
        // ne doit pas tomber dans le bras des frappes ordinaires.
        let mut view = editor(20, 4);
        let control = AirModifiers::NONE.with_control();
        press_with(&mut view, AirKey::Character('Z'), control);
        press_with(&mut view, AirKey::Character('R'), control);
        assert_eq!(
            view.drain_actions().collect::<Vec<_>>(),
            vec![AirTextAction::Undo, AirTextAction::Redo]
        );
    }

    fn mouse(
        view: &mut AirEditorView,
        kind: AirMouseEventKind,
        column: u16,
        row: u16,
    ) -> AirOutcome {
        mouse_with(view, kind, column, row, AirModifiers::default())
    }

    fn mouse_with(
        view: &mut AirEditorView,
        kind: AirMouseEventKind,
        column: u16,
        row: u16,
        modifiers: AirModifiers,
    ) -> AirOutcome {
        let mut commands = AirCommandSet::new();
        let mut context = AirContext {
            commands: &mut commands,
        };
        let mut event = AirEvent::new(AirInputEvent::Mouse(AirMouseEvent {
            position: AirPoint::new(AirColumn::new(column), AirRow::new(row)),
            kind,
            modifiers,
        }));
        view.handle_event(&mut event, &mut context)
    }

    fn render(view: &AirEditorView, size: AirSize) -> String {
        let mut surface = AirSurface::new(size);
        let theme = AirTheme::turbo_vision();
        let commands = AirCommandSet::new();
        {
            let mut painter = AirPainter::new(&mut surface);
            draw_view(view, &mut painter, &AirDrawContext::new(&theme, &commands));
        }
        let mut lines: Vec<String> = Vec::new();
        for row in 0..size.rows {
            let mut line = String::new();
            for column in 0..size.columns {
                let point = AirPoint::new(AirColumn::new(column), AirRow::new(row));
                if let Some(cell) = surface.cell(point) {
                    line.push(cell.character);
                }
            }
            lines.push(line);
        }
        lines.join("\n")
    }

    /// Les fonds de chaque cellule d'une rangée, rendus sous le thème Turbo Vision.
    ///
    /// Le surlignage se vérifie sur la **couleur** et non sur le texte : un essai qui ne
    /// regarderait que les caractères passerait alors que rien n'est surligné.
    fn backgrounds(view: &AirEditorView, size: AirSize, row: u16) -> Vec<Option<AirColor>> {
        let mut surface = AirSurface::new(size);
        let theme = AirTheme::turbo_vision();
        let commands = AirCommandSet::new();
        {
            let mut painter = AirPainter::new(&mut surface);
            draw_view(view, &mut painter, &AirDrawContext::new(&theme, &commands));
        }
        (0..size.columns)
            .map(|column| {
                surface
                    .cell(AirPoint::new(AirColumn::new(column), AirRow::new(row)))
                    .map(|cell| cell.background)
            })
            .collect()
    }

    /// Le fond du texte sélectionné, et celui du texte ordinaire, sous Turbo Vision.
    fn selection_backgrounds() -> (AirColor, AirColor) {
        let theme = AirTheme::turbo_vision();
        (
            theme.palette.color_pair(AirRole::EditorSelected).background,
            theme.palette.color_pair(AirRole::WindowText).background,
        )
    }

    fn cursor_of(view: &AirEditorView, size: AirSize) -> Option<AirPoint> {
        let mut surface = AirSurface::new(size);
        let theme = AirTheme::turbo_vision();
        let commands = AirCommandSet::new();
        let mut painter = AirPainter::new(&mut surface);
        draw_view(view, &mut painter, &AirDrawContext::new(&theme, &commands))
    }

    #[test]
    fn une_vue_d_edition_est_sans_bordure() {
        assert!(!editor(10, 5).state().has_border());
    }

    #[test]
    fn une_vue_neuve_ne_reflete_rien_avant_sa_synchronisation() {
        let view = editor(10, 3);
        assert_eq!(
            render(&view, AirSize::new(10, 3)),
            "          \n          \n          "
        );
    }

    #[test]
    fn la_synchronisation_reflete_les_lignes_visibles() {
        // La dimension et les générations d'un tampon neuf valent zéro comme celles de la
        // vue : sans le drapeau de première synchronisation, cette vue resterait vide.
        let mut view = editor(6, 3);
        let buffer = AirTextBuffer::from_text("une\ndeux\ntrois\nquatre");
        view.synchronize(&buffer);
        assert_eq!(render(&view, AirSize::new(6, 3)), "une   \ndeux  \ntrois ");
    }

    #[test]
    fn une_synchronisation_sans_changement_ne_refait_rien() {
        let mut view = editor(6, 2);
        let buffer = AirTextBuffer::from_text("une\ndeux");
        view.synchronize(&buffer);
        let before = render(&view, AirSize::new(6, 2));
        // Deuxième passage : les générations sont les mêmes, la vue sort tout de suite.
        view.synchronize(&buffer);
        assert_eq!(render(&view, AirSize::new(6, 2)), before);
    }

    #[test]
    fn les_lignes_au_dela_du_texte_sont_vides_et_non_absentes() {
        let mut view = editor(5, 4);
        let buffer = AirTextBuffer::from_text("une");
        view.synchronize(&buffer);
        assert_eq!(
            render(&view, AirSize::new(5, 4)),
            "une  \n     \n     \n     "
        );
    }

    #[test]
    fn une_frappe_devient_une_intention_et_non_une_mutation() {
        // La vue ne peut pas muter le tampon : elle accumule, le contrôleur applique.
        let mut view = editor(10, 2);
        let mut buffer = AirTextBuffer::new();
        assert_eq!(
            press(&mut view, AirKey::Character('a')),
            AirOutcome::Consumed
        );
        assert!(view.has_pending_actions());
        assert_eq!(buffer.line(0), Some(""));
        turn(&mut view, &mut buffer);
        assert_eq!(buffer.line(0), Some("a"));
        assert!(!view.has_pending_actions());
    }

    #[test]
    fn les_touches_d_edition_produisent_leurs_intentions() {
        let mut view = editor(10, 3);
        let mut buffer = AirTextBuffer::from_text("abc");
        view.synchronize(&buffer);
        for key in [
            AirKey::End,
            AirKey::Backspace,
            AirKey::Enter,
            AirKey::Character('z'),
        ] {
            press(&mut view, key);
            turn(&mut view, &mut buffer);
        }
        assert_eq!(buffer.lines(), ["ab", "z"]);
    }

    #[test]
    fn controle_plus_fleche_se_deplace_par_mot() {
        let mut view = editor(20, 2);
        let mut buffer = AirTextBuffer::from_text("le chat noir");
        view.synchronize(&buffer);
        let control = AirModifiers {
            control: true,
            ..AirModifiers::default()
        };
        press_with(&mut view, AirKey::Right, control);
        turn(&mut view, &mut buffer);
        assert_eq!(buffer.cursor().offset, 3);
    }

    #[test]
    fn controle_plus_debut_et_fin_vont_aux_bouts_du_texte() {
        let mut view = editor(20, 3);
        let mut buffer = AirTextBuffer::from_text("une\ndeux\ntrois");
        view.synchronize(&buffer);
        let control = AirModifiers {
            control: true,
            ..AirModifiers::default()
        };
        press_with(&mut view, AirKey::End, control);
        turn(&mut view, &mut buffer);
        assert_eq!(buffer.cursor(), AirTextPosition::new(2, 5));
        press_with(&mut view, AirKey::Home, control);
        turn(&mut view, &mut buffer);
        assert_eq!(buffer.cursor(), AirTextPosition::new(0, 0));
    }

    #[test]
    fn controle_y_supprime_la_ligne() {
        let mut view = editor(20, 3);
        let mut buffer = AirTextBuffer::from_text("une\ndeux\ntrois");
        view.synchronize(&buffer);
        let control = AirModifiers {
            control: true,
            ..AirModifiers::default()
        };
        press_with(&mut view, AirKey::Character('y'), control);
        turn(&mut view, &mut buffer);
        assert_eq!(buffer.lines(), ["deux", "trois"]);
    }

    #[test]
    fn une_frappe_avec_alt_n_est_pas_saisie() {
        // Elle appartient aux raccourcis : la barre de menu doit pouvoir la prendre.
        let mut view = editor(10, 2);
        let alternate = AirModifiers {
            alternate: true,
            ..AirModifiers::default()
        };
        assert_eq!(
            press_with(&mut view, AirKey::Character('f'), alternate),
            AirOutcome::Ignored
        );
        assert!(!view.has_pending_actions());
    }

    #[test]
    fn une_frappe_avec_controle_sans_sens_est_ignoree() {
        let mut view = editor(10, 2);
        let control = AirModifiers {
            control: true,
            ..AirModifiers::default()
        };
        assert_eq!(
            press_with(&mut view, AirKey::Character('q'), control),
            AirOutcome::Ignored
        );
    }

    #[test]
    fn la_tabulation_n_est_pas_consommee() {
        // Absence voulue : insérer un `\t` demanderait de choisir des taquets, et un
        // caractère dont l'affichage serait faux est pire que pas de caractère.
        let mut view = editor(10, 2);
        assert_eq!(press(&mut view, AirKey::Tab), AirOutcome::Ignored);
    }

    #[test]
    fn les_pages_se_mesurent_sur_la_hauteur_de_la_vue() {
        let mut view = editor(10, 4);
        let text: Vec<String> = (0..20).map(|index| format!("l{index}")).collect();
        let mut buffer = AirTextBuffer::from_lines(text);
        view.synchronize(&buffer);
        press(&mut view, AirKey::PageDown);
        turn(&mut view, &mut buffer);
        assert_eq!(buffer.cursor().line, 4);
    }

    #[test]
    fn le_curseur_ramene_la_fenetre_quand_il_sort_par_le_bas() {
        let mut view = editor(10, 3);
        let text: Vec<String> = (0..10).map(|index| format!("l{index}")).collect();
        let mut buffer = AirTextBuffer::from_lines(text);
        view.synchronize(&buffer);
        for _ in 0..4 {
            press(&mut view, AirKey::Down);
            turn(&mut view, &mut buffer);
        }
        // Quatre lignes plus bas dans une fenêtre de trois : la fenêtre a suivi.
        assert_eq!(view.top_line(), 2);
        assert_eq!(
            render(&view, AirSize::new(10, 3)),
            "l2        \nl3        \nl4        "
        );
    }

    #[test]
    fn le_curseur_ramene_la_fenetre_quand_il_sort_par_le_haut() {
        let mut view = editor(10, 2);
        let text: Vec<String> = (0..10).map(|index| format!("l{index}")).collect();
        let mut buffer = AirTextBuffer::from_lines(text);
        view.synchronize(&buffer);
        press(&mut view, AirKey::PageDown);
        turn(&mut view, &mut buffer);
        press(&mut view, AirKey::Up);
        press(&mut view, AirKey::Up);
        press(&mut view, AirKey::Up);
        turn(&mut view, &mut buffer);
        assert_eq!(view.top_line(), buffer.cursor().line);
    }

    #[test]
    fn la_fenetre_defile_horizontalement_avec_le_curseur() {
        let mut view = editor(5, 1);
        let mut buffer = AirTextBuffer::from_text("abcdefghij");
        view.synchronize(&buffer);
        press(&mut view, AirKey::End);
        turn(&mut view, &mut buffer);
        assert_eq!(view.left_column(), 6);
        assert_eq!(render(&view, AirSize::new(5, 1)), "ghij ");
    }

    #[test]
    fn la_molette_defile_sans_deplacer_le_curseur() {
        let mut view = editor(10, 3);
        let text: Vec<String> = (0..20).map(|index| format!("l{index}")).collect();
        let mut buffer = AirTextBuffer::from_lines(text);
        view.synchronize(&buffer);
        mouse(&mut view, AirMouseEventKind::WheelDown, 0, 0);
        turn(&mut view, &mut buffer);
        assert_eq!(view.top_line(), 3);
        // Le curseur n'a pas bougé dans le tampon, et il n'est plus visible.
        assert_eq!(buffer.cursor(), AirTextPosition::new(0, 0));
        assert_eq!(cursor_of(&view, AirSize::new(10, 3)), None);
    }

    #[test]
    fn la_molette_vers_le_haut_remonte_la_fenetre() {
        let mut view = editor(10, 3);
        let text: Vec<String> = (0..20).map(|index| format!("l{index}")).collect();
        let mut buffer = AirTextBuffer::from_lines(text);
        view.synchronize(&buffer);
        view.scroll_to_line(10);
        turn(&mut view, &mut buffer);
        mouse(&mut view, AirMouseEventKind::WheelUp, 0, 0);
        turn(&mut view, &mut buffer);
        assert_eq!(view.top_line(), 7);
    }

    #[test]
    fn un_defilement_hors_du_texte_est_borne() {
        let mut view = editor(10, 3);
        let buffer = AirTextBuffer::from_text("une\ndeux");
        view.synchronize(&buffer);
        view.scroll_to_line(99);
        assert_eq!(view.top_line(), 1);
    }

    #[test]
    fn un_clic_produit_une_intention_de_position() {
        let mut view = editor(10, 3);
        let mut buffer = AirTextBuffer::from_text("une\ndeux\ntrois");
        view.synchronize(&buffer);
        mouse(
            &mut view,
            AirMouseEventKind::Pressed(AirMouseButton::Left),
            2,
            1,
        );
        turn(&mut view, &mut buffer);
        assert_eq!(buffer.cursor(), AirTextPosition::new(1, 2));
    }

    #[test]
    fn un_clic_tient_compte_du_defilement() {
        let mut view = editor(10, 2);
        let text: Vec<String> = (0..10).map(|index| format!("ligne{index}")).collect();
        let mut buffer = AirTextBuffer::from_lines(text);
        view.synchronize(&buffer);
        view.scroll_to_line(4);
        turn(&mut view, &mut buffer);
        mouse(
            &mut view,
            AirMouseEventKind::Pressed(AirMouseButton::Left),
            0,
            1,
        );
        turn(&mut view, &mut buffer);
        assert_eq!(buffer.cursor().line, 5);
    }

    #[test]
    fn le_curseur_est_publie_en_coordonnees_de_contenu() {
        let mut view = editor(10, 3);
        let mut buffer = AirTextBuffer::from_text("une\ndeux");
        view.synchronize(&buffer);
        press(&mut view, AirKey::Down);
        press(&mut view, AirKey::Right);
        turn(&mut view, &mut buffer);
        assert_eq!(
            cursor_of(&view, AirSize::new(10, 3)),
            Some(AirPoint::new(AirColumn::new(1), AirRow::new(1)))
        );
    }

    #[test]
    fn une_vue_desactivee_ne_publie_pas_de_curseur_et_ignore_tout() {
        let mut view = editor(10, 3);
        let buffer = AirTextBuffer::from_text("une");
        view.synchronize(&buffer);
        view.state_mut().set_enabled(false);
        assert_eq!(cursor_of(&view, AirSize::new(10, 3)), None);
        assert_eq!(
            press(&mut view, AirKey::Character('a')),
            AirOutcome::Ignored
        );
        assert_eq!(
            mouse(
                &mut view,
                AirMouseEventKind::Pressed(AirMouseButton::Left),
                0,
                0
            ),
            AirOutcome::Ignored
        );
    }

    #[test]
    fn un_relachement_de_souris_est_ignore() {
        let mut view = editor(10, 3);
        assert_eq!(
            mouse(&mut view, AirMouseEventKind::Released(None), 0, 0),
            AirOutcome::Ignored
        );
    }

    #[test]
    fn un_redimensionnement_refait_le_reflet() {
        // La dimension fait partie des conditions de sortie rapide : sans elle, agrandir
        // la fenêtre laisserait les lignes du bas vides jusqu'à la frappe suivante.
        let mut view = editor(6, 1);
        let buffer = AirTextBuffer::from_text("une\ndeux\ntrois");
        view.synchronize(&buffer);
        assert_eq!(render(&view, AirSize::new(6, 1)), "une   ");
        view.state_mut()
            .set_bounds(AirRectangle::from_parts(0, 0, 6, 3));
        view.synchronize(&buffer);
        assert_eq!(render(&view, AirSize::new(6, 3)), "une   \ndeux  \ntrois ");
    }

    #[test]
    fn une_vue_sans_place_ne_reflete_rien_et_ne_panique_pas() {
        let mut view = AirEditorView::new(AirViewState::new(
            AirViewId::FIRST,
            AirRectangle::from_parts(0, 0, 0, 0),
        ));
        let buffer = AirTextBuffer::from_text("une");
        view.synchronize(&buffer);
        assert_eq!(view.visible_rows(), 0);
        assert_eq!(view.visible_columns(), 0);
        assert_eq!(cursor_of(&view, AirSize::new(4, 2)), None);
    }

    #[test]
    fn une_tranche_visible_commence_au_bord_gauche() {
        assert_eq!(visible_slice("abcdef", 0, 3).text, "abc");
        assert_eq!(visible_slice("abcdef", 2, 3).text, "cde");
        assert_eq!(visible_slice("abcdef", 2, 3).column, 0);
    }

    #[test]
    fn une_tranche_apres_la_fin_de_la_ligne_est_vide() {
        let slice = visible_slice("ab", 5, 4);
        assert_eq!(slice.text, "");
        assert_eq!(slice.column, 0);
    }

    #[test]
    fn une_grappe_large_coupee_par_le_bord_gauche_laisse_sa_place_vide() {
        // On ne dessine pas la moitié d'un glyphe : l'idéogramme disparaît et le `a` est
        // posé en colonne 1, pas en colonne 0.
        let slice = visible_slice("\u{6F22}ab", 1, 4);
        assert_eq!(slice.text, "ab");
        assert_eq!(slice.column, 1);
    }

    #[test]
    fn une_grappe_large_qui_ne_tient_pas_a_droite_est_ecartee() {
        let slice = visible_slice("a\u{6F22}b", 0, 2);
        assert_eq!(slice.text, "a");
    }

    #[test]
    fn une_ligne_vide_donne_une_tranche_vide() {
        assert_eq!(visible_slice("", 0, 10).text, "");
    }

    #[test]
    fn un_texte_large_se_dessine_sans_ecraser_le_voisin() {
        let mut view = editor(5, 1);
        let buffer = AirTextBuffer::from_text("a\u{6F22}b");
        view.synchronize(&buffer);
        // Quatre colonnes occupées, la cinquième reste du fond. L'espace qui suit
        // l'idéogramme est sa **cellule de continuation** : c'est elle qui empêche le `b`
        // de s'écrire sur sa moitié droite.
        assert_eq!(render(&view, AirSize::new(5, 1)), "a漢 b ");
    }

    // ------------------------------------------------------ le surlignage

    /// `Maj` et une touche, qui doit étendre la sélection.
    fn press_with_shift(view: &mut AirEditorView, key: AirKey) -> AirOutcome {
        press_with(
            view,
            key,
            AirModifiers {
                shift: true,
                ..AirModifiers::default()
            },
        )
    }

    #[test]
    fn maj_et_une_fleche_surlignent_le_texte() {
        let (marked, normal) = selection_backgrounds();
        let mut buffer = AirTextBuffer::from_text("abcd");
        let mut view = editor(4, 1);
        view.synchronize(&buffer);
        press_with_shift(&mut view, AirKey::Right);
        press_with_shift(&mut view, AirKey::Right);
        turn(&mut view, &mut buffer);

        assert_eq!(buffer.selected_text().as_deref(), Some("ab"));
        assert_eq!(
            backgrounds(&view, AirSize::new(4, 1), 0),
            vec![Some(marked), Some(marked), Some(normal), Some(normal)],
            "le surlignage ne couvre pas exactement les deux premières colonnes"
        );
    }

    #[test]
    fn une_fleche_nue_n_en_surligne_aucun() {
        let (_, normal) = selection_backgrounds();
        let mut buffer = AirTextBuffer::from_text("abcd");
        let mut view = editor(4, 1);
        view.synchronize(&buffer);
        press(&mut view, AirKey::Right);
        turn(&mut view, &mut buffer);
        assert!(
            backgrounds(&view, AirSize::new(4, 1), 0)
                .iter()
                .all(|background| *background == Some(normal))
        );
    }

    #[test]
    fn abandonner_la_selection_efface_le_surlignage() {
        // Le piège que la génération de sélection évite : sans elle, le reflet ne serait
        // pas repoussé et la surbrillance resterait à l'écran après son abandon.
        let (_, normal) = selection_backgrounds();
        let mut buffer = AirTextBuffer::from_text("abcd");
        let mut view = editor(4, 1);
        view.synchronize(&buffer);
        press_with_shift(&mut view, AirKey::Right);
        turn(&mut view, &mut buffer);

        let mut changes = AirChangeSet::new();
        buffer.apply(AirTextAction::ClearSelection, &mut changes);
        view.synchronize(&buffer);
        assert!(
            backgrounds(&view, AirSize::new(4, 1), 0)
                .iter()
                .all(|background| *background == Some(normal)),
            "la surbrillance a survécu à l'abandon de la sélection"
        );
    }

    #[test]
    fn une_selection_de_plusieurs_lignes_surligne_le_saut_de_ligne() {
        // La colonne qui suit la fin de la ligne porte le saut de ligne, et il est bien
        // dans la sélection : sans elle, une ligne courte paraîtrait trouée.
        let (marked, normal) = selection_backgrounds();
        let mut buffer = AirTextBuffer::from_text("ab\ncd");
        let mut view = editor(4, 2);
        view.synchronize(&buffer);
        press_with_shift(&mut view, AirKey::Down);
        turn(&mut view, &mut buffer);

        assert_eq!(
            backgrounds(&view, AirSize::new(4, 2), 0),
            vec![Some(marked), Some(marked), Some(marked), Some(normal)],
            "le saut de ligne n'est pas surligné"
        );
    }

    #[test]
    fn une_ligne_vide_prise_au_milieu_d_une_selection_porte_une_marque() {
        let (marked, _) = selection_backgrounds();
        let mut buffer = AirTextBuffer::from_text("a\n\nb");
        let mut view = editor(4, 3);
        view.synchronize(&buffer);
        press_with_shift(&mut view, AirKey::Down);
        press_with_shift(&mut view, AirKey::Down);
        turn(&mut view, &mut buffer);

        let middle = backgrounds(&view, AirSize::new(4, 3), 1);
        assert_eq!(
            middle.first().copied().flatten(),
            Some(marked),
            "la ligne vide ne porte aucune marque : la sélection paraît trouée"
        );
    }

    #[test]
    fn le_surlignage_suit_le_defilement_horizontal() {
        // La vue ne garde que les colonnes visibles ; les bornes de la sélection sont des
        // décalages en octets dans la ligne entière. C'est la conversion qui est éprouvée.
        let (marked, normal) = selection_backgrounds();
        let mut buffer = AirTextBuffer::from_text("abcdefgh");
        let mut view = editor(4, 1);
        view.synchronize(&buffer);
        // Le curseur va en fin de ligne, la fenêtre défile, puis on sélectionne à gauche.
        press(&mut view, AirKey::End);
        turn(&mut view, &mut buffer);
        press_with_shift(&mut view, AirKey::Left);
        press_with_shift(&mut view, AirKey::Left);
        turn(&mut view, &mut buffer);

        assert_eq!(buffer.selected_text().as_deref(), Some("gh"));
        // La fenêtre montre les colonnes 5 à 8 : `fgh` puis la place du curseur. Le `gh`
        // sélectionné tombe donc aux positions 1 et 2, et c'est ce décalage qui est
        // éprouvé ici — l'écrire de mémoire donne 2 et 3.
        assert_eq!(render(&view, AirSize::new(4, 1)), "fgh ");
        assert_eq!(
            backgrounds(&view, AirSize::new(4, 1), 0),
            vec![Some(normal), Some(marked), Some(marked), Some(normal)],
            "le surlignage n'a pas suivi le défilement horizontal"
        );
    }

    #[test]
    fn ctrl_a_selectionne_tout() {
        let mut buffer = AirTextBuffer::from_text("ab\ncd");
        let mut view = editor(4, 2);
        view.synchronize(&buffer);
        press_with(
            &mut view,
            AirKey::Character('a'),
            AirModifiers {
                control: true,
                ..AirModifiers::default()
            },
        );
        turn(&mut view, &mut buffer);
        assert_eq!(buffer.selected_text().as_deref(), Some("ab\ncd"));
    }

    #[test]
    fn maj_et_un_clic_etendent_la_selection() {
        let mut buffer = AirTextBuffer::from_text("abcdef");
        let mut view = editor(6, 1);
        view.synchronize(&buffer);
        mouse(
            &mut view,
            AirMouseEventKind::Pressed(AirMouseButton::Left),
            1,
            0,
        );
        turn(&mut view, &mut buffer);
        mouse_with(
            &mut view,
            AirMouseEventKind::Pressed(AirMouseButton::Left),
            4,
            0,
            AirModifiers {
                shift: true,
                ..AirModifiers::default()
            },
        );
        turn(&mut view, &mut buffer);
        assert_eq!(buffer.selected_text().as_deref(), Some("bcd"));
    }

    #[test]
    fn un_clic_nu_abandonne_la_selection() {
        let mut buffer = AirTextBuffer::from_text("abcdef");
        let mut view = editor(6, 1);
        view.synchronize(&buffer);
        press_with_shift(&mut view, AirKey::Right);
        press_with_shift(&mut view, AirKey::Right);
        turn(&mut view, &mut buffer);
        assert!(buffer.has_selection());
        mouse(
            &mut view,
            AirMouseEventKind::Pressed(AirMouseButton::Left),
            4,
            0,
        );
        turn(&mut view, &mut buffer);
        assert_eq!(buffer.selection(), None);
    }

    #[test]
    fn glisser_la_souris_selectionne() {
        // Aucun état n'est gardé dans la vue : l'appui efface la sélection, et le premier
        // déplacement pose l'ancre là où le bouton est tombé.
        let mut buffer = AirTextBuffer::from_text("abcdef");
        let mut view = editor(6, 1);
        view.synchronize(&buffer);
        mouse(
            &mut view,
            AirMouseEventKind::Pressed(AirMouseButton::Left),
            1,
            0,
        );
        turn(&mut view, &mut buffer);
        mouse(
            &mut view,
            AirMouseEventKind::Moved(Some(AirMouseButton::Left)),
            3,
            0,
        );
        turn(&mut view, &mut buffer);
        assert_eq!(buffer.selected_text().as_deref(), Some("bc"));
        mouse(
            &mut view,
            AirMouseEventKind::Moved(Some(AirMouseButton::Left)),
            5,
            0,
        );
        turn(&mut view, &mut buffer);
        assert_eq!(buffer.selected_text().as_deref(), Some("bcde"));
    }

    #[test]
    fn un_deplacement_sans_bouton_ne_selectionne_rien() {
        let mut buffer = AirTextBuffer::from_text("abcdef");
        let mut view = editor(6, 1);
        view.synchronize(&buffer);
        let outcome = mouse(&mut view, AirMouseEventKind::Moved(None), 3, 0);
        assert_eq!(outcome, AirOutcome::Ignored);
        turn(&mut view, &mut buffer);
        assert_eq!(buffer.selection(), None);
    }

    // --------------------------------------- le presse-papiers et le collage

    fn control() -> AirModifiers {
        AirModifiers {
            control: true,
            ..AirModifiers::default()
        }
    }

    #[test]
    fn les_gestes_du_controleur_sortent_en_commandes() {
        // Par le canal des commandes et non celui des intentions : ni le presse-papiers ni
        // la question d'une recherche n'appartiennent à la vue. La casse ne compte pas —
        // un terminal qui transmet `Ctrl`+`Maj`+`F` donne une majuscule.
        for (character, expected) in [
            ('c', crate::command::COPY),
            ('x', crate::command::CUT),
            ('v', crate::command::PASTE),
            ('f', crate::command::FIND),
            ('F', crate::command::FIND),
            ('l', crate::command::FIND_NEXT),
            ('L', crate::command::FIND_NEXT),
        ] {
            let mut view = editor(10, 3);
            let outcome = press_with(&mut view, AirKey::Character(character), control());
            assert_eq!(outcome, AirOutcome::Command(expected));
            assert!(
                !view.has_pending_actions(),
                "la vue a aussi produit une intention pour {character}"
            );
        }
    }

    #[test]
    fn un_collage_encadre_est_insere_d_un_coup() {
        // Le décodeur produit cet événement depuis le jalon M1, et jusqu'au jalon M7d
        // personne ne le consommait : la séquence était décodée, puis jetée.
        let mut buffer = AirTextBuffer::new();
        let mut view = editor(10, 3);
        view.synchronize(&buffer);
        let mut commands = AirCommandSet::new();
        let mut context = AirContext {
            commands: &mut commands,
        };
        let mut event = AirEvent::new(AirInputEvent::Paste("ab\ncd".to_owned()));
        let outcome = view.handle_event(&mut event, &mut context);
        assert_eq!(outcome, AirOutcome::Consumed);
        turn(&mut view, &mut buffer);
        assert_eq!(buffer.lines(), ["ab", "cd"]);
    }

    #[test]
    fn un_collage_encadre_remplace_la_selection() {
        let mut buffer = AirTextBuffer::from_text("abcd");
        let mut view = editor(10, 3);
        view.synchronize(&buffer);
        press_with_shift(&mut view, AirKey::Right);
        press_with_shift(&mut view, AirKey::Right);
        turn(&mut view, &mut buffer);

        let mut commands = AirCommandSet::new();
        let mut context = AirContext {
            commands: &mut commands,
        };
        let mut event = AirEvent::new(AirInputEvent::Paste("XY".to_owned()));
        view.handle_event(&mut event, &mut context);
        turn(&mut view, &mut buffer);
        assert_eq!(buffer.lines(), ["XYcd"]);
    }

    // ------------------------------------------------- le clic multiple

    #[test]
    fn un_double_clic_selectionne_le_mot() {
        // L'enfoncement place le curseur, le double clic prend le segment : les deux
        // intentions sont appliquées dans l'ordre au même tour.
        let mut buffer = AirTextBuffer::from_text("une belle journée");
        let mut view = editor(20, 1);
        view.synchronize(&buffer);
        mouse(
            &mut view,
            AirMouseEventKind::Pressed(AirMouseButton::Left),
            6,
            0,
        );
        mouse(
            &mut view,
            AirMouseEventKind::DoubleClicked(AirMouseButton::Left),
            6,
            0,
        );
        turn(&mut view, &mut buffer);
        assert_eq!(buffer.selected_text().as_deref(), Some("belle"));
    }

    #[test]
    fn un_triple_clic_selectionne_la_ligne() {
        let mut buffer = AirTextBuffer::from_text("une\ndeux\ntrois");
        let mut view = editor(20, 3);
        view.synchronize(&buffer);
        mouse(
            &mut view,
            AirMouseEventKind::Pressed(AirMouseButton::Left),
            2,
            1,
        );
        mouse(
            &mut view,
            AirMouseEventKind::TripleClicked(AirMouseButton::Left),
            2,
            1,
        );
        turn(&mut view, &mut buffer);
        assert_eq!(buffer.selected_text().as_deref(), Some("deux\n"));
    }

    #[test]
    fn le_mot_selectionne_par_un_double_clic_est_surligne() {
        let (marked, normal) = selection_backgrounds();
        let mut buffer = AirTextBuffer::from_text("ab cd");
        let mut view = editor(5, 1);
        view.synchronize(&buffer);
        mouse(
            &mut view,
            AirMouseEventKind::Pressed(AirMouseButton::Left),
            0,
            0,
        );
        mouse(
            &mut view,
            AirMouseEventKind::DoubleClicked(AirMouseButton::Left),
            0,
            0,
        );
        turn(&mut view, &mut buffer);
        assert_eq!(
            backgrounds(&view, AirSize::new(5, 1), 0),
            vec![
                Some(marked),
                Some(marked),
                Some(normal),
                Some(normal),
                Some(normal)
            ]
        );
    }

    #[test]
    fn un_double_clic_du_bouton_droit_ne_selectionne_rien() {
        let mut buffer = AirTextBuffer::from_text("une belle");
        let mut view = editor(20, 1);
        view.synchronize(&buffer);
        let outcome = mouse(
            &mut view,
            AirMouseEventKind::DoubleClicked(AirMouseButton::Right),
            6,
            0,
        );
        assert_eq!(outcome, AirOutcome::Ignored);
        turn(&mut view, &mut buffer);
        assert_eq!(buffer.selection(), None);
    }
}
