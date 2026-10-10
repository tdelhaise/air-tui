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
//! # Ce qui n'est pas là
//!
//! La **sélection** : la vue n'a qu'un curseur, pas d'étendue. `Maj`+flèche, le
//! copier-coller interne et la surbrillance attendent, et avec eux le double-clic.
//!
//! La **tabulation** : insérer un `\t` demande de choisir des taquets et de les appliquer
//! au rendu comme au calcul de colonne. Tant que ce choix n'est pas fait, `Tab` n'est pas
//! consommée — elle reste disponible pour le parcours du focus, plutôt que d'insérer un
//! caractère dont l'affichage serait faux.

use crate::cell::{AirAttributes, AirCell};
use crate::geometry::{AirColumn, AirPoint, AirRectangle, AirRow, AirSize};
use crate::input::event::{AirInputEvent, AirKey, AirMouseButton, AirMouseEventKind};
use crate::model::{AirGeneration, AirTextAction, AirTextBuffer, AirTextPosition};
use crate::theme::AirRole;
use crate::unicode::{grapheme_width, graphemes};
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
        return VisibleLine {
            column: 0,
            text: String::new(),
        };
    };
    let Some(tail) = line.get(begin..) else {
        return VisibleLine {
            column: 0,
            text: String::new(),
        };
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
    }
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
        let resized = self.synchronized_size != Some(size);
        if !cursor_moved && !text_changed && !resized && !self.scrolled {
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
        for row in 0..rows {
            let index = self.top_line.saturating_add(row);
            // Au delà de la dernière ligne : une ligne vide, et non une absence. Le
            // dessin la remplit comme les autres, donc le fond reste uniforme.
            let visible = match buffer.line(index) {
                Some(line) => visible_slice(line, self.left_column, columns),
                None => VisibleLine {
                    column: 0,
                    text: String::new(),
                },
            };
            self.visible_lines.push(visible);
        }
    }

    /// L'intention correspondant à une frappe, s'il y en a une.
    fn action_for_key(&self, key: &crate::input::event::AirKeyEvent) -> Option<AirTextAction> {
        let control = key.modifiers.control;
        let page = self.visible_rows().max(1);
        match key.logical {
            // Une frappe avec `Alt` appartient aux raccourcis ; la barre de menu en décide.
            AirKey::Character(_) if key.modifiers.alternate => None,
            // `Ctrl`+`Y` supprime la ligne : c'est le geste de Turbo Vision, et il n'a pas
            // d'autre équivalent au clavier.
            AirKey::Character('y' | 'Y') if control => Some(AirTextAction::DeleteLine),
            AirKey::Character(_) if control => None,
            AirKey::Character(character) => Some(AirTextAction::InsertCharacter(character)),
            AirKey::Enter => Some(AirTextAction::InsertLineBreak),
            AirKey::Backspace => Some(AirTextAction::DeleteBackward),
            AirKey::Delete => Some(AirTextAction::DeleteForward),
            AirKey::Left if control => Some(AirTextAction::MoveToPreviousWord),
            AirKey::Right if control => Some(AirTextAction::MoveToNextWord),
            AirKey::Left => Some(AirTextAction::MoveLeft),
            AirKey::Right => Some(AirTextAction::MoveRight),
            AirKey::Up => Some(AirTextAction::MoveUp),
            AirKey::Down => Some(AirTextAction::MoveDown),
            AirKey::Home if control => Some(AirTextAction::MoveToDocumentStart),
            AirKey::End if control => Some(AirTextAction::MoveToDocumentEnd),
            AirKey::Home => Some(AirTextAction::MoveToLineStart),
            AirKey::End => Some(AirTextAction::MoveToLineEnd),
            AirKey::PageUp => Some(AirTextAction::MoveUpByLines(page)),
            AirKey::PageDown => Some(AirTextAction::MoveDownByLines(page)),
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
        let size = painter.size();
        painter.fill_rectangle(
            AirRectangle::new(AirPoint::ORIGIN, size),
            AirCell::new(' ', colors.foreground, colors.background),
        );
        for (row, line) in self.visible_lines.iter().enumerate() {
            let Ok(row) = u16::try_from(row) else {
                break;
            };
            if row >= size.rows {
                break;
            }
            if line.text.is_empty() {
                continue;
            }
            painter.draw_text(
                AirPoint::new(AirColumn::new(line.column), AirRow::new(row)),
                &line.text,
                colors.foreground,
                colors.background,
                AirAttributes::NONE,
            );
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
                        let line = self
                            .top_line
                            .saturating_add(usize::from(position.row.value()));
                        let column = self
                            .left_column
                            .saturating_add(usize::from(position.column.value()));
                        self.pending_actions
                            .push(AirTextAction::MoveToColumn { line, column });
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
                let Some(action) = self.action_for_key(key) else {
                    return AirOutcome::Ignored;
                };
                self.pending_actions.push(action);
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
    use crate::command::AirCommandSet;
    use crate::geometry::{AirColumn, AirPoint, AirRectangle, AirRow, AirSize};
    use crate::input::event::{
        AirInputEvent, AirKey, AirKeyEvent, AirModifiers, AirMouseButton, AirMouseEvent,
        AirMouseEventKind,
    };
    use crate::model::{AirChangeSet, AirModel, AirTextAction, AirTextBuffer, AirTextPosition};
    use crate::surface::AirSurface;
    use crate::theme::AirTheme;
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

    fn mouse(
        view: &mut AirEditorView,
        kind: AirMouseEventKind,
        column: u16,
        row: u16,
    ) -> AirOutcome {
        let mut commands = AirCommandSet::new();
        let mut context = AirContext {
            commands: &mut commands,
        };
        let mut event = AirEvent::new(AirInputEvent::Mouse(AirMouseEvent {
            position: AirPoint::new(AirColumn::new(column), AirRow::new(row)),
            kind,
            modifiers: AirModifiers::default(),
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
}
