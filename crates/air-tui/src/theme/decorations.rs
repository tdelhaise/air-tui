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

/// La taille d'une ombre portée, en cellules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AirShadow {
    /// Le nombre de colonnes à droite de la vue.
    pub columns: u16,
    /// Le nombre de lignes sous la vue.
    pub rows: u16,
}

impl AirShadow {
    /// L'ombre de Turbo Vision : deux colonnes, une ligne.
    ///
    /// Les **deux colonnes** sont celles que l'ADR-008 nomme dans son contexte. La ligne
    /// unique est le choix de ce jalon : une ombre plus haute n'aurait pas de source, et
    /// une ombre d'une seule colonne ne ressemblerait pas à celle de Borland.
    pub const TURBO_VISION: Self = Self {
        columns: 2,
        rows: 1,
    };
}

/// Les décorations d'un thème.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AirDecorations {
    /// L'ombre que projettent les vues qui en projettent une, ou `None`.
    ///
    /// `None` éteint les ombres du thème entier, d'un seul endroit : c'est ce qui permet
    /// à un thème de ne pas en avoir sans que chaque vue ait à le savoir.
    pub shadow: Option<AirShadow>,
}

impl AirDecorations {
    /// Les décorations de Turbo Vision.
    #[must_use]
    pub const fn turbo_vision() -> Self {
        Self {
            shadow: Some(AirShadow::TURBO_VISION),
        }
    }

    /// Les décorations d'un thème compilé depuis un skin de Midnight Commander.
    ///
    /// La taille est celle de Turbo Vision, et ce n'est **pas** une transcription : le
    /// format de skin de `mc` décrit la *couleur* de l'ombre — `[core] shadow`, que
    /// trente et un des trente-neuf skins livrés définissent — mais pas sa taille. La
    /// reprendre de Turbo Vision est un choix, et il est écrit ici plutôt que supposé
    /// ailleurs.
    #[must_use]
    pub const fn midnight_commander() -> Self {
        Self {
            shadow: Some(AirShadow::TURBO_VISION),
        }
    }

    /// Les mêmes décorations, sans ombre.
    ///
    /// Pour un thème qui n'en veut pas, et pour les essais : une ombre recolore ce qui est
    /// dessous, donc un essai qui vérifie autre chose a intérêt à l'éteindre.
    #[must_use]
    pub const fn without_shadow(mut self) -> Self {
        self.shadow = None;
        self
    }
}

impl Default for AirDecorations {
    fn default() -> Self {
        Self::turbo_vision()
    }
}

#[cfg(test)]
mod tests {
    use super::{AirDecorations, AirShadow};

    #[test]
    fn l_ombre_de_turbo_vision_fait_deux_colonnes_et_une_ligne() {
        // Les deux colonnes sont celles que l'ADR-008 nomme dans son contexte.
        assert_eq!(AirShadow::TURBO_VISION.columns, 2);
        assert_eq!(AirShadow::TURBO_VISION.rows, 1);
    }

    #[test]
    fn les_deux_themes_portent_la_meme_ombre() {
        // Le format de skin de `mc` décrit la couleur d'une ombre, pas sa taille : celle
        // de Turbo Vision est reprise, et c'est écrit comme un choix.
        assert_eq!(
            AirDecorations::turbo_vision(),
            AirDecorations::midnight_commander()
        );
    }

    #[test]
    fn les_decorations_par_defaut_sont_celles_de_turbo_vision() {
        assert_eq!(AirDecorations::default(), AirDecorations::turbo_vision());
    }

    #[test]
    fn une_ombre_s_eteint_d_un_seul_endroit() {
        assert_eq!(AirDecorations::turbo_vision().without_shadow().shadow, None);
    }
}
