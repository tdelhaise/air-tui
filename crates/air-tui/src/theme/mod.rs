// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Les thèmes — palette et glyphes.
//!
//! # État
//!
//! **Deux thèmes, et c'est ce qui fait de ce module un moteur.** La palette indexée par
//! rôles sémantiques — **trente-huit** rôles —, le jeu de glyphes et le quantificateur de
//! couleurs existent depuis le jalon M5 ; le thème Midnight Commander et l'importeur de
//! skins qui le produit, depuis M7a ; les quatre rôles de liste, depuis M7b ; ceux du
//! texte sélectionné, depuis M7d ; ceux des cases à cocher et des boutons radio, depuis
//! M7f.
//!
//! Depuis le jalon M7g, un rôle porte aussi des **attributs** — graisse, italique,
//! souligné —, et ce n'est pas un enrichissement gratuit : vingt des trente-neuf skins
//! livrés avec `mc` 4.8.33 en posent un sur une clef que la table de correspondance
//! emploie, et l'importeur les refusait tous pour cette seule raison. Un test de ce module
//! peint chaque widget avec une palette dont **tout** est gras et vérifie qu'aucune
//! cellule ne perd l'attribut en route : c'est le seul défaut de ce jalon qui serait
//! invisible, puisque ni l'un ni l'autre des deux thèmes livrés n'en pose.
//!
//! Ces quatre derniers rôles sont la première **extension** de la palette depuis que le
//! second thème existe, et c'est à cette occasion que le moteur a été éprouvé plutôt que
//! décrit : ajouter un rôle demande une valeur dans chacun des deux thèmes, et celle du
//! thème compilé vient d'une clef du skin que la table de correspondance d'`xtask`
//! désigne — pas d'une valeur écrite à la main dans l'artefact.
//!
//! Un thème est un **couple** `palette` + `glyphs`. L'ADR-008 décrit un **triplet**, la
//! troisième assiette étant `AirDecorations` — ombres, style de trait, style des touches
//! rapides. Elle n'existe pas, et l'écart est assumé plutôt que comblé par une structure
//! vide : le style de trait vit déjà dans le jeu de glyphes, qui porte un cadre de
//! fenêtre et un cadre de dialogue distincts, et l'ombre portée ne peut pas être dessinée
//! aujourd'hui — elle tombe **hors** des limites de la vue qui la projette, ce que le
//! découpage du peintre refuse sans exception (M4, M5b). Tant qu'aucune vue ne dessine
//! d'ombre, une assiette qui en décrirait une annoncerait une capacité absente.
//!
//! Le rôle `AirRole::ButtonShadow` est dans la palette pour la même raison inverse : il
//! est transcrit de Borland et de `mc`, et **aucun widget ne le lit** encore.

pub mod glyphs;
pub mod palette;
pub mod quantize;
pub(crate) mod skins;

pub use glyphs::{AirClusterGlyphs, AirFrameGlyphs, AirGlyphSet};
pub use palette::{AirColorPair, AirPalette, AirRole};
pub use quantize::{AirColorDepth, quantize};

/// Un thème complet.
///
/// Les deux assiettes sont séparées parce que ce qui fait reconnaître un look tient autant
/// aux glyphes qu'aux couleurs : le `╔═╗` double trait des dialogues de Turbo Vision est
/// aussi caractéristique que son fond gris clair — et le skin par défaut de Midnight
/// Commander, lui, dessine ses dialogues en trait **simple**, ce qui se voit d'un coup
/// d'œil.
#[derive(Debug, Clone)]
pub struct AirTheme {
    /// Les couleurs, indexées par rôle sémantique.
    pub palette: AirPalette,
    /// Les caractères de dessin.
    pub glyphs: AirGlyphSet,
}

impl AirTheme {
    /// Le thème par défaut : celui de Turbo Vision.
    #[must_use]
    pub fn turbo_vision() -> Self {
        Self {
            palette: AirPalette::turbo_vision(),
            glyphs: AirGlyphSet::turbo_vision(),
        }
    }

    /// Le thème de Midnight Commander, compilé depuis son skin par défaut.
    ///
    /// Un avertissement que l'ADR-008 écrit noir sur blanc : **ce thème ne suffit pas à
    /// ressembler à Midnight Commander**. Ce qui fait reconnaître `mc` est sa coquille —
    /// deux panneaux, ligne de commande, barre de touches étiquetée —, et une coquille
    /// n'est pas un thème. Appliqué au chrome de `redit`, ce thème donne un éditeur aux
    /// couleurs de `mc`, pas `mc`.
    #[must_use]
    pub fn midnight_commander() -> Self {
        Self {
            palette: AirPalette::midnight_commander(),
            glyphs: AirGlyphSet::midnight_commander(),
        }
    }

    /// La description que le skin d'origine porte, pour le thème de Midnight Commander.
    #[must_use]
    pub const fn midnight_commander_description() -> &'static str {
        skins::midnight_commander::DESCRIPTION
    }
}

impl Default for AirTheme {
    fn default() -> Self {
        Self::turbo_vision()
    }
}

#[cfg(test)]
mod tests {
    use super::AirTheme;
    use crate::cell::{AirAttributes, AirCell, AirColor};
    use crate::command::{AirCommandSet, OK};
    use crate::geometry::{AirColumn, AirPoint, AirRectangle, AirRow};
    use crate::input::event::AirKey;
    use crate::model::text::{AirTextAction, AirTextBuffer};
    use crate::model::{AirChangeSet, AirModel};
    use crate::surface::AirSurface;
    use crate::theme::glyphs::AirGlyphSet;
    use crate::theme::palette::{AirColorPair, AirPalette, AirPaletteParts, AirRole};
    use crate::view::AirDrawContext;
    use crate::view::painter::AirPainter;
    use crate::view::{AirView, AirViewId, AirViewState, draw_view};
    use crate::widgets::{
        AirButton, AirCluster, AirClusterItem, AirEditorView, AirInputLine, AirListBox,
        AirListItem, AirMenu, AirMenuBar, AirMenuItem, AirOrientation, AirPanel, AirPromptDialog,
        AirScrollBar, AirStaticText, AirStatusItem, AirStatusLine, AirWindow,
    };

    /// Une palette dont les trente-huit rôles portent le même attribut.
    ///
    /// Elle n'a aucun intérêt visuel — tout y est gris clair sur bleu et en graisse — et
    /// c'est précisément ce qui la rend utile : si un widget peint une cellule sans
    /// l'attribut de son rôle, il est le seul à produire une cellule non grasse.
    fn bold_everywhere() -> AirPalette {
        let pair = AirColorPair::new(AirColor::LIGHT_GRAY, AirColor::BLUE)
            .with_attributes(AirAttributes::NONE.with_bold());
        AirPalette::from_parts(&AirPaletteParts {
            desktop_background: pair,
            menu: [pair; 6],
            status: [pair; 3],
            window: [pair; 6],
            dialog: [pair; 4],
            button: [pair; 6],
            scroll_bar: [pair; 2],
            list: [pair; 4],
            editor: [pair; 1],
            input_line: [pair; 2],
            cluster: [pair; 3],
        })
    }

    /// La cellule qu'aucun widget ne peut produire, pour distinguer « peint » de
    /// « jamais touché ».
    ///
    /// Un fond jamais écrit garde la cellule par défaut, et un widget qui peint une espace
    /// sur du gris clair produirait la même chose : sans témoin, le test ne saurait pas
    /// quelles cellules ont été peintes. Les deux vraies couleurs sont là pour cela — la
    /// palette de l'essai n'emploie que des index.
    fn witness() -> AirCell {
        AirCell::new(
            '\u{a7}',
            AirColor::Rgb {
                red: 1,
                green: 2,
                blue: 3,
            },
            AirColor::Rgb {
                red: 4,
                green: 5,
                blue: 6,
            },
        )
    }

    /// Un état de vue aux dimensions données, à l'origine, et focalisé.
    ///
    /// Focalisé pour que les rôles « sous le curseur » soient atteints eux aussi : un
    /// groupe sans focus ne dessine pas son curseur, et le rôle resterait non peint.
    fn focused_state(columns: u16, rows: u16) -> AirViewState {
        let mut state = AirViewState::new(
            AirViewId::new(1).unwrap_or(AirViewId::FIRST),
            AirRectangle::from_parts(0, 0, columns, rows),
        );
        state.set_focused(true);
        state
    }

    /// Les vues à éprouver, chacune avec le nom qui paraîtra dans le message d'échec.
    fn every_widget() -> Vec<(&'static str, Box<dyn AirView>)> {
        let mut editor = AirEditorView::new(focused_state(20, 4));
        let mut buffer = AirTextBuffer::from_text("une ligne\nune autre");
        // Tout sélectionner pour que `EditorSelected` soit peint lui aussi : sans
        // sélection, ce rôle ne toucherait aucune cellule et sortirait de la mesure.
        let mut changes = AirChangeSet::new();
        buffer.apply(AirTextAction::SelectAll, &mut changes);
        editor.synchronize(&buffer);

        let mut list = AirListBox::new(focused_state(20, 4));
        list.set_items(vec![
            AirListItem::new("premier").with_detail("2 o").marked(),
            AirListItem::new("second"),
        ]);

        vec![
            (
                "AirStaticText",
                Box::new(AirStaticText::new(focused_state(12, 1), "texte")),
            ),
            (
                "AirPanel",
                Box::new(AirPanel::new(focused_state(12, 3))) as Box<dyn AirView>,
            ),
            (
                "AirWindow",
                Box::new(AirWindow::new(focused_state(14, 3)).with_title("titre")),
            ),
            (
                "AirButton",
                Box::new(AirButton::new(focused_state(12, 1), "~O~K", OK).as_default()),
            ),
            (
                "AirInputLine",
                Box::new(AirInputLine::new(focused_state(12, 1)).with_text("saisie")),
            ),
            (
                "AirMenuBar",
                Box::new(AirMenuBar::new(focused_state(20, 1)).with_menu(
                    AirMenu::new("~F~ichier").with_item(AirMenuItem::new("~O~uvrir", OK)),
                )),
            ),
            (
                "AirStatusLine",
                Box::new(
                    AirStatusLine::new(focused_state(20, 1)).with_item(AirStatusItem::new(
                        "~F2~ Enregistrer",
                        AirKey::Function(2),
                        OK,
                    )),
                ),
            ),
            (
                "AirScrollBar",
                Box::new(AirScrollBar::new(
                    focused_state(1, 6),
                    AirOrientation::Vertical,
                )),
            ),
            (
                "AirCluster",
                Box::new(
                    AirCluster::check_boxes(focused_state(18, 2))
                        .with_item(AirClusterItem::new("~G~ras").checked())
                        .with_item(AirClusterItem::new("~I~talique")),
                ),
            ),
            ("AirListBox", Box::new(list)),
            ("AirEditorView", Box::new(editor)),
            (
                "AirPromptDialog",
                Box::new(AirPromptDialog::new(focused_state(24, 7), "Titre", "Nom :")),
            ),
        ]
    }

    #[test]
    fn l_attribut_d_un_role_atteint_toutes_les_cellules_qu_un_widget_peint() {
        // Le jalon M7g a ajouté un attribut par rôle, et le défaut qu'il rend possible est
        // silencieux : un widget qui construit sa cellule à partir des deux champs de
        // couleur du couple laisse l'attribut au bord du chemin, et rien ne le dit — le
        // thème par défaut n'en pose aucun, et seuls les vingt skins de `mc` qui en posent
        // un le feraient voir. D'où une palette où **tout** est gras : la cellule qui ne
        // l'est pas désigne le site fautif.
        let theme = AirTheme {
            palette: bold_everywhere(),
            glyphs: AirGlyphSet::turbo_vision(),
        };
        let commands = AirCommandSet::new();
        for (name, view) in every_widget() {
            let bounds = view.state().bounds();
            let mut surface = AirSurface::new(bounds.size);
            {
                let mut painter = AirPainter::new(&mut surface);
                painter.fill_rectangle(AirRectangle::new(AirPoint::ORIGIN, bounds.size), witness());
                draw_view(
                    view.as_ref(),
                    &mut painter,
                    &AirDrawContext::new(&theme, &commands),
                );
            }
            let mut painted = 0usize;
            for row in 0..bounds.size.rows {
                for column in 0..bounds.size.columns {
                    let point = AirPoint::new(AirColumn::new(column), AirRow::new(row));
                    let Some(cell) = surface.cell(point) else {
                        continue;
                    };
                    if *cell == witness() {
                        continue;
                    }
                    painted = painted.saturating_add(1);
                    assert!(
                        cell.attributes.bold,
                        "{name} peint {cell:?} en ({column}, {row}) sans l'attribut de son \
                         rôle : la cellule a été construite à partir des deux couleurs du \
                         couple au lieu du couple entier"
                    );
                }
            }
            assert!(painted > 0, "{name} n'a peint aucune cellule");
        }
    }

    #[test]
    fn les_deux_themes_se_distinguent_a_la_barre_de_menu() {
        // Turbo Vision : noir sur gris clair. Midnight Commander : blanc sur cyan. C'est
        // la différence la plus visible entre les deux, et elle vient du skin.
        let turbo = AirTheme::turbo_vision();
        let midnight = AirTheme::midnight_commander();
        assert_ne!(
            turbo.palette.color_pair(AirRole::MenuBar),
            midnight.palette.color_pair(AirRole::MenuBar)
        );
    }

    #[test]
    fn les_deux_themes_s_accordent_sur_le_fond_du_bureau() {
        // Et ce n'est pas un défaut : les deux sont gris clair sur bleu. Midnight
        // Commander a repris le fond de Norton Commander, que Turbo Vision employait
        // déjà. Deux transcriptions indépendantes qui tombent sur la même valeur.
        let turbo = AirTheme::turbo_vision();
        let midnight = AirTheme::midnight_commander();
        assert_eq!(
            turbo.palette.color_pair(AirRole::DesktopBackground),
            midnight.palette.color_pair(AirRole::DesktopBackground)
        );
    }

    #[test]
    fn le_bureau_de_midnight_commander_n_est_pas_hachure() {
        // mc n'a pas de bureau : ses panneaux couvrent l'écran. Le remplissage est donc
        // un espace, là où Turbo Vision pose son `░`.
        assert_eq!(AirTheme::turbo_vision().glyphs.desktop_fill, '░');
        assert_eq!(AirTheme::midnight_commander().glyphs.desktop_fill, ' ');
    }

    #[test]
    fn midnight_commander_dessine_ses_dialogues_en_trait_simple() {
        // La famille `d*` du skin par défaut est en trait simple, contrairement au
        // `╔═╗` de Turbo Vision. C'est le fichier qui le dit, pas une interprétation.
        let midnight = AirTheme::midnight_commander();
        assert_eq!(midnight.glyphs.dialog_frame, midnight.glyphs.window_frame);
        let turbo = AirTheme::turbo_vision();
        assert_ne!(turbo.glyphs.dialog_frame, turbo.glyphs.window_frame);
    }

    #[test]
    fn la_case_de_fermeture_porte_la_marque_du_skin() {
        assert_eq!(AirTheme::turbo_vision().glyphs.close_box_mark, '■');
        assert_eq!(AirTheme::midnight_commander().glyphs.close_box_mark, 'X');
    }

    #[test]
    fn les_trente_huit_roles_repondent_dans_les_deux_themes() {
        // Un rôle sans valeur serait un trou dans la palette, et le compilateur ne le
        // verrait pas : `color_pair` est exhaustif par construction, mais une palette
        // compilée depuis un skin pourrait porter des couples laissés à zéro.
        let roles = [
            AirRole::DesktopBackground,
            AirRole::MenuBar,
            AirRole::MenuDisabled,
            AirRole::MenuHotkey,
            AirRole::MenuSelected,
            AirRole::MenuSelectedDisabled,
            AirRole::MenuSelectedHotkey,
            AirRole::StatusLine,
            AirRole::StatusDisabled,
            AirRole::StatusHotkey,
            AirRole::WindowFrameActive,
            AirRole::WindowFrameInactive,
            AirRole::WindowFrameIcon,
            AirRole::WindowTitle,
            AirRole::WindowInterior,
            AirRole::WindowText,
            AirRole::DialogFrameActive,
            AirRole::DialogFrameInactive,
            AirRole::DialogFrameIcon,
            AirRole::DialogInterior,
            AirRole::ButtonNormal,
            AirRole::ButtonDefault,
            AirRole::ButtonFocused,
            AirRole::ButtonDisabled,
            AirRole::ButtonHotkey,
            AirRole::ButtonShadow,
            AirRole::ScrollBarPage,
            AirRole::ScrollBarControls,
            AirRole::ListNormal,
            AirRole::ListSelected,
            AirRole::ListMarked,
            AirRole::ListMarkedSelected,
        ];
        assert_eq!(roles.len(), 32);
        let midnight = AirTheme::midnight_commander();
        let mut distinct = Vec::new();
        for role in roles {
            let pair = midnight.palette.color_pair(role);
            distinct.push((pair.foreground, pair.background));
        }
        distinct.dedup();
        assert!(
            distinct.len() > 1,
            "une palette qui ne rend qu'un seul couple n'est pas une palette"
        );
    }

    #[test]
    fn le_theme_compile_porte_la_description_de_son_skin() {
        assert_eq!(AirTheme::midnight_commander_description(), "Standard skin");
    }

    #[test]
    fn les_deux_themes_marquent_un_item_en_jaune() {
        // Deux traditions indépendantes tombent sur la même teinte : `[core] marked` du
        // skin de mc est `yellow;blue`, et `cpAppColor[59]` de Borland porte le jaune
        // vif en avant-plan. Ce n'est pas une transcription de l'une vers l'autre.
        for theme in [AirTheme::turbo_vision(), AirTheme::midnight_commander()] {
            let marked = theme.palette.color_pair(AirRole::ListMarked);
            assert_eq!(marked.foreground, crate::cell::AirColor::Indexed(11));
        }
    }

    #[test]
    fn l_item_sous_le_curseur_se_distingue_de_l_item_ordinaire() {
        // Un thème qui les confondrait rendrait une liste inutilisable : on ne saurait
        // plus où on est. Les deux thèmes doivent les séparer, et pour les quatre rôles.
        for theme in [AirTheme::turbo_vision(), AirTheme::midnight_commander()] {
            let normal = theme.palette.color_pair(AirRole::ListNormal);
            let selected = theme.palette.color_pair(AirRole::ListSelected);
            let marked = theme.palette.color_pair(AirRole::ListMarked);
            let marked_selected = theme.palette.color_pair(AirRole::ListMarkedSelected);
            assert_ne!(normal, selected);
            assert_ne!(normal, marked);
            assert_ne!(marked, marked_selected);
            assert_ne!(selected, marked_selected);
        }
    }

    #[test]
    fn le_theme_par_defaut_est_turbo_vision() {
        let theme = AirTheme::default();
        let reference = AirTheme::turbo_vision();
        assert_eq!(
            theme.palette.color_pair(AirRole::DesktopBackground),
            reference.palette.color_pair(AirRole::DesktopBackground)
        );
        assert_eq!(theme.glyphs.desktop_fill, reference.glyphs.desktop_fill);
    }
}
