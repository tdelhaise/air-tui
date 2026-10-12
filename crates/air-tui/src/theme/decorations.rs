// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Les décorations — la troisième assiette du triplet de l'ADR-008.
//!
//! # Pourquoi elle arrive si tard
//!
//! L'ADR-008 décrit un thème comme un **triplet** depuis le jalon M5, et `AirTheme` était
//! un **couple** jusqu'au jalon M7j. L'écart était assumé plutôt que comblé par une
//! structure vide, et pour une raison précise : la seule décoration qu'elle aurait portée
//! est l'**ombre portée**, qui tombe hors des limites de la vue qui la projette — ce que
//! le découpage du peintre refusait sans exception (M4, M5b). Une assiette qui aurait
//! décrit une ombre que rien ne dessine aurait annoncé une capacité absente.
//!
//! L'ADR-015 a tranché la question sans toucher à l'invariant : ce qui dépasse d'une vue
//! est peint par le **propriétaire de la surface où cela dépasse**. L'assiette décrit donc
//! maintenant quelque chose que l'on dessine.
//!
//! # Ce qu'elle ne porte pas
//!
//! Ni le **style de trait**, ni le **style des touches rapides**, et c'est délibéré :
//!
//! - le style de trait vit déjà dans le jeu de glyphes, qui porte un cadre de fenêtre et
//!   un cadre de dialogue distincts. C'est par là que le skin par défaut de `mc` se
//!   distingue de Turbo Vision, ses dialogues étant en trait simple ;
//! - le marqueur de touche rapide est `~X~`, en dur dans `widgets/hotkey.rs`. Les deux
//!   thèmes emploient le même, donc une assiette qui le porterait ne prouverait rien.
//!
//! Une assiette ne gagne un champ que le jour où deux thèmes le remplissent
//! différemment.
//!
//! # Le deuxième champ, et pourquoi il arrive au jalon M7k
//!
//! L'ombre des **boutons** est ce champ-là. Elle n'est pas une taille de plus : Turbo
//! Vision en dessine une, Midnight Commander n'en dessine aucune — ses boutons sont
//! `[ OK ]`, plats. C'est donc le premier champ que les deux thèmes du dépôt remplissent
//! autrement, et la règle du paragraphe précédent est respectée plutôt que contournée.

/// Le genre d'ombre qu'une vue projette.
///
/// Une vue déclare le **genre**, et le thème décide de l'apparence — jusqu'à l'éteindre.
/// C'est ce partage qui permet à `AirButton` de projeter une ombre sans savoir qu'un thème
/// de Midnight Commander n'en dessine pas.
///
/// Les deux genres diffèrent par **trois** choses à la fois — la forme, le remplissage et
/// le rôle de couleur —, et c'est la raison de cette énumération plutôt que de trois
/// champs sur [`AirShadow`] : une ombre paramétrée par trois boutons décrirait des
/// combinaisons qu'aucun thème ne remplit, et l'ADR-015 refuse les réglages sans lecteur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AirShadowStyle {
    /// L'ombre d'une fenêtre ou d'un dialogue.
    ///
    /// Le rectangle de la vue **décalé**, moins la vue elle-même : une bande en bas et une
    /// bande à droite, qui **recolorent** ce qui est dessous en gardant le caractère.
    /// C'est ce qui lui donne l'aspect d'une ombre sur le motif du bureau plutôt que d'un
    /// rectangle collé à la fenêtre. Couleur : [`crate::theme::AirRole::WindowShadow`].
    Window,
    /// L'ombre d'un bouton.
    ///
    /// Un trait fin qui **épouse** la vue — une colonne à droite sur toute sa hauteur, une
    /// ligne dessous décalée d'une colonne —, peint en **demi-blocs** : `▄` à droite, `▀`
    /// dessous. Le demi-bloc est la transcription de Turbo Vision, qui emploie les
    /// caractères `0xDC` et `0xDF` de la page de code 437 pour que l'ombre d'un bouton
    /// paraisse deux fois plus fine que celle d'une fenêtre. Couleur :
    /// [`crate::theme::AirRole::ButtonShadow`].
    Button,
}

/// La taille d'une ombre portée, en cellules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AirShadow {
    /// Le nombre de colonnes à droite de la vue.
    pub columns: u16,
    /// Le nombre de lignes sous la vue.
    pub rows: u16,
}

impl AirShadow {
    /// L'ombre d'une fenêtre de Turbo Vision : deux colonnes, une ligne.
    ///
    /// Les **deux colonnes** sont celles que l'ADR-008 nomme dans son contexte. La ligne
    /// unique est le choix du jalon M7j : une ombre plus haute n'aurait pas de source, et
    /// une ombre d'une seule colonne ne ressemblerait pas à celle de Borland.
    pub const TURBO_VISION: Self = Self {
        columns: 2,
        rows: 1,
    };

    /// L'ombre d'un bouton : une colonne, une ligne.
    ///
    /// Une seule colonne, là où une fenêtre en a deux, et ce n'est pas une
    /// approximation : Turbo Vision écrit l'ombre d'un bouton dans **une** colonne et
    /// **une** ligne, en demi-blocs. Un bouton fait une ligne de haut, et une ombre de
    /// deux colonnes sous un libellé de dix en ferait un objet qui flotte.
    pub const BUTTON: Self = Self {
        columns: 1,
        rows: 1,
    };
}

/// Les décorations d'un thème.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AirDecorations {
    /// L'ombre des fenêtres et des dialogues, ou `None`.
    ///
    /// `None` éteint ces ombres pour le thème entier, d'un seul endroit : c'est ce qui
    /// permet à un thème de ne pas en avoir sans que chaque vue ait à le savoir.
    pub window_shadow: Option<AirShadow>,
    /// L'ombre des boutons, ou `None`.
    ///
    /// Le champ qui justifie l'énumération [`AirShadowStyle`] : les deux thèmes du dépôt
    /// le remplissent **différemment**. Turbo Vision dessine un trait fin sous chaque
    /// bouton ; Midnight Commander n'en dessine aucun — ses boutons sont `[ OK ]`, plats,
    /// et son format de skin ne décrit rien de tel. Une assiette ne gagne un champ que le
    /// jour où deux thèmes le remplissent autrement, et c'est ce jour-là.
    pub button_shadow: Option<AirShadow>,
}

impl AirDecorations {
    /// Les décorations de Turbo Vision : les deux ombres.
    #[must_use]
    pub const fn turbo_vision() -> Self {
        Self {
            window_shadow: Some(AirShadow::TURBO_VISION),
            button_shadow: Some(AirShadow::BUTTON),
        }
    }

    /// Les décorations d'un thème compilé depuis un skin de Midnight Commander.
    ///
    /// L'ombre de fenêtre a la taille de celle de Turbo Vision, et ce n'est **pas** une
    /// transcription : le format de skin de `mc` décrit la *couleur* de l'ombre —
    /// `[core] shadow`, que trente et un des trente-neuf skins livrés définissent — mais
    /// pas sa taille. La reprendre de Turbo Vision est un choix, et il est écrit ici
    /// plutôt que supposé ailleurs.
    ///
    /// L'ombre de bouton, elle, est **absente**, et ce n'en est pas une non plus : `mc`
    /// dessine ses boutons en `[ OK ]` et `[< OK >]`, sans rien dessous. Le rôle
    /// [`crate::theme::AirRole::ButtonShadow`] garde pourtant une valeur dans la palette
    /// compilée, parce que l'importeur la tire de `[core] shadow` pour que la table de
    /// rôles soit complète — personne ne la lit dans ce thème, et c'est juste.
    #[must_use]
    pub const fn midnight_commander() -> Self {
        Self {
            window_shadow: Some(AirShadow::TURBO_VISION),
            button_shadow: None,
        }
    }

    /// Les mêmes décorations, sans **aucune** ombre.
    ///
    /// Pour un thème qui n'en veut pas, et pour les essais : une ombre recolore ce qui est
    /// dessous, donc un essai qui vérifie autre chose a intérêt à les éteindre toutes.
    #[must_use]
    pub const fn without_shadow(mut self) -> Self {
        self.window_shadow = None;
        self.button_shadow = None;
        self
    }

    /// L'ombre que ce thème donne à un genre, s'il lui en donne une.
    ///
    /// C'est l'unique endroit où un genre devient une taille : le peintre demande, le
    /// thème répond, et une vue n'a jamais à connaître la table.
    #[must_use]
    pub const fn shadow(self, style: AirShadowStyle) -> Option<AirShadow> {
        match style {
            AirShadowStyle::Window => self.window_shadow,
            AirShadowStyle::Button => self.button_shadow,
        }
    }
}

impl Default for AirDecorations {
    fn default() -> Self {
        Self::turbo_vision()
    }
}

#[cfg(test)]
mod tests {
    use super::{AirDecorations, AirShadow, AirShadowStyle};

    #[test]
    fn l_ombre_d_une_fenetre_de_turbo_vision_fait_deux_colonnes_et_une_ligne() {
        // Les deux colonnes sont celles que l'ADR-008 nomme dans son contexte.
        assert_eq!(AirShadow::TURBO_VISION.columns, 2);
        assert_eq!(AirShadow::TURBO_VISION.rows, 1);
    }

    #[test]
    fn l_ombre_d_un_bouton_fait_une_colonne_et_une_ligne() {
        assert_eq!(AirShadow::BUTTON.columns, 1);
        assert_eq!(AirShadow::BUTTON.rows, 1);
    }

    #[test]
    fn les_deux_themes_portent_la_meme_ombre_de_fenetre() {
        // Le format de skin de `mc` décrit la couleur d'une ombre, pas sa taille : celle
        // de Turbo Vision est reprise, et c'est écrit comme un choix.
        assert_eq!(
            AirDecorations::turbo_vision().window_shadow,
            AirDecorations::midnight_commander().window_shadow
        );
    }

    #[test]
    fn midnight_commander_ne_dessine_pas_d_ombre_sous_ses_boutons() {
        // Le champ qui justifie l'énumération : deux thèmes le remplissent autrement.
        assert_eq!(
            AirDecorations::turbo_vision().shadow(AirShadowStyle::Button),
            Some(AirShadow::BUTTON)
        );
        assert_eq!(
            AirDecorations::midnight_commander().shadow(AirShadowStyle::Button),
            None
        );
    }

    #[test]
    fn les_decorations_par_defaut_sont_celles_de_turbo_vision() {
        assert_eq!(AirDecorations::default(), AirDecorations::turbo_vision());
    }

    #[test]
    fn les_deux_ombres_s_eteignent_d_un_seul_endroit() {
        let dark = AirDecorations::turbo_vision().without_shadow();
        assert_eq!(dark.shadow(AirShadowStyle::Window), None);
        assert_eq!(dark.shadow(AirShadowStyle::Button), None);
    }
}
