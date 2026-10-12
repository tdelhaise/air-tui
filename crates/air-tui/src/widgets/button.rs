// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Le bouton — un libellé centré qui émet une commande.

use crate::command::AirCommand;
use crate::geometry::{AirColumn, AirPoint, AirRectangle, AirRow};
use crate::input::event::{AirInputEvent, AirKey, AirMouseButton, AirMouseEventKind};
use crate::theme::{AirRole, AirShadowStyle};
use crate::view::painter::AirPainter;
use crate::view::{AirContext, AirDrawContext, AirEvent, AirOutcome, AirView, AirViewState};
use crate::widgets::hotkey::split_hotkey;

/// Un bouton.
///
/// **Sans bordure** : c'est un petit contrôle, et l'encadrer ne laisserait plus de place au
/// libellé (ADR-010). Le constructeur retire donc le défaut, explicitement.
///
/// # Ce qui l'active
///
/// Un clic, `Entrée` ou `Espace` quand il a le focus, et sa lettre d'accès rapide avec la
/// touche alternative — `Alt+O` pour `~O~K`. Les trois chemins mènent à la même commande,
/// et c'est voulu : un raccourci qui ferait autre chose que le clic serait un piège.
pub struct AirButton {
    /// L'état commun.
    state: AirViewState,
    /// Le libellé, notation `~X~` comprise.
    label: String,
    /// La commande émise.
    command: AirCommand,
    /// Vrai si c'est le bouton par défaut, celui qu'`Entrée` active de loin.
    default: bool,
}

impl AirButton {
    /// Un bouton sans bordure.
    #[must_use]
    pub fn new(mut state: AirViewState, label: impl Into<String>, command: AirCommand) -> Self {
        state.set_border(false);
        // Un bouton projette une ombre, et son propriétaire la peint : elle tombe hors de
        // ses limites (ADR-015). Le **genre** suffit ici — c'est le thème qui décide de
        // son apparence, et celui de Midnight Commander n'en dessine aucune.
        state.set_shadow_style(Some(AirShadowStyle::Button));
        Self {
            state,
            label: label.into(),
            command,
            default: false,
        }
    }

    /// Le même bouton, désigné comme bouton par défaut.
    #[must_use]
    pub const fn as_default(mut self) -> Self {
        self.default = true;
        self
    }

    /// Le libellé, notation comprise.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// La commande émise.
    #[must_use]
    pub const fn command(&self) -> AirCommand {
        self.command
    }

    /// Vrai si c'est le bouton par défaut.
    #[must_use]
    pub const fn is_default(&self) -> bool {
        self.default
    }

    /// Le rôle de couleur du libellé, selon l'état.
    ///
    /// L'ordre des conditions est celui de la lisibilité : un bouton désactivé l'est quel
    /// que soit son focus, et un bouton focalisé prime sur le fait d'être par défaut.
    fn label_role(&self, enabled: bool) -> AirRole {
        if !enabled {
            return AirRole::ButtonDisabled;
        }
        if self.state.is_focused() {
            return AirRole::ButtonFocused;
        }
        if self.default {
            return AirRole::ButtonDefault;
        }
        AirRole::ButtonNormal
    }
}

impl AirView for AirButton {
    fn state(&self) -> &AirViewState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut AirViewState {
        &mut self.state
    }

    fn draw_content(&self, painter: &mut AirPainter<'_>, context: &AirDrawContext<'_>) {
        // Deux raisons de griser, et il faut les deux : la vue peut être désactivée
        // elle-même, et sa commande peut l'être globalement (ADR-007, mécanisme 3).
        let enabled = self.state.is_enabled() && context.is_command_enabled(self.command);
        let colors = context.theme.palette.color_pair(self.label_role(enabled));
        let size = painter.size();
        painter.fill_rectangle(AirRectangle::new(AirPoint::ORIGIN, size), colors.cell(' '));

        let parts = split_hotkey(&self.label);
        // Le libellé est centré. La largeur visible exclut les tildes, qui ne s'affichent
        // pas : les compter décalerait le centrage d'un ou deux caractères.
        let visible = crate::unicode::text_width(parts.before)
            .saturating_add(parts.hotkey.map_or(0, crate::unicode::text_width))
            .saturating_add(crate::unicode::text_width(parts.after));
        let visible = u16::try_from(visible).unwrap_or(u16::MAX);
        let free = size.columns.saturating_sub(visible);
        let row = AirRow::new(size.rows / 2);
        let mut column = AirColumn::new(free / 2);

        // La lettre d'accès rapide reste lisible même désactivée : la griser avec le reste
        // vaut mieux que de la laisser en jaune sur un bouton inactif.
        let hotkey_colors = if enabled {
            context.theme.palette.color_pair(AirRole::ButtonHotkey)
        } else {
            colors
        };
        column = painter.draw_text_with_colors(AirPoint::new(column, row), parts.before, colors);
        if let Some(hotkey) = parts.hotkey {
            column =
                painter.draw_text_with_colors(AirPoint::new(column, row), hotkey, hotkey_colors);
        }
        painter.draw_text_with_colors(AirPoint::new(column, row), parts.after, colors);
    }

    fn handle_event(&mut self, event: &mut AirEvent, context: &mut AirContext<'_>) -> AirOutcome {
        if !self.state.is_enabled() {
            return AirOutcome::Ignored;
        }
        let Some(input) = event.input() else {
            return AirOutcome::Ignored;
        };
        let activated = match input {
            AirInputEvent::Mouse(mouse) => {
                matches!(mouse.kind, AirMouseEventKind::Pressed(AirMouseButton::Left))
            }
            AirInputEvent::Key(key) => {
                let is_activation =
                    matches!(key.shortcut_key(), AirKey::Enter | AirKey::Character(' '));
                let focused_activation =
                    self.state.is_focused() && is_activation && key.modifiers.is_empty();
                let hotkey_activation = key.modifiers.alternate
                    && match key.shortcut_key() {
                        AirKey::Character(character) => {
                            split_hotkey(&self.label).hotkey_character()
                                == Some(character.to_ascii_lowercase())
                        }
                        _ => false,
                    };
                focused_activation || hotkey_activation
            }
            _ => false,
        };
        if !activated {
            return AirOutcome::Ignored;
        }
        if !context.commands.is_enabled(self.command) {
            // La commande est grisée : on consomme pour que personne d'autre n'interprète
            // le geste, mais on n'émet rien.
            event.consume();
            return AirOutcome::Consumed;
        }
        event.consume();
        AirOutcome::Command(self.command)
    }
}

impl core::fmt::Debug for AirButton {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("AirButton")
            .field("label", &self.label)
            .field("command", &self.command)
            .field("default", &self.default)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::AirButton;
    use crate::command::{AirCommandSet, OK, SAVE};
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

    fn state(bounds: AirRectangle) -> AirViewState {
        AirViewState::new(AirViewId::FIRST, bounds)
    }

    fn point(column: u16, row: u16) -> AirPoint {
        AirPoint::new(AirColumn::new(column), AirRow::new(row))
    }

    fn button(label: &str) -> AirButton {
        AirButton::new(state(AirRectangle::from_parts(0, 0, 10, 1)), label, OK)
    }

    fn dispatch(view: &mut dyn AirView, event: &mut AirEvent) -> AirOutcome {
        let mut commands = AirCommandSet::new();
        let mut context = AirContext {
            commands: &mut commands,
        };
        dispatch_event(view, event, &mut context)
    }

    fn dispatch_with(
        view: &mut dyn AirView,
        event: &mut AirEvent,
        commands: &mut AirCommandSet,
    ) -> AirOutcome {
        let mut context = AirContext { commands };
        dispatch_event(view, event, &mut context)
    }

    fn key(logical: AirKey, modifiers: AirModifiers) -> AirEvent {
        AirEvent::new(AirInputEvent::Key(
            AirKeyEvent::press(logical).with_modifiers(modifiers),
        ))
    }

    fn click() -> AirEvent {
        AirEvent::new(AirInputEvent::Mouse(AirMouseEvent {
            position: point(1, 0),
            kind: AirMouseEventKind::Pressed(AirMouseButton::Left),
            modifiers: AirModifiers::NONE,
        }))
    }

    fn rendered(surface: &AirSurface, size: AirSize) -> String {
        (0..size.columns)
            .map(|column| {
                surface
                    .cell(point(column, 0))
                    .map_or(' ', |found| found.character)
            })
            .collect()
    }

    #[test]
    fn un_bouton_retire_la_bordure_dans_son_constructeur() {
        // Corollaire d'ADR-010 : un bouton encadré n'aurait plus de place pour son libellé.
        assert!(!button("~O~K").state().has_border());
    }

    #[test]
    fn les_accesseurs_rendent_ce_qui_a_ete_posé() {
        let plain = button("~O~K");
        assert_eq!(plain.label(), "~O~K");
        assert_eq!(plain.command(), OK);
        assert!(!plain.is_default());
        assert!(button("~O~K").as_default().is_default());
    }

    #[test]
    fn le_libelle_est_centre_sans_compter_les_tildes() {
        // Les tildes ne s'affichent pas : les compter décalerait le centrage.
        let theme = AirTheme::turbo_vision();
        let size = AirSize::new(10, 1);
        let mut surface = AirSurface::new(size);
        let view = AirButton::new(state(AirRectangle::new(AirPoint::ORIGIN, size)), "~O~K", OK);
        {
            let mut painter = AirPainter::new(&mut surface);
            draw_view(
                &view,
                &mut painter,
                &AirDrawContext::new(&theme, &AirCommandSet::new()),
            );
        }
        // Dix colonnes, deux caractères visibles : quatre espaces de chaque côté.
        assert_eq!(rendered(&surface, size), "    OK    ");
    }

    #[test]
    fn la_lettre_d_acces_rapide_porte_sa_propre_couleur() {
        let theme = AirTheme::turbo_vision();
        let size = AirSize::new(6, 1);
        let mut surface = AirSurface::new(size);
        let view = AirButton::new(state(AirRectangle::new(AirPoint::ORIGIN, size)), "~O~K", OK);
        {
            let mut painter = AirPainter::new(&mut surface);
            draw_view(
                &view,
                &mut painter,
                &AirDrawContext::new(&theme, &AirCommandSet::new()),
            );
        }
        let hotkey = theme.palette.color_pair(AirRole::ButtonHotkey);
        let normal = theme.palette.color_pair(AirRole::ButtonNormal);
        assert_eq!(
            surface.cell(point(2, 0)).map(|found| found.foreground),
            Some(hotkey.foreground),
            "le O doit être jaune"
        );
        assert_eq!(
            surface.cell(point(3, 0)).map(|found| found.foreground),
            Some(normal.foreground),
            "le K doit être noir"
        );
    }

    #[test]
    fn chaque_etat_a_son_role_de_couleur() {
        let theme = AirTheme::turbo_vision();
        let size = AirSize::new(6, 1);
        let cases = [
            (false, false, false, AirRole::ButtonNormal),
            (true, false, false, AirRole::ButtonFocused),
            (false, true, false, AirRole::ButtonDefault),
            // Le focus prime sur le défaut.
            (true, true, false, AirRole::ButtonFocused),
            // Désactivé prime sur tout.
            (true, true, true, AirRole::ButtonDisabled),
        ];
        for (focused, default, disabled, expected) in cases {
            let mut surface = AirSurface::new(size);
            let mut view =
                AirButton::new(state(AirRectangle::new(AirPoint::ORIGIN, size)), "ab", OK);
            if default {
                view = view.as_default();
            }
            view.state_mut().set_focused(focused);
            view.state_mut().set_enabled(!disabled);
            {
                let mut painter = AirPainter::new(&mut surface);
                draw_view(
                    &view,
                    &mut painter,
                    &AirDrawContext::new(&theme, &AirCommandSet::new()),
                );
            }
            let colors = theme.palette.color_pair(expected);
            assert_eq!(
                surface.cell(point(0, 0)).map(|found| found.background),
                Some(colors.background),
                "pour focus={focused} défaut={default} désactivé={disabled}"
            );
        }
    }

    #[test]
    fn un_clic_emet_la_commande() {
        let mut view = button("~O~K");
        let mut event = click();
        assert_eq!(dispatch(&mut view, &mut event), AirOutcome::Command(OK));
        assert!(event.is_consumed());
    }

    #[test]
    fn entree_et_espace_activent_le_bouton_qui_a_le_focus() {
        for logical in [AirKey::Enter, AirKey::Character(' ')] {
            let mut view = button("~O~K");
            view.state_mut().set_focused(true);
            let mut event = key(logical, AirModifiers::NONE);
            assert_eq!(
                dispatch(&mut view, &mut event),
                AirOutcome::Command(OK),
                "pour {logical:?}"
            );
        }
    }

    #[test]
    fn entree_n_active_pas_un_bouton_sans_focus() {
        let mut view = button("~O~K");
        let mut event = key(AirKey::Enter, AirModifiers::NONE);
        assert_eq!(dispatch(&mut view, &mut event), AirOutcome::Ignored);
    }

    #[test]
    fn la_lettre_d_acces_rapide_active_le_bouton_de_loin() {
        // Sans focus, et sans tenir compte de la casse.
        for character in ['o', 'O'] {
            let mut view = button("~O~K");
            let mut event = key(
                AirKey::Character(character),
                AirModifiers::NONE.with_alternate(),
            );
            assert_eq!(
                dispatch(&mut view, &mut event),
                AirOutcome::Command(OK),
                "pour {character}"
            );
        }
    }

    #[test]
    fn une_autre_lettre_avec_la_touche_alternative_n_active_rien() {
        let mut view = button("~O~K");
        let mut event = key(AirKey::Character('k'), AirModifiers::NONE.with_alternate());
        assert_eq!(dispatch(&mut view, &mut event), AirOutcome::Ignored);
    }

    #[test]
    fn la_lettre_sans_la_touche_alternative_n_active_rien() {
        let mut view = button("~O~K");
        let mut event = key(AirKey::Character('o'), AirModifiers::NONE);
        assert_eq!(dispatch(&mut view, &mut event), AirOutcome::Ignored);
    }

    #[test]
    fn un_bouton_sans_lettre_ignore_la_touche_alternative() {
        let mut view = button("OK");
        let mut event = key(AirKey::Character('o'), AirModifiers::NONE.with_alternate());
        assert_eq!(dispatch(&mut view, &mut event), AirOutcome::Ignored);
    }

    #[test]
    fn une_touche_non_caractere_avec_la_touche_alternative_n_active_rien() {
        let mut view = button("~O~K");
        let mut event = key(AirKey::Up, AirModifiers::NONE.with_alternate());
        assert_eq!(dispatch(&mut view, &mut event), AirOutcome::Ignored);
    }

    #[test]
    fn un_bouton_desactive_n_emet_rien() {
        let mut view = button("~O~K");
        view.state_mut().set_enabled(false);
        // La distribution s'arrête avant le bouton : une vue désactivée ne reçoit rien.
        let mut event = click();
        assert_eq!(dispatch(&mut view, &mut event), AirOutcome::Ignored);
        assert!(!event.is_consumed());
    }

    #[test]
    fn une_commande_grisee_consomme_le_geste_sans_rien_emettre() {
        // Laisser le geste poursuivre ferait réagir ce qui se trouve dessous, alors que
        // l'utilisateur visait un bouton grisé.
        let mut commands = AirCommandSet::new();
        commands.disable(OK);
        let mut view = button("~O~K");
        let mut event = click();
        assert_eq!(
            dispatch_with(&mut view, &mut event, &mut commands),
            AirOutcome::Consumed
        );
        assert!(event.is_consumed());
    }

    #[test]
    fn un_evenement_sans_charge_utile_est_ignore() {
        let mut view = button("~O~K");
        let mut event = click();
        event.consume();
        assert_eq!(dispatch(&mut view, &mut event), AirOutcome::Ignored);
    }

    #[test]
    fn un_evenement_qui_n_est_ni_touche_ni_souris_est_ignore() {
        let mut view = button("~O~K");
        let mut event = AirEvent::new(AirInputEvent::FocusGained);
        assert_eq!(dispatch(&mut view, &mut event), AirOutcome::Ignored);
    }

    #[test]
    fn la_commande_emise_est_celle_du_bouton() {
        let mut view = AirButton::new(state(AirRectangle::from_parts(0, 0, 10, 1)), "~E~", SAVE);
        let mut event = click();
        assert_eq!(dispatch(&mut view, &mut event), AirOutcome::Command(SAVE));
    }

    #[test]
    fn l_affichage_mentionne_le_libelle_et_la_commande() {
        let text = format!("{:?}", button("~O~K"));
        assert!(text.contains("O~K"), "rendu : {text}");
        assert!(text.contains("command"), "rendu : {text}");
    }
}
