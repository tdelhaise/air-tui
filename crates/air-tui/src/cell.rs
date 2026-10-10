// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! La cellule — l'unité de la grille de texte.

/// Une couleur de cellule.
///
/// Les deux formes coexistent délibérément : un thème s'écrit en RGB, mais une console
/// Linux n'offre que seize couleurs. Le quantificateur qui ramènera l'un vers l'autre est
/// prévu au jalon M5 (ADR-008) et **n'est pas écrit**.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AirColor {
    /// Une des couleurs de la palette du terminal.
    ///
    /// Les seize premières valeurs sont les couleurs ANSI historiques : 0 noir, 1 rouge,
    /// 2 vert, 3 jaune, 4 bleu, 5 magenta, 6 cyan, 7 gris clair, puis 8 à 15 leurs
    /// variantes vives.
    Indexed(u8),
    /// Une couleur en vraies couleurs, pour les terminaux qui l'annoncent.
    Rgb {
        /// Composante rouge.
        red: u8,
        /// Composante verte.
        green: u8,
        /// Composante bleue.
        blue: u8,
    },
}

impl AirColor {
    /// Noir — index ANSI 0.
    pub const BLACK: Self = Self::Indexed(0);
    /// Bleu — index ANSI 4. Le fond du bureau de Turbo Vision.
    pub const BLUE: Self = Self::Indexed(4);
    /// Cyan — index ANSI 6. Le fond de la barre de statut de Turbo Vision.
    pub const CYAN: Self = Self::Indexed(6);
    /// Rouge — index ANSI 1. La couleur des lettres d'accès rapide dans les menus.
    pub const RED: Self = Self::Indexed(1);
    /// Gris clair — index ANSI 7. Le fond des dialogues et de la barre de menu.
    pub const LIGHT_GRAY: Self = Self::Indexed(7);
    /// Vert vif — index ANSI 10. La marque de la case de fermeture.
    pub const BRIGHT_GREEN: Self = Self::Indexed(10);
    /// Blanc vif — index ANSI 15. Le cadre d'une fenêtre active.
    pub const WHITE: Self = Self::Indexed(15);
}

/// Les attributs de rendu d'une cellule.
///
/// Une structure de booléens nommés plutôt qu'un masque de bits : le Principe 7 d'Air
/// préfère la verbosité qui clarifie à la compacité qui cache.
// Six booléens, et c'est la bonne forme : chacun correspond à un paramètre SGR
// indépendant du terminal. Les regrouper en masque de bits ne gagnerait que de la
// compacité, au prix de la lisibilité que le Principe 7 d'Air privilégie.
#[allow(
    clippy::struct_excessive_bools,
    reason = "un booléen par paramètre SGR"
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AirAttributes {
    /// Graisse accentuée.
    pub bold: bool,
    /// Intensité réduite.
    pub dim: bool,
    /// Italique.
    pub italic: bool,
    /// Souligné.
    pub underline: bool,
    /// Couleurs inversées.
    pub reverse: bool,
    /// Barré.
    pub strikethrough: bool,
}

impl AirAttributes {
    /// Aucun attribut.
    pub const NONE: Self = Self {
        bold: false,
        dim: false,
        italic: false,
        underline: false,
        reverse: false,
        strikethrough: false,
    };

    /// Les mêmes attributs, en graisse accentuée.
    #[must_use]
    pub const fn with_bold(mut self) -> Self {
        self.bold = true;
        self
    }
}

/// Une cellule de la grille : un caractère et sa présentation.
///
/// Le caractère est un `char`, donc un point de code Unicode complet. La gestion des
/// graphèmes composés de plusieurs points de code — et de leur largeur d'affichage —
/// relève des tables UCD du jalon M3 (ADR-006) et **n'est pas écrite**.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AirCell {
    /// Le caractère affiché.
    pub character: char,
    /// La couleur du caractère.
    pub foreground: AirColor,
    /// La couleur du fond.
    pub background: AirColor,
    /// Les attributs de rendu.
    pub attributes: AirAttributes,
    /// Vrai si cette cellule est la **seconde moitié** d'un caractère large.
    ///
    /// Un idéogramme ou un emoji occupe deux colonnes de la grille mais ne porte qu'un
    /// seul caractère. La cellule qui suit est marquée ainsi, et le rendu la **saute** :
    /// l'émettre écraserait la moitié droite du glyphe que le terminal vient de dessiner.
    ///
    /// Un booléen nommé plutôt qu'un caractère sentinelle : l'ADR-021 §1 refuse les
    /// sentinelles, et `'\0'` dans un champ `character` serait exactement cela.
    pub is_continuation: bool,
}

impl AirCell {
    /// Construit une cellule.
    pub const fn new(character: char, foreground: AirColor, background: AirColor) -> Self {
        Self {
            character,
            foreground,
            background,
            attributes: AirAttributes::NONE,
            is_continuation: false,
        }
    }

    /// La seconde moitié d'un caractère large, qui reprend sa présentation.
    ///
    /// Elle porte une espace : si le terminal ne sait pas dessiner le glyphe large, la
    /// cellule reste au moins de la bonne couleur au lieu d'afficher un résidu.
    #[must_use]
    pub const fn continuation_of(wide: Self) -> Self {
        Self {
            character: ' ',
            foreground: wide.foreground,
            background: wide.background,
            attributes: wide.attributes,
            is_continuation: true,
        }
    }

    /// La même cellule, avec d'autres attributs.
    #[must_use]
    pub const fn with_attributes(mut self, attributes: AirAttributes) -> Self {
        self.attributes = attributes;
        self
    }
}

impl Default for AirCell {
    fn default() -> Self {
        Self::new(' ', AirColor::LIGHT_GRAY, AirColor::BLACK)
    }
}

#[cfg(test)]
mod tests {
    use super::{AirAttributes, AirCell, AirColor};

    #[test]
    fn la_cellule_par_defaut_est_une_espace() {
        let cell = AirCell::default();
        assert_eq!(cell.character, ' ');
        assert_eq!(cell.attributes, AirAttributes::NONE);
        assert!(!cell.is_continuation);
    }

    #[test]
    fn une_continuation_reprend_la_presentation_du_caractere_large() {
        let wide = AirCell::new('漢', AirColor::WHITE, AirColor::BLUE)
            .with_attributes(AirAttributes::NONE.with_bold());
        let continuation = AirCell::continuation_of(wide);
        assert!(continuation.is_continuation);
        assert_eq!(continuation.character, ' ');
        assert_eq!(continuation.foreground, AirColor::WHITE);
        assert_eq!(continuation.background, AirColor::BLUE);
        assert!(continuation.attributes.bold);
    }

    #[test]
    fn with_bold_n_active_que_la_graisse() {
        let attributes = AirAttributes::NONE.with_bold();
        assert!(attributes.bold);
        assert!(!attributes.italic);
        assert!(!attributes.underline);
    }

    #[test]
    fn with_attributes_remplace_les_attributs() {
        let cell = AirCell::new('X', AirColor::WHITE, AirColor::BLUE)
            .with_attributes(AirAttributes::NONE.with_bold());
        assert!(cell.attributes.bold);
        assert_eq!(cell.character, 'X');
    }
}
