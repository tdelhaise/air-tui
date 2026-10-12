// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! La barre de menu et ses menus déroulants.
//!
//! # Pourquoi il n'y a pas de mécanisme de surimpression
//!
//! Un menu ouvert doit se dessiner par-dessus le bureau et les fenêtres, donc hors des
//! limites d'une barre d'une seule ligne. On attendrait un mécanisme de surimpression —
//! une exception au découpage par vue.
//!
//! Il n'en faut aucun. La barre **grandit** quand elle s'ouvre : ses limites couvrent
//! alors la zone du menu déroulant, et tout reste à l'intérieur. Le découpage par vue est
//! intact, et l'invariant « une vue ne peut pas déborder de son propriétaire » n'a pas
//! d'exception.
//!
//! # La contrainte que cela impose, et elle est réelle
//!
//! La barre doit être le **frère au premier plan**. L'arbre dessine les enfants de
//! l'arrière vers l'avant et route les clics à l'envers : une barre insérée avant le
//! bureau verrait son menu déroulant recouvert, et les clics dedans iraient au bureau.
//!
//! Insérer la barre **en dernier** est donc obligatoire, pas conseillé. C'est le prix de
//! l'absence de mécanisme dédié, et il est bien plus faible que celui d'un trou dans le
//! modèle de découpage.

use crate::command::AirCommand;
use crate::geometry::{AirColumn, AirPoint, AirRectangle, AirRow, AirSize};
use crate::input::event::{AirInputEvent, AirKey, AirMouseButton, AirMouseEventKind};
use crate::theme::{AirColorPair, AirFrameGlyphs, AirRole, AirTheme};
use crate::view::painter::AirPainter;
use crate::view::{
    AirContext, AirDrawContext, AirEvent, AirOutcome, AirView, AirViewState, frame_glyphs,
};
use crate::widgets::hotkey::split_hotkey;
use crate::{AirBorderStyle, unicode};

/// L'espace entre deux libellés de la barre.
const LABEL_GAP: u16 = 2;
/// La colonne du premier libellé.
const FIRST_LABEL_COLUMN: u16 = 1;
/// La marge intérieure d'un menu déroulant, de chaque côté du libellé.
const BOX_PADDING: u16 = 1;

/// Un item de menu déroulant.
#[derive(Debug, Clone)]
pub struct AirMenuItem {
    /// Le libellé, notation `~X~` comprise. Vide pour un séparateur.
    label: String,
    /// La commande émise. Absente pour un séparateur.
    command: Option<AirCommand>,
}

impl AirMenuItem {
    /// Un item qui émet une commande.
    #[must_use]
    pub fn new(label: impl Into<String>, command: AirCommand) -> Self {
        Self {
            label: label.into(),
            command: Some(command),
        }
    }

    /// Un séparateur, qui ne s'active pas et ne se survole pas.
    #[must_use]
    pub fn separator() -> Self {
        Self {
            label: String::new(),
            command: None,
        }
    }

    /// Vrai s'il s'agit d'un séparateur.
    #[must_use]
    pub const fn is_separator(&self) -> bool {
        self.command.is_none()
    }

    /// Le libellé, notation comprise.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// La commande émise, si l'item en a une.
    #[must_use]
    pub const fn command(&self) -> Option<AirCommand> {
        self.command
    }

    /// La largeur visible du libellé, tildes exclus.
    fn visible_width(&self) -> u16 {
        let parts = split_hotkey(&self.label);
        let width = unicode::text_width(parts.before)
            .saturating_add(parts.hotkey.map_or(0, unicode::text_width))
            .saturating_add(unicode::text_width(parts.after));
        u16::try_from(width).unwrap_or(u16::MAX)
    }
}

/// Un menu de la barre : un libellé et ses items.
#[derive(Debug, Clone)]
pub struct AirMenu {
    /// Le libellé de premier niveau, notation `~X~` comprise.
    label: String,
    /// Les items du menu déroulant.
    items: Vec<AirMenuItem>,
}

impl AirMenu {
    /// Un menu vide.
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            items: Vec::new(),
        }
    }

    /// Le même menu, avec un item de plus.
    #[must_use]
    pub fn with_item(mut self, item: AirMenuItem) -> Self {
        self.items.push(item);
        self
    }

    /// Le libellé de premier niveau.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Les items.
    #[must_use]
    pub fn items(&self) -> &[AirMenuItem] {
        &self.items
    }

    /// La largeur visible du libellé de premier niveau.
    fn visible_label_width(&self) -> u16 {
        let parts = split_hotkey(&self.label);
        let width = unicode::text_width(parts.before)
            .saturating_add(parts.hotkey.map_or(0, unicode::text_width))
            .saturating_add(unicode::text_width(parts.after));
        u16::try_from(width).unwrap_or(u16::MAX)
    }

    /// La dimension du menu déroulant, cadre compris.
    fn box_size(&self) -> AirSize {
        let widest = self
            .items
            .iter()
            .map(AirMenuItem::visible_width)
            .max()
            .unwrap_or(0);
        let columns = widest
            .saturating_add(BOX_PADDING.saturating_mul(2))
            .saturating_add(2)
            .max(4);
        let rows = u16::try_from(self.items.len())
            .unwrap_or(u16::MAX)
            .saturating_add(2);
        AirSize::new(columns, rows)
    }

    /// L'indice du premier item activable, s'il y en a un.
    fn first_selectable(&self) -> Option<usize> {
        self.items.iter().position(|item| !item.is_separator())
    }
}

/// Remplit la zone d'un menu déroulant et trace son cadre.
///
/// Le cadre est dessiné ici et non par la traversée de l'arbre : la vue est la barre
/// entière, et ce cadre n'entoure qu'une partie d'elle.
fn fill_and_frame(painter: &mut AirPainter<'_>, colors: AirColorPair, glyphs: AirFrameGlyphs) {
    let size = painter.size();
    painter.fill_rectangle(AirRectangle::new(AirPoint::ORIGIN, size), colors.cell(' '));
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

/// L'index suivant dans un anneau de `count` éléments, sans opérateur `%`.
///
/// `count` nul rendrait zéro, ce qui n'a pas de sens : les appelants écartent ce cas
/// avant d'appeler.
fn next_in_ring(index: usize, count: usize) -> usize {
    let next = index.saturating_add(1);
    if next >= count { 0 } else { next }
}

/// L'index précédent dans un anneau de `count` éléments.
fn previous_in_ring(index: usize, count: usize) -> usize {
    if index == 0 {
        count.saturating_sub(1)
    } else {
        index.saturating_sub(1)
    }
}

/// Une barre de menu à menus déroulants.
///
/// Elle est en **pré-traitement** : elle voit les événements avant la vue qui a le focus,
/// ce qui lui permet d'intercepter `Échap` quand un menu est ouvert et `Alt`+lettre quand
/// il ne l'est pas. Elle ne consomme que ce qu'elle reconnaît.
pub struct AirMenuBar {
    /// L'état commun.
    state: AirViewState,
    /// Les menus, de gauche à droite.
    menus: Vec<AirMenu>,
    /// L'indice du menu ouvert, s'il y en a un.
    open: Option<usize>,
    /// L'item survolé dans le menu ouvert.
    highlighted: usize,
    /// La largeur de la barre, conservée pour la restaurer à la fermeture.
    columns: u16,
}

impl AirMenuBar {
    /// Une barre de menu vide, sans bordure, en pré-traitement.
    #[must_use]
    pub fn new(mut state: AirViewState) -> Self {
        // La barre est un bandeau, pas une fenêtre : pas de bordure.
        state.set_border(false);
        // Elle doit voir `Échap` et `Alt`+lettre avant la vue qui a le focus.
        state.set_pre_process(true);
        let columns = state.bounds().size.columns;
        Self {
            state,
            menus: Vec::new(),
            open: None,
            highlighted: 0,
            columns,
        }
    }

    /// La même barre, avec un menu de plus.
    #[must_use]
    pub fn with_menu(mut self, menu: AirMenu) -> Self {
        self.menus.push(menu);
        self
    }

    /// Les menus.
    #[must_use]
    pub fn menus(&self) -> &[AirMenu] {
        &self.menus
    }

    /// L'indice du menu ouvert, s'il y en a un.
    #[must_use]
    pub const fn open_menu(&self) -> Option<usize> {
        self.open
    }

    /// L'item survolé dans le menu ouvert.
    #[must_use]
    pub const fn highlighted_item(&self) -> usize {
        self.highlighted
    }

    /// La colonne où commence le libellé du menu d'indice donné.
    fn label_column(&self, index: usize) -> u16 {
        let mut column = FIRST_LABEL_COLUMN;
        for menu in self.menus.iter().take(index) {
            column = column
                .saturating_add(menu.visible_label_width())
                .saturating_add(LABEL_GAP);
        }
        column
    }

    /// L'indice du menu dont le libellé couvre cette colonne.
    fn menu_at_column(&self, column: u16) -> Option<usize> {
        for (index, menu) in self.menus.iter().enumerate() {
            let first = self.label_column(index);
            let last = first
                .saturating_add(menu.visible_label_width())
                .saturating_sub(1);
            if column >= first && column <= last {
                return Some(index);
            }
        }
        None
    }

    /// Les limites du menu déroulant ouvert, en coordonnées de la barre.
    fn open_box_bounds(&self) -> Option<AirRectangle> {
        let index = self.open?;
        let menu = self.menus.get(index)?;
        let size = menu.box_size();
        let column = self.label_column(index).saturating_sub(1);
        Some(AirRectangle::new(
            AirPoint::new(AirColumn::new(column), AirRow::new(1)),
            size,
        ))
    }

    /// Ouvre un menu et ajuste les limites de la barre pour le contenir.
    fn open_at(&mut self, index: usize) {
        let Some(menu) = self.menus.get(index) else {
            return;
        };
        self.open = Some(index);
        self.highlighted = menu.first_selectable().unwrap_or(0);
        let height = menu.box_size().rows.saturating_add(1);
        let bounds = self.state.bounds();
        self.state.set_bounds(AirRectangle::new(
            bounds.origin,
            AirSize::new(self.columns, height),
        ));
    }

    /// Referme le menu ouvert et rend à la barre sa hauteur d'une ligne.
    fn close(&mut self) {
        self.open = None;
        let bounds = self.state.bounds();
        self.state.set_bounds(AirRectangle::new(
            bounds.origin,
            AirSize::new(self.columns, 1),
        ));
    }

    /// Déplace le survol au prochain item activable, dans la direction donnée.
    fn move_highlight(&mut self, forward: bool) {
        let Some(index) = self.open else {
            return;
        };
        let Some(menu) = self.menus.get(index) else {
            return;
        };
        let count = menu.items.len();
        if count == 0 {
            return;
        }
        // Un tour complet au plus : s'il n'y a que des séparateurs, on ne boucle pas.
        let mut candidate = self.highlighted;
        for _ in 0..count {
            candidate = if forward {
                next_in_ring(candidate, count)
            } else {
                previous_in_ring(candidate, count)
            };
            if menu
                .items
                .get(candidate)
                .is_some_and(|item| !item.is_separator())
            {
                self.highlighted = candidate;
                return;
            }
        }
    }

    /// Ouvre le menu voisin, en conservant l'ouverture.
    fn move_menu(&mut self, forward: bool) {
        let Some(index) = self.open else {
            return;
        };
        let count = self.menus.len();
        if count == 0 {
            return;
        }
        let next = if forward {
            next_in_ring(index, count)
        } else {
            previous_in_ring(index, count)
        };
        self.open_at(next);
    }

    /// Active l'item survolé, si sa commande est permise.
    fn activate_highlighted(&mut self, context: &mut AirContext<'_>) -> Option<AirCommand> {
        let index = self.open?;
        let command = self
            .menus
            .get(index)?
            .items
            .get(self.highlighted)?
            .command()?;
        if !context.commands.is_enabled(command) {
            return None;
        }
        self.close();
        Some(command)
    }

    /// L'indice de l'item du menu ouvert dont la lettre d'accès rapide est `character`.
    ///
    /// Un séparateur n'a pas de lettre, et un item sans commande non plus : les deux sont
    /// écartés par `hotkey_character`, qui ne rend rien pour un libellé sans tilde.
    fn item_for_hotkey(&self, character: char) -> Option<usize> {
        let wanted = character.to_ascii_lowercase();
        self.menus
            .get(self.open?)?
            .items
            .iter()
            .position(|item| split_hotkey(item.label()).hotkey_character() == Some(wanted))
    }

    /// L'indice du menu dont la lettre d'accès rapide est `character`.
    fn menu_for_hotkey(&self, character: char) -> Option<usize> {
        let wanted = character.to_ascii_lowercase();
        self.menus
            .iter()
            .position(|menu| split_hotkey(menu.label()).hotkey_character() == Some(wanted))
    }

    /// Dessine le bandeau de la barre.
    fn draw_bar(&self, painter: &mut AirPainter<'_>, theme: &AirTheme) {
        let colors = theme.palette.color_pair(AirRole::MenuBar);
        let selected = theme.palette.color_pair(AirRole::MenuSelected);
        let hotkey = theme.palette.color_pair(AirRole::MenuHotkey);
        let selected_hotkey = theme.palette.color_pair(AirRole::MenuSelectedHotkey);
        let size = painter.size();
        painter.fill_rectangle(
            AirRectangle::from_parts(0, 0, size.columns, 1),
            colors.cell(' '),
        );
        for (index, menu) in self.menus.iter().enumerate() {
            let is_open = self.open == Some(index);
            let (label_colors, letter_colors) = if is_open {
                (selected, selected_hotkey)
            } else {
                (colors, hotkey)
            };
            let parts = split_hotkey(menu.label());
            let mut column = AirColumn::new(self.label_column(index));
            let row = AirRow::FIRST;
            column = painter.draw_text_with_colors(
                AirPoint::new(column, row),
                parts.before,
                label_colors,
            );
            if let Some(letter) = parts.hotkey {
                column = painter.draw_text_with_colors(
                    AirPoint::new(column, row),
                    letter,
                    letter_colors,
                );
            }
            painter.draw_text_with_colors(AirPoint::new(column, row), parts.after, label_colors);
        }
    }

    /// Dessine le menu déroulant ouvert.
    fn draw_box(&self, painter: &mut AirPainter<'_>, context: &AirDrawContext<'_>) {
        let theme = context.theme;
        let Some(bounds) = self.open_box_bounds() else {
            return;
        };
        let Some(index) = self.open else {
            return;
        };
        let Some(menu) = self.menus.get(index) else {
            return;
        };
        let Some(mut box_painter) = painter.child(bounds) else {
            return;
        };

        let colors = theme.palette.color_pair(AirRole::MenuBar);
        let selected = theme.palette.color_pair(AirRole::MenuSelected);
        let disabled = theme.palette.color_pair(AirRole::MenuDisabled);
        let hotkey = theme.palette.color_pair(AirRole::MenuHotkey);
        let selected_hotkey = theme.palette.color_pair(AirRole::MenuSelectedHotkey);
        let glyphs = frame_glyphs(theme, AirBorderStyle::Single);
        let size = box_painter.size();
        let right = size.columns.saturating_sub(1);

        fill_and_frame(&mut box_painter, colors, glyphs);

        for (position, item) in menu.items.iter().enumerate() {
            let row = AirRow::new(u16::try_from(position).unwrap_or(0).saturating_add(1));
            if item.is_separator() {
                let mut column = 1;
                while column < right {
                    box_painter.set_cell(
                        AirPoint::new(AirColumn::new(column), row),
                        colors.cell(glyphs.horizontal),
                    );
                    column = column.saturating_add(1);
                }
                continue;
            }
            // Le grisage vient du jeu de commandes, interrogé ici même : c'est le
            // mécanisme 3 de l'ADR-007, et c'est ce que le contexte de dessin apporte.
            let item_enabled = item
                .command()
                .is_some_and(|command| context.is_command_enabled(command));
            let is_highlighted = position == self.highlighted;
            let (item_colors, letter_colors) = match (item_enabled, is_highlighted) {
                (false, _) => (disabled, disabled),
                (true, true) => (selected, selected_hotkey),
                (true, false) => (colors, hotkey),
            };
            // La ligne entière prend la couleur du survol : sans cela, le survol ne se
            // verrait que sous le texte et la sélection serait illisible.
            let mut column = 1;
            while column < right {
                box_painter.set_cell(
                    AirPoint::new(AirColumn::new(column), row),
                    item_colors.cell(' '),
                );
                column = column.saturating_add(1);
            }
            let parts = split_hotkey(item.label());
            let mut text_column = AirColumn::new(BOX_PADDING.saturating_add(1));
            text_column = box_painter.draw_text_with_colors(
                AirPoint::new(text_column, row),
                parts.before,
                item_colors,
            );
            if let Some(letter) = parts.hotkey {
                text_column = box_painter.draw_text_with_colors(
                    AirPoint::new(text_column, row),
                    letter,
                    letter_colors,
                );
            }
            box_painter.draw_text_with_colors(
                AirPoint::new(text_column, row),
                parts.after,
                item_colors,
            );
        }
    }

    /// Traite un clic. Rend le résultat et dit si l'événement est consommé.
    fn handle_click(
        &mut self,
        position: AirPoint,
        context: &mut AirContext<'_>,
    ) -> (AirOutcome, bool) {
        // Sur le bandeau : ouvrir, refermer, ou changer de menu.
        if position.row.value() == 0 {
            let Some(index) = self.menu_at_column(position.column.value()) else {
                // Un clic sur le bandeau hors de tout libellé referme : c'est le geste
                // qu'on fait pour annuler.
                if self.open.is_some() {
                    self.close();
                    return (AirOutcome::Consumed, true);
                }
                return (AirOutcome::Ignored, false);
            };
            if self.open == Some(index) {
                self.close();
            } else {
                self.open_at(index);
            }
            return (AirOutcome::Consumed, true);
        }
        // Dans le menu ouvert : activer l'item cliqué.
        if let Some(bounds) = self.open_box_bounds()
            && bounds.contains(position)
        {
            let row = position.row.value().saturating_sub(bounds.top());
            let item_index = usize::from(row.saturating_sub(1));
            let is_interior = row >= 1 && row < bounds.size.rows.saturating_sub(1);
            if is_interior {
                self.highlighted = item_index;
                if let Some(command) = self.activate_highlighted(context) {
                    return (AirOutcome::Command(command), true);
                }
            }
            return (AirOutcome::Consumed, true);
        }
        // Un clic ailleurs referme, sans consommer : la vue visée doit tout de même le
        // recevoir, sinon il faudrait cliquer deux fois.
        if self.open.is_some() {
            self.close();
        }
        (AirOutcome::Ignored, false)
    }
}

impl AirView for AirMenuBar {
    fn state(&self) -> &AirViewState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut AirViewState {
        &mut self.state
    }

    fn draw_content(&self, painter: &mut AirPainter<'_>, context: &AirDrawContext<'_>) {
        self.draw_bar(painter, context.theme);
        self.draw_box(painter, context);
    }

    fn handle_event(&mut self, event: &mut AirEvent, context: &mut AirContext<'_>) -> AirOutcome {
        let Some(input) = event.input() else {
            return AirOutcome::Ignored;
        };
        match input {
            AirInputEvent::Mouse(mouse) => {
                if !matches!(mouse.kind, AirMouseEventKind::Pressed(AirMouseButton::Left)) {
                    return AirOutcome::Ignored;
                }
                let position = mouse.position;
                let (outcome, consumed) = self.handle_click(position, context);
                if consumed {
                    event.consume();
                }
                outcome
            }
            AirInputEvent::Key(key) => {
                let logical = key.shortcut_key();
                if key.modifiers.alternate {
                    if let AirKey::Character(character) = logical
                        && let Some(index) = self.menu_for_hotkey(character)
                    {
                        self.open_at(index);
                        event.consume();
                        return AirOutcome::Consumed;
                    }
                    return AirOutcome::Ignored;
                }
                if self.open.is_none() {
                    // Fermée, la barre ne retient rien d'autre : elle est en
                    // pré-traitement et ne doit pas voler les frappes.
                    return AirOutcome::Ignored;
                }
                match logical {
                    AirKey::Escape => {
                        self.close();
                        event.consume();
                        AirOutcome::Consumed
                    }
                    AirKey::Down => {
                        self.move_highlight(true);
                        event.consume();
                        AirOutcome::Consumed
                    }
                    AirKey::Up => {
                        self.move_highlight(false);
                        event.consume();
                        AirOutcome::Consumed
                    }
                    AirKey::Right => {
                        self.move_menu(true);
                        event.consume();
                        AirOutcome::Consumed
                    }
                    AirKey::Left => {
                        self.move_menu(false);
                        event.consume();
                        AirOutcome::Consumed
                    }
                    AirKey::Enter => {
                        event.consume();
                        match self.activate_highlighted(context) {
                            Some(command) => AirOutcome::Command(command),
                            None => AirOutcome::Consumed,
                        }
                    }
                    // La lettre d'accès rapide d'un **item**, dans le menu ouvert : elle
                    // le désigne et l'active d'un coup. C'est ce que fait
                    // `TMenuView::handleEvent`, qui cherche la lettre par `findItem` —
                    // sans distinguer la casse — puis pose `result = current->command`.
                    //
                    // Sans cela, le tilde du libellé promettait une touche qui ne faisait
                    // rien : la lettre était dessinée dans la couleur d'accès rapide, et
                    // inerte.
                    AirKey::Character(character) => {
                        // Un menu ouvert est **modal pour les lettres** : elles lui sont
                        // adressées. Les laisser passer les écrivait dans le document —
                        // mesuré en pseudo-terminal avant ce jalon, où `Alt`+`E` puis
                        // `z` insérait un `z` dans le fichier de `redit`.
                        event.consume();
                        let Some(index) = self.item_for_hotkey(character) else {
                            return AirOutcome::Consumed;
                        };
                        self.highlighted = index;
                        match self.activate_highlighted(context) {
                            Some(command) => AirOutcome::Command(command),
                            None => AirOutcome::Consumed,
                        }
                    }
                    _ => AirOutcome::Ignored,
                }
            }
            _ => AirOutcome::Ignored,
        }
    }
}

impl core::fmt::Debug for AirMenuBar {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("AirMenuBar")
            .field("menus", &self.menus.len())
            .field("open", &self.open)
            .field("highlighted", &self.highlighted)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::{AirMenu, AirMenuBar, AirMenuItem};
    use crate::cell::AirCell;
    use crate::command::{AirCommand, AirCommandSet, OPEN, QUIT, SAVE};
    use crate::geometry::{AirColumn, AirPoint, AirRectangle, AirRow, AirSize};
    use crate::input::event::{
        AirInputEvent, AirKey, AirKeyEvent, AirModifiers, AirMouseButton, AirMouseEvent,
        AirMouseEventKind,
    };
    use crate::surface::AirSurface;
    use crate::theme::AirTheme;
    use crate::view::painter::AirPainter;
    use crate::view::{
        AirContext, AirDrawContext, AirEvent, AirOutcome, AirView, AirViewId, AirViewState,
        dispatch_event, draw_view,
    };

    /// Une barre à trois menus, telle que l'exemple l'emploie.
    fn menu_bar(columns: u16) -> AirMenuBar {
        AirMenuBar::new(AirViewState::new(
            AirViewId::FIRST,
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
            AirMenu::new("~E~dit").with_item(AirMenuItem::new("~C~ut", AirCommand::new(300))),
        )
        .with_menu(AirMenu::new("~W~indow"))
    }

    fn point(column: u16, row: u16) -> AirPoint {
        AirPoint::new(AirColumn::new(column), AirRow::new(row))
    }

    fn key(logical: AirKey) -> AirEvent {
        AirEvent::new(AirInputEvent::Key(AirKeyEvent::press(logical)))
    }

    fn alternate(character: char) -> AirEvent {
        AirEvent::new(AirInputEvent::Key(
            AirKeyEvent::press(AirKey::Character(character))
                .with_modifiers(AirModifiers::NONE.with_alternate()),
        ))
    }

    fn click(column: u16, row: u16) -> AirEvent {
        AirEvent::new(AirInputEvent::Mouse(AirMouseEvent {
            position: point(column, row),
            kind: AirMouseEventKind::Pressed(AirMouseButton::Left),
            modifiers: AirModifiers::NONE,
        }))
    }

    fn release(column: u16, row: u16) -> AirEvent {
        AirEvent::new(AirInputEvent::Mouse(AirMouseEvent {
            position: point(column, row),
            kind: AirMouseEventKind::Released(Some(AirMouseButton::Left)),
            modifiers: AirModifiers::NONE,
        }))
    }

    fn dispatch_with(
        bar: &mut AirMenuBar,
        event: &mut AirEvent,
        commands: &mut AirCommandSet,
    ) -> AirOutcome {
        let mut context = AirContext { commands };
        dispatch_event(bar, event, &mut context)
    }

    fn dispatch(bar: &mut AirMenuBar, event: &mut AirEvent) -> AirOutcome {
        let mut commands = AirCommandSet::new();
        dispatch_with(bar, event, &mut commands)
    }

    /// Dessine la barre sur une surface de la taille donnée et rend l'image, lignes
    /// séparées par des sauts de ligne.
    fn render(bar: &AirMenuBar, commands: &AirCommandSet, size: AirSize) -> String {
        let theme = AirTheme::turbo_vision();
        let mut surface = AirSurface::new(size);
        {
            let mut painter = AirPainter::new(&mut surface);
            let Some(mut own) = painter.child(bar.state().bounds()) else {
                panic!("la barre doit tenir dans la surface");
            };
            draw_view(bar, &mut own, &AirDrawContext::new(&theme, commands));
        }
        let rows: Vec<String> = (0..size.rows)
            .map(|row| {
                (0..size.columns)
                    .map(|column| {
                        surface
                            .cell(point(column, row))
                            .map_or(' ', |found| found.character)
                    })
                    .collect()
            })
            .collect();
        rows.join("\n")
    }

    fn cell_at(bar: &AirMenuBar, size: AirSize, column: u16, row: u16) -> AirCell {
        let theme = AirTheme::turbo_vision();
        let commands = AirCommandSet::new();
        let mut surface = AirSurface::new(size);
        {
            let mut painter = AirPainter::new(&mut surface);
            let Some(mut own) = painter.child(bar.state().bounds()) else {
                panic!("la barre doit tenir dans la surface");
            };
            draw_view(bar, &mut own, &AirDrawContext::new(&theme, &commands));
        }
        *surface
            .cell(point(column, row))
            .unwrap_or_else(|| panic!("la cellule ({column}, {row}) doit exister"))
    }

    #[test]
    fn la_lettre_d_un_item_le_choisit_dans_le_menu_ouvert() {
        // Le tilde du libellé promettait une touche : il la tient depuis ce jalon.
        // `TMenuView::findItem` cherche la lettre sans distinguer la casse, et pose
        // aussitôt la commande de l'item.
        let mut bar = menu_bar(40);
        assert_eq!(
            dispatch(&mut bar, &mut alternate('f')),
            AirOutcome::Consumed
        );
        let outcome = dispatch(&mut bar, &mut key(AirKey::Character('S')));
        assert_eq!(outcome, AirOutcome::Command(SAVE), "la casse ne compte pas");
        assert_eq!(bar.open_menu(), None, "activer un item referme le menu");
    }

    #[test]
    fn une_lettre_inconnue_est_avalee_par_le_menu_ouvert() {
        // Un menu ouvert est modal pour les lettres. Avant ce jalon, celle-ci traversait
        // la barre et arrivait dans la vue focalisée : `Alt`+`E` puis `z` écrivait un `z`
        // dans le document de `redit`, ce qui se mesure en pseudo-terminal.
        let mut bar = menu_bar(40);
        dispatch(&mut bar, &mut alternate('f'));
        assert_eq!(
            dispatch(&mut bar, &mut key(AirKey::Character('z'))),
            AirOutcome::Consumed
        );
        assert_eq!(bar.open_menu(), Some(0), "et le menu reste ouvert");
    }

    #[test]
    fn la_lettre_d_un_item_interdit_ne_produit_aucune_commande() {
        // Même règle que pour `Entrée` : le grisage vient du jeu de commandes, et les
        // deux chemins lisent la même source.
        let mut bar = menu_bar(40);
        let mut commands = AirCommandSet::new();
        commands.disable(SAVE);
        dispatch_with(&mut bar, &mut alternate('f'), &mut commands);
        let outcome = dispatch_with(&mut bar, &mut key(AirKey::Character('s')), &mut commands);
        assert_eq!(outcome, AirOutcome::Consumed);
        assert_eq!(
            bar.open_menu(),
            Some(0),
            "le menu reste ouvert sur un item interdit"
        );
    }

    #[test]
    fn une_barre_neuve_est_sans_bordure_et_en_pre_traitement() {
        // ADR-010 pose la bordure par défaut ; un bandeau d'une ligne ne peut pas en avoir.
        let bar = menu_bar(40);
        assert!(!bar.state().has_border());
        assert!(bar.state().is_pre_process());
        assert_eq!(bar.open_menu(), None);
        assert_eq!(bar.menus().len(), 3);
    }

    #[test]
    fn un_item_porte_son_libelle_et_sa_commande() {
        let item = AirMenuItem::new("~O~pen", OPEN);
        assert_eq!(item.label(), "~O~pen");
        assert_eq!(item.command(), Some(OPEN));
        assert!(!item.is_separator());

        let bar = AirMenuItem::separator();
        assert_eq!(bar.label(), "");
        assert_eq!(bar.command(), None);
        assert!(bar.is_separator());
    }

    #[test]
    fn la_largeur_visible_ignore_les_tildes() {
        // « ~O~pen » fait quatre caractères à l'écran, pas six.
        assert_eq!(AirMenuItem::new("~O~pen", OPEN).visible_width(), 4);
        assert_eq!(AirMenu::new("~F~ile").visible_label_width(), 4);
    }

    #[test]
    fn la_taille_du_menu_tient_le_plus_large_item() {
        // « ~O~pen » et « ~S~ave » font 4, « E~x~it » fait 4 : 4 + 2 de marge + 2 de cadre.
        let bar = menu_bar(40);
        let menu = bar.menus().first().expect("le menu « File » doit exister");
        assert_eq!(menu.box_size(), AirSize::new(8, 6));
        // Un menu vide garde une taille minimale valable : un cadre a besoin de 4 colonnes.
        assert_eq!(AirMenu::new("~W~indow").box_size(), AirSize::new(4, 2));
    }

    #[test]
    fn les_colonnes_des_libelles_suivent_les_largeurs_visibles() {
        let bar = menu_bar(40);
        // « File » commence en 1, puis 4 colonnes et 2 d'espace.
        assert_eq!(bar.label_column(0), 1);
        assert_eq!(bar.label_column(1), 7);
        assert_eq!(bar.label_column(2), 13);
        // Chaque libellé couvre sa largeur, et rien de plus.
        assert_eq!(bar.menu_at_column(0), None);
        assert_eq!(bar.menu_at_column(1), Some(0));
        assert_eq!(bar.menu_at_column(4), Some(0));
        assert_eq!(bar.menu_at_column(5), None);
        assert_eq!(bar.menu_at_column(7), Some(1));
        assert_eq!(bar.menu_at_column(13), Some(2));
        assert_eq!(bar.menu_at_column(39), None);
    }

    #[test]
    fn le_bandeau_affiche_les_libelles_sans_les_tildes() {
        let bar = menu_bar(30);
        let image = render(&bar, &AirCommandSet::new(), AirSize::new(30, 1));
        assert_eq!(image, " File  Edit  Window           ");
    }

    #[test]
    fn l_ouverture_agrandit_la_barre_et_la_fermeture_la_rend() {
        // Il n'y a pas de mécanisme de superposition : la barre grandit pour contenir
        // son menu déroulant, et tout reste dans ses limites.
        let mut bar = menu_bar(40);
        assert_eq!(bar.state().bounds().size, AirSize::new(40, 1));
        bar.open_at(0);
        // 6 lignes de menu, plus la ligne du bandeau.
        assert_eq!(bar.state().bounds().size, AirSize::new(40, 7));
        bar.close();
        assert_eq!(bar.state().bounds().size, AirSize::new(40, 1));
    }

    #[test]
    fn l_ouverture_d_un_indice_inconnu_ne_fait_rien() {
        let mut bar = menu_bar(40);
        bar.open_at(9);
        assert_eq!(bar.open_menu(), None);
        assert_eq!(bar.state().bounds().size, AirSize::new(40, 1));
    }

    #[test]
    fn l_ouverture_survole_le_premier_item_activable() {
        let mut bar = menu_bar(40);
        bar.open_at(0);
        assert_eq!(bar.highlighted_item(), 0);
    }

    #[test]
    fn le_menu_ouvert_se_dessine_sous_son_libelle() {
        let mut bar = menu_bar(13);
        bar.open_at(1);
        // Le menu « Edit » commence en colonne 7 : son cadre démarre une colonne avant.
        let image = render(&bar, &AirCommandSet::new(), AirSize::new(13, 4));
        assert_eq!(
            image,
            concat!(
                " File  Edit  \n",
                "      ┌─────┐\n",
                "      │ Cut │\n",
                "      └─────┘",
            )
        );
    }

    #[test]
    fn un_separateur_se_dessine_comme_un_trait() {
        let mut bar = menu_bar(8);
        bar.open_at(0);
        // Le séparateur joint les deux montants, comme dans Turbo Vision. Le libellé
        // « Edit » est coupé par le découpage, à une colonne de la largeur : c'est bien
        // le découpage qui le tronque, pas la barre qui déborde.
        let image = render(&bar, &AirCommandSet::new(), AirSize::new(8, 7));
        assert_eq!(
            image,
            concat!(
                " File  E\n",
                "┌──────┐\n",
                "│ Open │\n",
                "│ Save │\n",
                "│──────│\n",
                "│ Exit │\n",
                "└──────┘",
            )
        );
    }

    #[test]
    fn un_item_interdit_est_grise_a_partir_du_jeu_de_commandes() {
        // Mécanisme 3 de l'ADR-007 : la vue interroge le jeu quand elle se dessine.
        let theme = AirTheme::turbo_vision();
        let mut bar = menu_bar(40);
        bar.open_at(0);
        let size = AirSize::new(40, 8);

        let enabled = cell_at(&bar, size, 2, 2);
        let mut commands = AirCommandSet::new();
        commands.disable(OPEN);
        let mut surface = AirSurface::new(size);
        {
            let mut painter = AirPainter::new(&mut surface);
            let Some(mut own) = painter.child(bar.state().bounds()) else {
                panic!("la barre doit tenir dans la surface");
            };
            draw_view(&bar, &mut own, &AirDrawContext::new(&theme, &commands));
        }
        let disabled = surface
            .cell(point(2, 2))
            .expect("la cellule de l'item « Open » doit exister");
        assert_ne!(enabled.foreground, disabled.foreground);
    }

    #[test]
    fn alt_plus_lettre_ouvre_le_menu_correspondant() {
        let mut bar = menu_bar(40);
        let mut event = alternate('f');
        assert_eq!(dispatch(&mut bar, &mut event), AirOutcome::Consumed);
        assert!(event.is_consumed());
        assert_eq!(bar.open_menu(), Some(0));
        // La casse n'intervient pas : la notation est « ~F~ile », la frappe est « f ».
        let mut bar = menu_bar(40);
        let mut event = alternate('W');
        assert_eq!(dispatch(&mut bar, &mut event), AirOutcome::Consumed);
        assert_eq!(bar.open_menu(), Some(2));
    }

    #[test]
    fn alt_plus_lettre_inconnue_ne_consomme_rien() {
        let mut bar = menu_bar(40);
        let mut event = alternate('z');
        assert_eq!(dispatch(&mut bar, &mut event), AirOutcome::Ignored);
        assert!(!event.is_consumed());
        assert_eq!(bar.open_menu(), None);
    }

    #[test]
    fn fermee_la_barre_ne_vole_aucune_frappe() {
        // Elle est en pré-traitement : si elle consommait, la vue focalisée ne verrait
        // plus rien.
        let mut bar = menu_bar(40);
        for logical in [AirKey::Down, AirKey::Enter, AirKey::Escape, AirKey::Left] {
            let mut event = key(logical);
            assert_eq!(dispatch(&mut bar, &mut event), AirOutcome::Ignored);
            assert!(!event.is_consumed());
        }
    }

    #[test]
    fn echap_referme_le_menu_ouvert() {
        let mut bar = menu_bar(40);
        bar.open_at(0);
        let mut event = key(AirKey::Escape);
        assert_eq!(dispatch(&mut bar, &mut event), AirOutcome::Consumed);
        assert_eq!(bar.open_menu(), None);
    }

    #[test]
    fn les_fleches_verticales_sautent_les_separateurs() {
        let mut bar = menu_bar(40);
        bar.open_at(0);
        // Open, Save, séparateur, Exit : vers le bas, l'indice 2 est sauté.
        let mut event = key(AirKey::Down);
        dispatch(&mut bar, &mut event);
        assert_eq!(bar.highlighted_item(), 1);
        let mut event = key(AirKey::Down);
        dispatch(&mut bar, &mut event);
        assert_eq!(bar.highlighted_item(), 3);
        // Et l'anneau revient au premier.
        let mut event = key(AirKey::Down);
        dispatch(&mut bar, &mut event);
        assert_eq!(bar.highlighted_item(), 0);
        // Vers le haut, le séparateur est sauté aussi.
        let mut event = key(AirKey::Up);
        dispatch(&mut bar, &mut event);
        assert_eq!(bar.highlighted_item(), 3);
        let mut event = key(AirKey::Up);
        dispatch(&mut bar, &mut event);
        assert_eq!(bar.highlighted_item(), 1);
    }

    #[test]
    fn un_menu_sans_item_activable_ne_bloque_pas_le_survol() {
        // Un tour complet au plus : la boucle se termine même sans item activable.
        let mut bar = AirMenuBar::new(AirViewState::new(
            AirViewId::FIRST,
            AirRectangle::from_parts(0, 0, 20, 1),
        ))
        .with_menu(
            AirMenu::new("~O~nly")
                .with_item(AirMenuItem::separator())
                .with_item(AirMenuItem::separator()),
        );
        bar.open_at(0);
        let mut event = key(AirKey::Down);
        assert_eq!(dispatch(&mut bar, &mut event), AirOutcome::Consumed);
        assert_eq!(bar.highlighted_item(), 0);
    }

    #[test]
    fn les_fleches_horizontales_changent_de_menu_en_anneau() {
        let mut bar = menu_bar(40);
        bar.open_at(0);
        let mut event = key(AirKey::Right);
        dispatch(&mut bar, &mut event);
        assert_eq!(bar.open_menu(), Some(1));
        let mut event = key(AirKey::Right);
        dispatch(&mut bar, &mut event);
        assert_eq!(bar.open_menu(), Some(2));
        let mut event = key(AirKey::Right);
        dispatch(&mut bar, &mut event);
        assert_eq!(bar.open_menu(), Some(0));
        let mut event = key(AirKey::Left);
        dispatch(&mut bar, &mut event);
        assert_eq!(bar.open_menu(), Some(2));
    }

    #[test]
    fn entree_emet_la_commande_de_l_item_survole_et_referme() {
        let mut bar = menu_bar(40);
        bar.open_at(0);
        let mut event = key(AirKey::Enter);
        assert_eq!(dispatch(&mut bar, &mut event), AirOutcome::Command(OPEN));
        assert_eq!(bar.open_menu(), None);
        assert_eq!(bar.state().bounds().size, AirSize::new(40, 1));
    }

    #[test]
    fn entree_sur_un_item_interdit_ne_l_emet_pas_et_laisse_ouvert() {
        let mut bar = menu_bar(40);
        bar.open_at(0);
        let mut commands = AirCommandSet::new();
        commands.disable(OPEN);
        let mut event = key(AirKey::Enter);
        assert_eq!(
            dispatch_with(&mut bar, &mut event, &mut commands),
            AirOutcome::Consumed
        );
        assert_eq!(bar.open_menu(), Some(0));
    }

    #[test]
    fn un_clic_sur_un_libelle_ouvre_puis_referme() {
        let mut bar = menu_bar(40);
        let mut event = click(1, 0);
        assert_eq!(dispatch(&mut bar, &mut event), AirOutcome::Consumed);
        assert_eq!(bar.open_menu(), Some(0));
        let mut event = click(1, 0);
        assert_eq!(dispatch(&mut bar, &mut event), AirOutcome::Consumed);
        assert_eq!(bar.open_menu(), None);
    }

    #[test]
    fn un_clic_sur_un_autre_libelle_change_de_menu() {
        let mut bar = menu_bar(40);
        bar.open_at(0);
        let mut event = click(7, 0);
        assert_eq!(dispatch(&mut bar, &mut event), AirOutcome::Consumed);
        assert_eq!(bar.open_menu(), Some(1));
    }

    #[test]
    fn un_clic_sur_le_bandeau_hors_libelle_referme_ou_est_ignore() {
        let mut bar = menu_bar(40);
        // Fermée, un clic dans le vide du bandeau ne concerne personne.
        let mut event = click(30, 0);
        assert_eq!(dispatch(&mut bar, &mut event), AirOutcome::Ignored);
        assert!(!event.is_consumed());
        // Ouverte, c'est le geste d'annulation.
        bar.open_at(0);
        let mut event = click(30, 0);
        assert_eq!(dispatch(&mut bar, &mut event), AirOutcome::Consumed);
        assert_eq!(bar.open_menu(), None);
    }

    #[test]
    fn un_clic_sur_un_item_l_active() {
        let mut bar = menu_bar(40);
        bar.open_at(0);
        // Ligne 1 du menu « File », deuxième item : « Save ».
        let mut event = click(3, 3);
        assert_eq!(dispatch(&mut bar, &mut event), AirOutcome::Command(SAVE));
        assert_eq!(bar.open_menu(), None);
    }

    #[test]
    fn un_clic_sur_le_cadre_du_menu_ne_l_active_pas() {
        let mut bar = menu_bar(40);
        bar.open_at(0);
        // La dernière ligne du menu est son cadre bas.
        let mut event = click(3, 6);
        assert_eq!(dispatch(&mut bar, &mut event), AirOutcome::Consumed);
        assert_eq!(bar.open_menu(), Some(0));
    }

    #[test]
    fn un_clic_hors_du_menu_referme_sans_consommer() {
        // Sinon il faudrait cliquer deux fois : une pour fermer, une pour agir.
        let mut bar = menu_bar(40);
        bar.open_at(0);
        let mut event = click(30, 4);
        assert_eq!(dispatch(&mut bar, &mut event), AirOutcome::Ignored);
        assert!(!event.is_consumed());
        assert_eq!(bar.open_menu(), None);
    }

    #[test]
    fn un_relachement_de_souris_est_ignore() {
        let mut bar = menu_bar(40);
        let mut event = release(1, 0);
        assert_eq!(dispatch(&mut bar, &mut event), AirOutcome::Ignored);
        assert_eq!(bar.open_menu(), None);
    }

    #[test]
    fn les_accesseurs_rendent_ce_qui_a_ete_pose() {
        let mut bar = menu_bar(40);
        let items = bar.menus().first().expect("le menu « File »").items();
        assert_eq!(items.len(), 4);
        assert_eq!(items.first().map(AirMenuItem::label), Some("~O~pen"));
        // L'état est accessible en écriture : c'est par là qu'une application déplace la
        // barre quand le terminal change de taille.
        bar.state_mut()
            .set_bounds(AirRectangle::from_parts(0, 0, 80, 1));
        assert_eq!(bar.state().bounds().size, AirSize::new(80, 1));
    }

    #[test]
    fn la_representation_de_debogage_dit_l_essentiel() {
        let mut bar = menu_bar(40);
        bar.open_at(1);
        let text = format!("{bar:?}");
        assert!(text.contains("menus: 3"), "{text}");
        assert!(text.contains("open: Some(1)"), "{text}");
    }
}
