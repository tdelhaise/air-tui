// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Le dialogue de saisie — une question, une réponse, deux boutons.
//!
//! # Pourquoi il compose lui-même
//!
//! Ce dialogue tient son champ de saisie et ses deux boutons dans des champs **typés**, et
//! non dans la liste d'enfants de l'arbre. Il les dessine et leur route les événements
//! lui-même, par des peintres fils et une traduction de coordonnées — exactement ce que
//! fait [`crate::widgets::AirMenuBar`] avec son menu déroulant.
//!
//! La raison est concrète : `children` rend des `Box<dyn AirView>`, donc une fois le champ
//! inséré, plus personne ne peut lui demander son texte — et un dialogue de saisie dont on
//! ne peut pas lire la réponse ne sert à rien. Tenir le champ typé est le prix de
//! [`AirPromptDialog::text`], et c'est un prix honnête : il n'y a ni `Any`, ni
//! transtypage, ni registre d'identifiants.
//!
//! # Il n'est pas modal à lui tout seul
//!
//! La modalité n'est pas une propriété du dialogue mais du routage : c'est
//! [`crate::application::AirScreen::set_modal`] qui lui donne tous les événements. Posé
//! comme une vue ordinaire, ce dialogue se comporte comme une vue ordinaire.
//!
//! # Ce qui n'est pas là
//!
//! Le **parcours du focus** entre le champ et les boutons. Le champ a le focus, les deux
//! boutons s'activent au clic ou par `Alt`+lettre, et `Entrée` comme `Échap` concluent.
//! `Tab` demanderait une chaîne de focus à l'intérieur d'une vue qui n'a pas d'enfants,
//! donc un second mécanisme de focus à côté de celui de l'arbre — c'est une absence
//! voulue.

use crate::command::{AirCommand, CANCEL, OK};
use crate::geometry::{AirColumn, AirPoint, AirRectangle, AirRow, AirSize};
use crate::input::event::{AirInputEvent, AirKey};
use crate::theme::AirRole;
use crate::view::painter::AirPainter;
use crate::view::{
    AirBorderStyle, AirContext, AirDrawContext, AirEvent, AirOutcome, AirView, AirViewId,
    AirViewState, draw_view_shadow,
};
use crate::widgets::{AirButton, AirInputLine};

/// La hauteur de contenu d'un dialogue de saisie : étiquette, champ, vide, boutons, et la
/// ligne où tombe l'**ombre** des boutons.
///
/// La cinquième ligne est venue avec le jalon M7k, et elle n'est pas un espacement : une
/// ombre de bouton tombe hors des limites du bouton, donc dans la zone de contenu du
/// dialogue, qui en est le propriétaire (ADR-015). Sans cette ligne, le découpage
/// l'écarterait — ce qui serait juste, et invisible.
const CONTENT_ROWS: u16 = 5;

/// La ligne des boutons dans la zone de contenu.
///
/// Nommée plutôt que déduite de [`CONTENT_ROWS`] : les boutons ne sont plus sur la
/// dernière ligne depuis que l'ombre y tombe, et un `CONTENT_ROWS - 2` au site de calcul
/// obligerait à relire les deux constantes pour comprendre laquelle a changé.
const BUTTON_ROW: u16 = 3;

/// La largeur du bouton de validation, bordures de libellé comprises.
const CONFIRM_WIDTH: u16 = 10;

/// La largeur du bouton d'abandon.
const CANCEL_WIDTH: u16 = 12;

/// Un dialogue qui pose une question et rend une réponse.
#[derive(Debug)]
pub struct AirPromptDialog {
    /// L'état commun.
    state: AirViewState,
    /// Le titre, inscrit sur le cadre.
    title: String,
    /// L'étiquette au-dessus du champ.
    label: String,
    /// Le champ de saisie.
    input: AirInputLine,
    /// Le bouton de validation.
    confirm: AirButton,
    /// Le bouton d'abandon.
    cancel: AirButton,
}

impl AirPromptDialog {
    /// La hauteur qu'un dialogue de saisie demande, bordure comprise.
    ///
    /// Sept lignes : deux de cadre et cinq de contenu, dont celle de l'ombre des boutons.
    /// Exposé pour qu'un appelant calcule sa position sans deviner.
    #[must_use]
    pub const fn required_rows() -> u16 {
        CONTENT_ROWS.saturating_add(2)
    }

    /// Un dialogue de saisie, en cadre double.
    ///
    /// Les identifiants du champ et des boutons sont dérivés de celui du dialogue :
    /// l'appelant n'a qu'un identifiant à fournir, et les trois contrôles internes ne
    /// paraissent jamais dans l'arbre.
    #[must_use]
    pub fn new(
        mut state: AirViewState,
        title: impl Into<String>,
        label: impl Into<String>,
    ) -> Self {
        state.set_border_style(AirBorderStyle::Double);
        let identifier = state.identifier();
        let content = Self::content_size_of(&state);
        let input = AirInputLine::new(AirViewState::new(
            Self::derived_identifier(identifier, 1),
            AirRectangle::from_parts(0, 1, content.columns, 1),
        ));
        let confirm = AirButton::new(
            AirViewState::new(
                Self::derived_identifier(identifier, 2),
                AirRectangle::from_parts(0, BUTTON_ROW, CONFIRM_WIDTH, 1),
            ),
            "~O~K",
            OK,
        )
        .as_default();
        let cancel = AirButton::new(
            AirViewState::new(
                Self::derived_identifier(identifier, 3),
                AirRectangle::from_parts(
                    CONFIRM_WIDTH.saturating_add(2),
                    BUTTON_ROW,
                    CANCEL_WIDTH,
                    1,
                ),
            ),
            "~A~nnuler",
            CANCEL,
        );
        let mut dialog = Self {
            state,
            title: title.into(),
            label: label.into(),
            input,
            confirm,
            cancel,
        };
        // Le champ a le focus dès l'ouverture : c'est lui qui porte le curseur, et une
        // question sans curseur n'invite pas à répondre.
        dialog.input.state_mut().set_focused(true);
        dialog.relayout();
        dialog
    }

    /// Le même dialogue, avec une réponse pré-remplie.
    #[must_use]
    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.input.set_text(text);
        self
    }

    /// Le même dialogue, avec une longueur de réponse maximale.
    #[must_use]
    pub fn with_maximum_length(mut self, maximum_length: usize) -> Self {
        self.input.set_maximum_length(Some(maximum_length));
        self
    }

    /// Un identifiant dérivé de celui du dialogue.
    ///
    /// Le repli sur le premier identifiant valide ne peut se produire qu'au tout dernier
    /// identifiant représentable, et il vaut mieux qu'un `expect` dans un constructeur.
    fn derived_identifier(base: AirViewId, offset: u32) -> AirViewId {
        AirViewId::new(base.value().saturating_add(offset)).unwrap_or(AirViewId::FIRST)
    }

    /// La dimension de la zone de contenu d'un état.
    fn content_size_of(state: &AirViewState) -> AirSize {
        state
            .content_bounds()
            .map_or(AirSize::new(0, 0), |bounds| bounds.size)
    }

    /// La dimension de la zone de contenu.
    #[must_use]
    pub fn content_size(&self) -> AirSize {
        Self::content_size_of(&self.state)
    }

    /// L'origine du contenu dans les coordonnées de la vue : une cellule quand bordée.
    fn content_origin(&self) -> AirPoint {
        let offset = u16::from(self.state.has_border());
        AirPoint::new(AirColumn::new(offset), AirRow::new(offset))
    }

    /// Replace le champ et les boutons dans la zone de contenu courante.
    ///
    /// À appeler après tout changement de limites : le dialogue est centré par son
    /// propriétaire, et sa largeur peut donc changer.
    pub fn relayout(&mut self) {
        let content = self.content_size();
        self.input
            .state_mut()
            .set_bounds(AirRectangle::from_parts(0, 1, content.columns, 1));
        let row = BUTTON_ROW.min(content.rows);
        self.confirm
            .state_mut()
            .set_bounds(AirRectangle::from_parts(0, row, CONFIRM_WIDTH, 1));
        self.cancel.state_mut().set_bounds(AirRectangle::from_parts(
            CONFIRM_WIDTH.saturating_add(2),
            row,
            CANCEL_WIDTH,
            1,
        ));
    }

    /// La réponse saisie.
    #[must_use]
    pub fn text(&self) -> &str {
        self.input.text()
    }

    /// Remplace la réponse et place le curseur à la fin.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.input.set_text(text);
    }

    /// L'étiquette de la question.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// La commande de validation.
    #[must_use]
    pub fn confirm_command(&self) -> AirCommand {
        self.confirm.command()
    }

    /// La commande d'abandon.
    #[must_use]
    pub fn cancel_command(&self) -> AirCommand {
        self.cancel.command()
    }

    /// Route un événement vers un contrôle interne, en traduisant les coordonnées.
    ///
    /// La traduction est celle que fait la traversée de l'arbre : à la descente, et
    /// défaite à la remontée. Sans elle, un clic en bas d'un dialogue centré arriverait au
    /// champ avec les coordonnées de l'écran.
    fn route_to(
        control: &mut dyn AirView,
        origin: AirPoint,
        event: &mut AirEvent,
        context: &mut AirContext<'_>,
    ) -> AirOutcome {
        event.shift_into(origin);
        let outcome = control.handle_event(event, context);
        event.shift_out_of(origin);
        outcome
    }

    /// L'origine absolue d'un contrôle dans les coordonnées de la vue.
    fn origin_of(&self, control: &dyn AirView) -> AirPoint {
        let content = self.content_origin();
        let bounds = control.state().bounds();
        AirPoint::new(
            AirColumn::new(content.column.value().saturating_add(bounds.left())),
            AirRow::new(content.row.value().saturating_add(bounds.top())),
        )
    }
}

impl AirView for AirPromptDialog {
    fn state(&self) -> &AirViewState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut AirViewState {
        &mut self.state
    }

    fn title(&self) -> Option<&str> {
        if self.title.is_empty() {
            None
        } else {
            Some(&self.title)
        }
    }

    fn draw_content(&self, painter: &mut AirPainter<'_>, context: &AirDrawContext<'_>) {
        let colors = context.theme.palette.color_pair(AirRole::DialogInterior);
        let size = painter.size();
        painter.fill_rectangle(AirRectangle::new(AirPoint::ORIGIN, size), colors.cell(' '));
        painter.draw_text_with_colors(
            AirPoint::new(AirColumn::FIRST, AirRow::FIRST),
            &self.label,
            colors,
        );
        for control in [
            &self.input as &dyn AirView,
            &self.confirm as &dyn AirView,
            &self.cancel as &dyn AirView,
        ] {
            // L'ombre avant le contrôle, et par la fonction de la traversée plutôt que par
            // un calcul d'ici : ce dialogue est le **propriétaire** de la surface où elle
            // tombe, et c'est la seule chose qui le distingue d'un parent ordinaire
            // (ADR-015). Le champ de saisie n'en projette aucune, et c'est son état qui le
            // dit — pas une exception écrite ici.
            draw_view_shadow(painter, control, context);
            if let Some(mut child) = painter.child(control.state().bounds()) {
                control.draw_content(&mut child, context);
            }
        }
    }

    fn cursor(&self) -> Option<AirPoint> {
        // Le curseur du champ, ramené dans les coordonnées de la **zone de contenu** du
        // dialogue : c'est ce que la traversée attend, et elle ne le retient que si le
        // dialogue a le focus.
        let local = self.input.cursor()?;
        let bounds = self.input.state().bounds();
        Some(AirPoint::new(
            AirColumn::new(bounds.left().saturating_add(local.column.value())),
            AirRow::new(bounds.top().saturating_add(local.row.value())),
        ))
    }

    fn handle_event(&mut self, event: &mut AirEvent, context: &mut AirContext<'_>) -> AirOutcome {
        if !self.state.is_enabled() {
            return AirOutcome::Ignored;
        }
        let Some(input) = event.input() else {
            return AirOutcome::Ignored;
        };

        if let AirInputEvent::Mouse(mouse) = input {
            let position = mouse.position;
            // Du premier plan vers l'arrière n'a pas de sens ici — les trois contrôles ne
            // se recouvrent pas —, mais l'ordre reste celui de la déclaration.
            let targets: [AirPoint; 3] = [
                self.origin_of(&self.input),
                self.origin_of(&self.confirm),
                self.origin_of(&self.cancel),
            ];
            let sizes: [AirSize; 3] = [
                self.input.state().bounds().size,
                self.confirm.state().bounds().size,
                self.cancel.state().bounds().size,
            ];
            for index in 0..3 {
                let Some(origin) = targets.get(index) else {
                    continue;
                };
                let Some(size) = sizes.get(index) else {
                    continue;
                };
                if !AirRectangle::new(*origin, *size).contains(position) {
                    continue;
                }
                let outcome = match index {
                    0 => Self::route_to(&mut self.input, *origin, event, context),
                    1 => Self::route_to(&mut self.confirm, *origin, event, context),
                    _ => Self::route_to(&mut self.cancel, *origin, event, context),
                };
                if outcome != AirOutcome::Ignored {
                    return outcome;
                }
            }
            // Un clic ailleurs dans le dialogue lui appartient : le laisser passer le
            // donnerait à ce qui est derrière, ce qu'un modal ne doit jamais faire.
            event.consume();
            return AirOutcome::Consumed;
        }

        // Le champ d'abord : il ne prend ni `Ctrl` ni `Alt`, donc les raccourcis des
        // boutons lui échappent naturellement.
        let origin = self.content_origin();
        let outcome = Self::route_to(&mut self.input, origin, event, context);
        if outcome != AirOutcome::Ignored {
            return outcome;
        }
        for index in 0..2 {
            let outcome = if index == 0 {
                Self::route_to(&mut self.confirm, origin, event, context)
            } else {
                Self::route_to(&mut self.cancel, origin, event, context)
            };
            if outcome != AirOutcome::Ignored {
                return outcome;
            }
        }

        let Some(AirInputEvent::Key(key)) = event.input() else {
            return AirOutcome::Ignored;
        };
        if key.modifiers.control || key.modifiers.alternate {
            return AirOutcome::Ignored;
        }
        let command = match key.logical {
            AirKey::Enter => self.confirm.command(),
            AirKey::Escape => self.cancel.command(),
            _ => return AirOutcome::Ignored,
        };
        event.consume();
        AirOutcome::Command(command)
    }
}

#[cfg(test)]
mod tests {
    use super::AirPromptDialog;
    use crate::command::{AirCommandSet, CANCEL, OK};
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
        draw_view,
    };

    fn dialog(columns: u16) -> AirPromptDialog {
        let mut dialog = AirPromptDialog::new(
            AirViewState::new(
                AirViewId::FIRST,
                AirRectangle::from_parts(0, 0, columns, AirPromptDialog::required_rows()),
            ),
            "Ouvrir",
            "Nom du fichier :",
        );
        dialog.state_mut().set_focused(true);
        dialog
    }

    fn press(dialog: &mut AirPromptDialog, key: AirKey) -> AirOutcome {
        press_with(dialog, key, AirModifiers::default())
    }

    fn press_with(
        dialog: &mut AirPromptDialog,
        key: AirKey,
        modifiers: AirModifiers,
    ) -> AirOutcome {
        let mut commands = AirCommandSet::new();
        let mut context = AirContext {
            commands: &mut commands,
        };
        let mut event = AirEvent::new(AirInputEvent::Key(AirKeyEvent {
            modifiers,
            ..AirKeyEvent::press(key)
        }));
        dialog.handle_event(&mut event, &mut context)
    }

    fn click(dialog: &mut AirPromptDialog, column: u16, row: u16) -> AirOutcome {
        let mut commands = AirCommandSet::new();
        let mut context = AirContext {
            commands: &mut commands,
        };
        let mut event = AirEvent::new(AirInputEvent::Mouse(AirMouseEvent {
            position: AirPoint::new(AirColumn::new(column), AirRow::new(row)),
            kind: AirMouseEventKind::Pressed(AirMouseButton::Left),
            modifiers: AirModifiers::default(),
        }));
        dialog.handle_event(&mut event, &mut context)
    }

    fn render(dialog: &AirPromptDialog, size: AirSize) -> String {
        let mut surface = AirSurface::new(size);
        let theme = AirTheme::turbo_vision();
        let commands = AirCommandSet::new();
        {
            let mut painter = AirPainter::new(&mut surface);
            draw_view(
                dialog,
                &mut painter,
                &AirDrawContext::new(&theme, &commands),
            );
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

    fn cursor_of(dialog: &AirPromptDialog, size: AirSize) -> Option<AirPoint> {
        let mut surface = AirSurface::new(size);
        let theme = AirTheme::turbo_vision();
        let commands = AirCommandSet::new();
        let mut painter = AirPainter::new(&mut surface);
        draw_view(
            dialog,
            &mut painter,
            &AirDrawContext::new(&theme, &commands),
        )
    }

    #[test]
    fn un_dialogue_demande_sept_lignes() {
        // Cinq de contenu — dont celle où tombe l'ombre des boutons — et deux de cadre.
        assert_eq!(AirPromptDialog::required_rows(), 7);
    }

    #[test]
    fn un_dialogue_peint_l_ombre_de_ses_deux_boutons() {
        // Ce dialogue compose ses contrôles lui-même : ils ne paraissent pas dans ses
        // enfants, donc la traversée ne voit pas leurs ombres. C'est lui le propriétaire
        // de la surface où elles tombent, et c'est lui qui les peint (ADR-015).
        let rendered = render(
            &dialog(30),
            AirSize::new(30, AirPromptDialog::required_rows()),
        );
        let lines: Vec<&str> = rendered.lines().collect();
        let buttons = lines.get(4).copied().unwrap_or_default();
        let under = lines.get(5).copied().unwrap_or_default();
        assert_eq!(
            buttons.matches('▄').count(),
            2,
            "un demi-bloc inférieur à droite de chaque bouton — ligne « {buttons} »"
        );
        // Dix et douze colonnes de bouton, donc dix et douze demi-blocs dessous.
        assert_eq!(
            under.matches('▀').count(),
            22,
            "les deux bandes du dessous — ligne « {under} »"
        );
    }

    #[test]
    fn un_dialogue_est_en_cadre_double() {
        use crate::view::AirBorderStyle;
        assert_eq!(dialog(30).state().border_style(), AirBorderStyle::Double);
        assert!(dialog(30).state().has_border());
    }

    #[test]
    fn le_titre_et_l_etiquette_sont_ceux_qu_on_a_donnes() {
        let dialog = dialog(30);
        assert_eq!(dialog.title(), Some("Ouvrir"));
        assert_eq!(dialog.label(), "Nom du fichier :");
    }

    #[test]
    fn un_titre_vide_ne_s_inscrit_pas_sur_le_cadre() {
        let dialog = AirPromptDialog::new(
            AirViewState::new(AirViewId::FIRST, AirRectangle::from_parts(0, 0, 20, 6)),
            "",
            "Question",
        );
        assert_eq!(dialog.title(), None);
    }

    #[test]
    fn le_dialogue_se_dessine_avec_sa_question_son_champ_et_ses_boutons() {
        let dialog = dialog(30).with_text("fichier.txt");
        let image = render(&dialog, AirSize::new(30, 6));
        let lines: Vec<&str> = image.lines().collect();
        assert_eq!(lines.len(), 6);
        assert!(lines.first().is_some_and(|line| line.contains("Ouvrir")));
        assert!(
            lines
                .get(1)
                .is_some_and(|line| line.contains("Nom du fichier :")),
            "rendu : {image}"
        );
        assert!(
            lines
                .get(2)
                .is_some_and(|line| line.contains("fichier.txt")),
            "rendu : {image}"
        );
        assert!(
            lines.get(4).is_some_and(|line| line.contains("OK")),
            "rendu : {image}"
        );
        assert!(
            lines.get(4).is_some_and(|line| line.contains("Annuler")),
            "rendu : {image}"
        );
    }

    #[test]
    fn la_saisie_va_au_champ() {
        let mut dialog = dialog(30);
        for character in "essai".chars() {
            assert_eq!(
                press(&mut dialog, AirKey::Character(character)),
                AirOutcome::Consumed
            );
        }
        assert_eq!(dialog.text(), "essai");
    }

    #[test]
    fn une_reponse_pre_remplie_est_lisible_et_remplaçable() {
        let mut dialog = dialog(30).with_text("un");
        assert_eq!(dialog.text(), "un");
        dialog.set_text("deux");
        assert_eq!(dialog.text(), "deux");
    }

    #[test]
    fn une_longueur_maximale_borne_la_reponse() {
        let dialog = dialog(30).with_maximum_length(3).with_text("abcdef");
        assert_eq!(dialog.text(), "abc");
    }

    #[test]
    fn entree_valide_et_echappement_abandonne() {
        let mut dialog = dialog(30);
        assert_eq!(press(&mut dialog, AirKey::Enter), AirOutcome::Command(OK));
        assert_eq!(
            press(&mut dialog, AirKey::Escape),
            AirOutcome::Command(CANCEL)
        );
        assert_eq!(dialog.confirm_command(), OK);
        assert_eq!(dialog.cancel_command(), CANCEL);
    }

    #[test]
    fn les_lettres_d_acces_rapide_des_boutons_agissent() {
        let mut dialog = dialog(30);
        let alternate = AirModifiers {
            alternate: true,
            ..AirModifiers::default()
        };
        assert_eq!(
            press_with(&mut dialog, AirKey::Character('o'), alternate),
            AirOutcome::Command(OK)
        );
        assert_eq!(
            press_with(&mut dialog, AirKey::Character('a'), alternate),
            AirOutcome::Command(CANCEL)
        );
    }

    #[test]
    fn un_clic_sur_un_bouton_emet_sa_commande() {
        let mut dialog = dialog(30);
        // Ligne 4 de la vue : deux de contenu plus la bordure. Le bouton OK commence en
        // colonne 1.
        assert_eq!(click(&mut dialog, 2, 4), AirOutcome::Command(OK));
        assert_eq!(click(&mut dialog, 15, 4), AirOutcome::Command(CANCEL));
    }

    #[test]
    fn un_clic_dans_le_champ_y_place_le_curseur() {
        let mut dialog = dialog(30).with_text("abcdef");
        assert_eq!(click(&mut dialog, 3, 2), AirOutcome::Consumed);
        // Colonne 3 de la vue, moins la bordure : la troisième grappe.
        assert_eq!(
            cursor_of(&dialog, AirSize::new(30, 6)),
            Some(AirPoint::new(AirColumn::new(3), AirRow::new(2)))
        );
    }

    #[test]
    fn un_clic_ailleurs_dans_le_dialogue_est_consomme() {
        // Un modal ne laisse jamais passer un clic à ce qui est derrière.
        let mut dialog = dialog(30);
        assert_eq!(click(&mut dialog, 20, 1), AirOutcome::Consumed);
    }

    #[test]
    fn le_curseur_est_celui_du_champ_en_coordonnees_de_contenu() {
        let dialog = dialog(30).with_text("abc");
        assert_eq!(
            cursor_of(&dialog, AirSize::new(30, 6)),
            Some(AirPoint::new(AirColumn::new(4), AirRow::new(2)))
        );
    }

    #[test]
    fn un_dialogue_desactive_ignore_tout() {
        let mut dialog = dialog(30);
        dialog.state_mut().set_enabled(false);
        assert_eq!(press(&mut dialog, AirKey::Enter), AirOutcome::Ignored);
        assert_eq!(click(&mut dialog, 2, 4), AirOutcome::Ignored);
    }

    #[test]
    fn une_touche_sans_emploi_est_ignoree() {
        let mut dialog = dialog(30);
        assert_eq!(press(&mut dialog, AirKey::Function(5)), AirOutcome::Ignored);
    }

    #[test]
    fn une_frappe_avec_controle_est_laissee_aux_raccourcis() {
        let mut dialog = dialog(30);
        let control = AirModifiers {
            control: true,
            ..AirModifiers::default()
        };
        assert_eq!(
            press_with(&mut dialog, AirKey::Enter, control),
            AirOutcome::Ignored
        );
    }

    #[test]
    fn un_redimensionnement_replace_le_champ_sur_toute_la_largeur() {
        let mut dialog = dialog(30);
        dialog
            .state_mut()
            .set_bounds(AirRectangle::from_parts(0, 0, 50, 6));
        dialog.relayout();
        assert_eq!(dialog.content_size(), AirSize::new(48, 4));
        let image = render(&dialog, AirSize::new(50, 6));
        assert!(image.lines().count() == 6, "rendu sur six lignes : {image}");
    }

    #[test]
    fn un_dialogue_sans_zone_de_contenu_ne_panique_pas() {
        let dialog = AirPromptDialog::new(
            AirViewState::new(AirViewId::FIRST, AirRectangle::from_parts(0, 0, 1, 1)),
            "Titre",
            "Question",
        );
        assert_eq!(dialog.content_size(), AirSize::new(0, 0));
        assert_eq!(render(&dialog, AirSize::new(1, 1)), " ");
    }
}
