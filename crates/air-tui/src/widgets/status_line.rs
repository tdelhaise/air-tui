// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! La barre de statut : des raccourcis affichés, cliquables et actifs au clavier.
//!
//! # Pourquoi elle est en post-traitement
//!
//! C'est l'inverse de la barre de menu. Celle-ci intercepte `Alt`+lettre **avant** la vue
//! qui a le focus, parce qu'aucun champ de saisie n'a d'usage pour `Alt`+lettre. La barre
//! de statut, elle, publie des touches de fonction : si elle les prenait en
//! pré-traitement, une vue qui veut `F3` pour elle-même ne la verrait jamais. Elle est
//! donc en **post-traitement** — elle ne reçoit que ce que personne n'a voulu.
//!
//! # Ce que le grisage interdit
//!
//! Un item dont la commande est interdite est affiché en gris **et** reste muet : ni le
//! clic ni la touche ne l'activent. Le grisage vient du jeu de commandes interrogé au
//! moment du dessin (mécanisme 3 de l'ADR-007) ; le refus vient du même jeu interrogé au
//! moment de l'événement. Les deux chemins ne peuvent pas diverger : ils lisent la même
//! source.

use crate::cell::{AirAttributes, AirCell};
use crate::command::AirCommand;
use crate::geometry::{AirColumn, AirPoint, AirRectangle, AirRow};
use crate::input::event::{AirInputEvent, AirKey, AirMouseButton, AirMouseEventKind};
use crate::theme::AirRole;
use crate::unicode;
use crate::view::painter::AirPainter;
use crate::view::{AirContext, AirDrawContext, AirEvent, AirOutcome, AirView, AirViewState};
use crate::widgets::hotkey::split_hotkey;

/// L'espace entre deux items de la barre.
const ITEM_GAP: u16 = 1;
/// La colonne du premier item.
const FIRST_ITEM_COLUMN: u16 = 1;

/// Un item de barre de statut : un libellé, une touche, une commande.
#[derive(Debug, Clone)]
pub struct AirStatusItem {
    /// Le libellé, notation `~X~` comprise — par exemple `~F2~ Save`.
    label: String,
    /// La touche qui active l'item. Absente pour un item seulement cliquable.
    key: Option<AirKey>,
    /// La commande émise.
    command: AirCommand,
}

impl AirStatusItem {
    /// Un item activé par une touche et par un clic.
    #[must_use]
    pub fn new(label: impl Into<String>, key: AirKey, command: AirCommand) -> Self {
        Self {
            label: label.into(),
            key: Some(key),
            command,
        }
    }

    /// Un item seulement cliquable.
    ///
    /// Utile pour une indication qui a tout de même une action — « Overwrite » par
    /// exemple — sans occuper une touche de fonction.
    #[must_use]
    pub fn clickable(label: impl Into<String>, command: AirCommand) -> Self {
        Self {
            label: label.into(),
            key: None,
            command,
        }
    }

    /// Le libellé, notation comprise.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// La touche qui active l'item, s'il en a une.
    #[must_use]
    pub const fn key(&self) -> Option<AirKey> {
        self.key
    }

    /// La commande émise.
    #[must_use]
    pub const fn command(&self) -> AirCommand {
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

/// Une barre de statut à items.
///
/// Elle est en **post-traitement** : elle ne voit une frappe qu'après la vue qui a le
/// focus, et ne consomme que ce qu'elle reconnaît.
pub struct AirStatusLine {
    /// L'état commun.
    state: AirViewState,
    /// Les items, de gauche à droite.
    items: Vec<AirStatusItem>,
}

impl AirStatusLine {
    /// Une barre de statut vide, sans bordure, en post-traitement.
    #[must_use]
    pub fn new(mut state: AirViewState) -> Self {
        // La barre est un bandeau d'une ligne : une bordure ne laisserait pas de place.
        state.set_border(false);
        // Les touches de fonction ne doivent pas être volées à la vue focalisée.
        state.set_post_process(true);
        Self {
            state,
            items: Vec::new(),
        }
    }

    /// La même barre, avec un item de plus.
    #[must_use]
    pub fn with_item(mut self, item: AirStatusItem) -> Self {
        self.items.push(item);
        self
    }

    /// Les items.
    #[must_use]
    pub fn items(&self) -> &[AirStatusItem] {
        &self.items
    }

    /// La colonne où commence l'item d'indice donné.
    fn item_column(&self, index: usize) -> u16 {
        let mut column = FIRST_ITEM_COLUMN;
        for item in self.items.iter().take(index) {
            column = column
                .saturating_add(item.visible_width())
                .saturating_add(ITEM_GAP);
        }
        column
    }

    /// L'indice de l'item dont le libellé couvre cette colonne.
    fn item_at_column(&self, column: u16) -> Option<usize> {
        for (index, item) in self.items.iter().enumerate() {
            let first = self.item_column(index);
            let last = first.saturating_add(item.visible_width()).saturating_sub(1);
            if column >= first && column <= last {
                return Some(index);
            }
        }
        None
    }

    /// L'indice de l'item que cette touche active.
    fn item_for_key(&self, logical: AirKey) -> Option<usize> {
        self.items
            .iter()
            .position(|item| item.key() == Some(logical))
    }

    /// Émet la commande de l'item d'indice donné, si elle est permise.
    ///
    /// Rend `None` si l'item n'existe pas ou si sa commande est interdite — un item grisé
    /// est muet, par le clic comme par la touche.
    fn emit(&self, index: usize, context: &AirContext<'_>) -> Option<AirCommand> {
        let command = self.items.get(index)?.command();
        if context.commands.is_enabled(command) {
            Some(command)
        } else {
            None
        }
    }
}

impl AirView for AirStatusLine {
    fn state(&self) -> &AirViewState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut AirViewState {
        &mut self.state
    }

    fn draw_content(&self, painter: &mut AirPainter<'_>, context: &AirDrawContext<'_>) {
        let palette = &context.theme.palette;
        let normal = palette.color_pair(AirRole::StatusLine);
        let hotkey = palette.color_pair(AirRole::StatusHotkey);
        let disabled = palette.color_pair(AirRole::StatusDisabled);
        let size = painter.size();
        painter.fill_rectangle(
            AirRectangle::from_parts(0, 0, size.columns, 1),
            AirCell::new(' ', normal.foreground, normal.background),
        );
        for (index, item) in self.items.iter().enumerate() {
            // Le grisage est lu ici, au dessin : c'est le mécanisme 3 de l'ADR-007.
            let enabled = context.is_command_enabled(item.command());
            let (label_colors, letter_colors) = if enabled {
                (normal, hotkey)
            } else {
                (disabled, disabled)
            };
            let parts = split_hotkey(item.label());
            let mut column = AirColumn::new(self.item_column(index));
            column = painter.draw_text(
                AirPoint::new(column, AirRow::FIRST),
                parts.before,
                label_colors.foreground,
                label_colors.background,
                AirAttributes::NONE,
            );
            if let Some(letter) = parts.hotkey {
                column = painter.draw_text(
                    AirPoint::new(column, AirRow::FIRST),
                    letter,
                    letter_colors.foreground,
                    letter_colors.background,
                    AirAttributes::NONE,
                );
            }
            painter.draw_text(
                AirPoint::new(column, AirRow::FIRST),
                parts.after,
                label_colors.foreground,
                label_colors.background,
                AirAttributes::NONE,
            );
        }
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
                let Some(index) = self.item_at_column(mouse.position.column.value()) else {
                    return AirOutcome::Ignored;
                };
                // Un clic sur un item grisé est tout de même consommé : il a bien atteint
                // la barre, et le laisser passer à la vue du dessous n'aurait aucun sens.
                event.consume();
                match self.emit(index, context) {
                    Some(command) => AirOutcome::Command(command),
                    None => AirOutcome::Consumed,
                }
            }
            AirInputEvent::Key(key) => {
                let logical = key.shortcut_key();
                // Un raccourci de barre de statut est une touche nue : `Ctrl`+`F2` n'est
                // pas `F2`, et ne doit pas l'activer.
                if key.modifiers.control || key.modifiers.alternate || key.modifiers.shift {
                    return AirOutcome::Ignored;
                }
                let Some(index) = self.item_for_key(logical) else {
                    return AirOutcome::Ignored;
                };
                let Some(command) = self.emit(index, context) else {
                    // Interdite : la barre ne consomme pas. La frappe reste disponible,
                    // et l'utilisateur ne voit pas une touche disparaître sans effet.
                    return AirOutcome::Ignored;
                };
                event.consume();
                AirOutcome::Command(command)
            }
            _ => AirOutcome::Ignored,
        }
    }
}

impl core::fmt::Debug for AirStatusLine {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("AirStatusLine")
            .field("items", &self.items.len())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::{AirStatusItem, AirStatusLine};
    use crate::command::{AirCommandSet, OPEN, QUIT, SAVE};
    use crate::geometry::{AirColumn, AirPoint, AirRectangle, AirRow, AirSize};
    use crate::input::event::{
        AirInputEvent, AirKey, AirKeyEvent, AirModifiers, AirMouseButton, AirMouseEvent,
        AirMouseEventKind,
    };
    use crate::surface::AirSurface;
    use crate::theme::{AirRole, AirTheme};
    use crate::view::painter::AirPainter;
    use crate::view::{
        AirContext, AirDrawContext, AirEvent, AirOutcome, AirView, AirViewId, AirViewState,
        dispatch_event, draw_view,
    };

    /// La barre de statut de l'exemple : trois items, deux au clavier.
    fn status_line(columns: u16) -> AirStatusLine {
        AirStatusLine::new(AirViewState::new(
            AirViewId::FIRST,
            AirRectangle::from_parts(0, 0, columns, 1),
        ))
        .with_item(AirStatusItem::new("~F2~ Save", AirKey::Function(2), SAVE))
        .with_item(AirStatusItem::new("~F3~ Open", AirKey::Function(3), OPEN))
        .with_item(AirStatusItem::clickable("~X~ Exit", QUIT))
    }

    fn point(column: u16, row: u16) -> AirPoint {
        AirPoint::new(AirColumn::new(column), AirRow::new(row))
    }

    fn key(logical: AirKey) -> AirEvent {
        AirEvent::new(AirInputEvent::Key(AirKeyEvent::press(logical)))
    }

    fn key_with(logical: AirKey, modifiers: AirModifiers) -> AirEvent {
        AirEvent::new(AirInputEvent::Key(
            AirKeyEvent::press(logical).with_modifiers(modifiers),
        ))
    }

    fn click(column: u16) -> AirEvent {
        AirEvent::new(AirInputEvent::Mouse(AirMouseEvent {
            position: point(column, 0),
            kind: AirMouseEventKind::Pressed(AirMouseButton::Left),
            modifiers: AirModifiers::NONE,
        }))
    }

    fn dispatch_with(
        line: &mut AirStatusLine,
        event: &mut AirEvent,
        commands: &mut AirCommandSet,
    ) -> AirOutcome {
        let mut context = AirContext { commands };
        dispatch_event(line, event, &mut context)
    }

    fn dispatch(line: &mut AirStatusLine, event: &mut AirEvent) -> AirOutcome {
        let mut commands = AirCommandSet::new();
        dispatch_with(line, event, &mut commands)
    }

    /// Dessine la barre et rend la ligne obtenue.
    fn render(line: &AirStatusLine, commands: &AirCommandSet, columns: u16) -> String {
        let theme = AirTheme::turbo_vision();
        let size = AirSize::new(columns, 1);
        let mut surface = AirSurface::new(size);
        {
            let mut painter = AirPainter::new(&mut surface);
            let Some(mut own) = painter.child(line.state().bounds()) else {
                panic!("la barre doit tenir dans la surface");
            };
            draw_view(line, &mut own, &AirDrawContext::new(&theme, commands));
        }
        (0..columns)
            .map(|column| {
                surface
                    .cell(point(column, 0))
                    .map_or(' ', |found| found.character)
            })
            .collect()
    }

    /// La couleur du caractère en colonne donnée, pour un jeu de commandes donné.
    fn foreground_at(
        line: &AirStatusLine,
        commands: &AirCommandSet,
        column: u16,
    ) -> crate::cell::AirColor {
        let theme = AirTheme::turbo_vision();
        let size = AirSize::new(40, 1);
        let mut surface = AirSurface::new(size);
        {
            let mut painter = AirPainter::new(&mut surface);
            let Some(mut own) = painter.child(line.state().bounds()) else {
                panic!("la barre doit tenir dans la surface");
            };
            draw_view(line, &mut own, &AirDrawContext::new(&theme, commands));
        }
        surface
            .cell(point(column, 0))
            .map_or(crate::cell::AirColor::BLACK, |found| found.foreground)
    }

    #[test]
    fn une_barre_neuve_est_sans_bordure_et_en_post_traitement() {
        // Elle publie des touches de fonction : les prendre avant la vue focalisée
        // reviendrait à les lui voler.
        let line = status_line(40);
        assert!(!line.state().has_border());
        assert!(line.state().is_post_process());
        assert!(!line.state().is_pre_process());
        assert_eq!(line.items().len(), 3);
    }

    #[test]
    fn un_item_porte_son_libelle_sa_touche_et_sa_commande() {
        let keyed = AirStatusItem::new("~F2~ Save", AirKey::Function(2), SAVE);
        assert_eq!(keyed.label(), "~F2~ Save");
        assert_eq!(keyed.key(), Some(AirKey::Function(2)));
        assert_eq!(keyed.command(), SAVE);

        let clickable = AirStatusItem::clickable("~X~ Exit", QUIT);
        assert_eq!(clickable.key(), None);
        assert_eq!(clickable.command(), QUIT);
    }

    #[test]
    fn la_largeur_visible_ignore_les_tildes() {
        // « ~F2~ Save » fait sept caractères à l'écran, pas neuf.
        assert_eq!(
            AirStatusItem::new("~F2~ Save", AirKey::Function(2), SAVE).visible_width(),
            7
        );
    }

    #[test]
    fn les_colonnes_des_items_suivent_les_largeurs_visibles() {
        let line = status_line(40);
        assert_eq!(line.item_column(0), 1);
        // 7 colonnes pour « F2 Save », plus une d'espace.
        assert_eq!(line.item_column(1), 9);
        assert_eq!(line.item_column(2), 17);
        assert_eq!(line.item_at_column(0), None);
        assert_eq!(line.item_at_column(1), Some(0));
        assert_eq!(line.item_at_column(7), Some(0));
        assert_eq!(line.item_at_column(8), None);
        assert_eq!(line.item_at_column(9), Some(1));
        assert_eq!(line.item_at_column(17), Some(2));
        assert_eq!(line.item_at_column(39), None);
    }

    #[test]
    fn la_barre_affiche_les_libelles_sans_les_tildes() {
        let line = status_line(30);
        assert_eq!(
            render(&line, &AirCommandSet::new(), 30),
            " F2 Save F3 Open X Exit       "
        );
    }

    #[test]
    fn un_item_interdit_est_grise_au_dessin() {
        // Mécanisme 3 de l'ADR-007 : la vue interroge le jeu quand elle se dessine.
        let theme = AirTheme::turbo_vision();
        let line = status_line(40);
        let hotkey = theme.palette.color_pair(AirRole::StatusHotkey).foreground;
        let normal = theme.palette.color_pair(AirRole::StatusLine).foreground;
        let grey = theme.palette.color_pair(AirRole::StatusDisabled).foreground;
        let mut commands = AirCommandSet::new();
        // Permise : la lettre d'accès rapide ressort, le reste du libellé est ordinaire.
        assert_eq!(foreground_at(&line, &commands, 1), hotkey);
        assert_eq!(foreground_at(&line, &commands, 4), normal);
        // Interdite : les deux passent au gris, lettre comprise.
        commands.disable(SAVE);
        assert_eq!(foreground_at(&line, &commands, 1), grey);
        assert_eq!(foreground_at(&line, &commands, 4), grey);
        // L'item voisin, lui, n'a pas bougé.
        assert_eq!(foreground_at(&line, &commands, 9), hotkey);
        assert_eq!(foreground_at(&line, &commands, 12), normal);
    }

    #[test]
    fn la_touche_d_un_item_emet_sa_commande() {
        let mut line = status_line(40);
        let mut event = key(AirKey::Function(2));
        assert_eq!(dispatch(&mut line, &mut event), AirOutcome::Command(SAVE));
        assert!(event.is_consumed());
    }

    #[test]
    fn une_touche_sans_item_est_ignoree() {
        let mut line = status_line(40);
        let mut event = key(AirKey::Function(9));
        assert_eq!(dispatch(&mut line, &mut event), AirOutcome::Ignored);
        assert!(!event.is_consumed());
    }

    #[test]
    fn une_touche_modifiee_n_active_pas_l_item() {
        // `Ctrl`+`F2` n'est pas `F2` : l'application peut lui donner un autre sens.
        let mut line = status_line(40);
        for modifiers in [
            AirModifiers::NONE.with_control(),
            AirModifiers::NONE.with_alternate(),
            AirModifiers::NONE.with_shift(),
        ] {
            let mut event = key_with(AirKey::Function(2), modifiers);
            assert_eq!(dispatch(&mut line, &mut event), AirOutcome::Ignored);
            assert!(!event.is_consumed());
        }
    }

    #[test]
    fn la_touche_d_un_item_interdit_n_emet_rien_et_ne_consomme_pas() {
        // Si la barre consommait, la frappe disparaîtrait sans effet visible.
        let mut line = status_line(40);
        let mut commands = AirCommandSet::new();
        commands.disable(SAVE);
        let mut event = key(AirKey::Function(2));
        assert_eq!(
            dispatch_with(&mut line, &mut event, &mut commands),
            AirOutcome::Ignored
        );
        assert!(!event.is_consumed());
    }

    #[test]
    fn un_clic_sur_un_item_emet_sa_commande() {
        let mut line = status_line(40);
        let mut event = click(18);
        assert_eq!(dispatch(&mut line, &mut event), AirOutcome::Command(QUIT));
        assert!(event.is_consumed());
    }

    #[test]
    fn un_clic_sur_un_item_interdit_est_consomme_sans_commande() {
        // Le clic a bien atteint la barre : le passer à la vue du dessous serait faux.
        let mut line = status_line(40);
        let mut commands = AirCommandSet::new();
        commands.disable(QUIT);
        let mut event = click(18);
        assert_eq!(
            dispatch_with(&mut line, &mut event, &mut commands),
            AirOutcome::Consumed
        );
        assert!(event.is_consumed());
    }

    #[test]
    fn un_clic_hors_des_items_est_ignore() {
        let mut line = status_line(40);
        let mut event = click(35);
        assert_eq!(dispatch(&mut line, &mut event), AirOutcome::Ignored);
        assert!(!event.is_consumed());
    }

    #[test]
    fn un_relachement_de_souris_est_ignore() {
        let mut line = status_line(40);
        let mut event = AirEvent::new(AirInputEvent::Mouse(AirMouseEvent {
            position: point(1, 0),
            kind: AirMouseEventKind::Released(Some(AirMouseButton::Left)),
            modifiers: AirModifiers::NONE,
        }));
        assert_eq!(dispatch(&mut line, &mut event), AirOutcome::Ignored);
    }

    #[test]
    fn l_etat_est_accessible_en_ecriture() {
        // C'est par là qu'une application replace la barre quand le terminal change de
        // taille : elle est ancrée en bas, donc ses limites changent à chaque fois.
        let mut line = status_line(40);
        line.state_mut()
            .set_bounds(AirRectangle::from_parts(0, 23, 80, 1));
        assert_eq!(
            line.state().bounds(),
            AirRectangle::from_parts(0, 23, 80, 1)
        );
    }

    #[test]
    fn la_representation_de_debogage_dit_l_essentiel() {
        let text = format!("{:?}", status_line(40));
        assert!(text.contains("items: 3"), "{text}");
    }
}
