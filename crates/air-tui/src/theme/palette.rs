// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! La palette — des rôles sémantiques vers des couleurs.
//!
//! # Pourquoi des rôles et non la chaîne de Borland
//!
//! Turbo Vision résolvait une couleur en traversant la chaîne de propriété : un bouton
//! indexait la palette de son dialogue, qui indexait celle de l'application. Le mécanisme
//! est ingénieux et illisible — `cpButton[1]=0x0A → cpGrayDialog[10]=0x29 → cpAppColor[41]`
//! pour obtenir « noir sur vert ».
//!
//! air-tui indexe par **rôle** (ADR-008). La fidélité s'obtient en instanciant les valeurs
//! de Borland dans ce modèle, pas en reproduisant son mécanisme.
//!
//! # Comment les valeurs ont été obtenues
//!
//! Elles sont **transcrites** depuis `cpAppColor` et `cpGrayDialog` de
//! `magiblot/tvision`, en suivant les chaînes de `cpBackground`, `cpMenuView`,
//! `cpStatusLine`, `cpFrame` et `cpButton`. Chaque entrée porte l'octet d'origine et sa
//! chaîne en commentaire : la transcription est donc vérifiable ligne à ligne contre le
//! source C++.
//!
//! Deux pièges s'y cachaient, et ils sont traités explicitement.
//!
//! ## Les indices VGA ne sont pas les indices ANSI
//!
//! Dans un attribut VGA, 1 est le **bleu** et 4 le **rouge** ; en ANSI c'est l'inverse.
//! Transcrire les octets sans conversion donnerait un bureau rouge et des lettres d'accès
//! rapide bleues. La conversion est faite par [`ansi_from_vga`], et un test vérifie
//! qu'elle est bien une bijection qui échange ces deux paires.
//!
//! ## La barre de statut n'est pas cyan
//!
//! `cpStatusLine` et `cpMenuView` partagent la même chaîne : la barre de statut est
//! **noir sur gris clair**, comme la barre de menu. Les premières valeurs de ce projet la
//! donnaient sur cyan, par souvenir plutôt que par lecture.
//!
//! ## « Selected » ne veut pas dire la même chose de part et d'autre
//!
//! Dans `cpListViewer`, Turbo Vision appelle **Focused** l'item sous le curseur et
//! **Selected** un item marqué. air-tui emploie `Selected` pour ce qui est sous le
//! curseur — c'est déjà le sens d'[`AirRole::MenuSelected`] — et `Marked` pour ce qui est
//! marqué. La transcription croise donc les deux noms, et ce n'est pas une étourderie :
//! `ListSelected` vient de `cpListViewer[3]`, `ListMarked` de `cpListViewer[4]`.
//!
//! Le décompte des indices de `cpAppColor` se vérifie sans compter sur ses doigts :
//! `examples/tvdemo/tvdemo2.cpp` nomme lui-même ses entrées, et il donne « List normal »
//! pour 57, « List focused » pour 58 et « List selected » pour 59. Les six entrées des
//! boutons, transcrites au jalon M5, portent les mêmes numéros que ce fichier leur
//! attribue — c'est ce qui valide le décompte des quatre nouvelles.

use crate::cell::{AirAttributes, AirCell, AirColor};

/// Un rôle sémantique de l'interface.
///
/// Fermée par le framework à dessein : un rôle est un contrat entre un widget et un thème,
/// et un thème ne peut pas colorer ce qu'il ne connaît pas. Un widget d'application qui
/// veut sa propre couleur emploie [`AirColor`] directement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AirRole {
    /// Le fond du bureau et son motif.
    DesktopBackground,

    /// La barre de menu, texte ordinaire.
    MenuBar,
    /// Un item de menu désactivé.
    MenuDisabled,
    /// La lettre d'accès rapide d'un item de menu.
    MenuHotkey,
    /// Un item de menu sélectionné.
    MenuSelected,
    /// Un item de menu sélectionné et désactivé.
    MenuSelectedDisabled,
    /// La lettre d'accès rapide d'un item sélectionné.
    MenuSelectedHotkey,

    /// La barre de statut, texte ordinaire.
    StatusLine,
    /// Un item de barre de statut désactivé.
    StatusDisabled,
    /// La lettre d'accès rapide d'un item de barre de statut.
    StatusHotkey,

    /// Le cadre d'une fenêtre active.
    WindowFrameActive,
    /// Le cadre d'une fenêtre inactive.
    WindowFrameInactive,
    /// Les décorations du cadre d'une fenêtre — la case de fermeture.
    WindowFrameIcon,
    /// Le titre d'une fenêtre.
    WindowTitle,
    /// L'intérieur d'une fenêtre.
    WindowInterior,
    /// Le texte d'une fenêtre.
    WindowText,

    /// Le cadre d'un dialogue actif.
    DialogFrameActive,
    /// Le cadre d'un dialogue inactif.
    DialogFrameInactive,
    /// Les décorations du cadre d'un dialogue.
    DialogFrameIcon,
    /// L'intérieur d'un dialogue.
    DialogInterior,

    /// Un bouton au repos.
    ButtonNormal,
    /// Le bouton par défaut, celui qu'« Entrée » active.
    ButtonDefault,
    /// Un bouton qui a le focus.
    ButtonFocused,
    /// Un bouton désactivé.
    ButtonDisabled,
    /// La lettre d'accès rapide d'un bouton.
    ButtonHotkey,
    /// L'ombre portée d'un bouton.
    ButtonShadow,

    /// La zone parcourue d'un ascenseur.
    ScrollBarPage,
    /// Les flèches et le curseur d'un ascenseur.
    ScrollBarControls,

    /// Un item de liste ordinaire.
    ListNormal,
    /// L'item sous le curseur, dans une liste qui a le focus.
    ListSelected,
    /// Un item marqué.
    ListMarked,
    /// Un item marqué et sous le curseur.
    ListMarkedSelected,

    /// Le texte sélectionné dans la vue d'édition.
    ///
    /// Turbo Vision ne lui donne pas de nom dans la palette de l'application : c'est
    /// `cpEditor[2]`, soit `cpBlueWindow[7]`, soit `cpAppColor[14]` — que `tvdemo2.cpp`
    /// nomme « Highlighted text » dans son groupe « Puzzle », une fenêtre bleue comme
    /// celle de l'éditeur. Le texte **non** sélectionné n'a pas de rôle propre : c'est
    /// [`AirRole::WindowText`], car `cpEditor[1]` vaut `cpAppColor[13]`, déjà transcrit au
    /// jalon M5.
    EditorSelected,

    /// Le texte d'un champ de saisie.
    InputLineText,
    /// Le texte sélectionné dans un champ de saisie.
    InputLineSelected,

    /// Un item de case à cocher ou de bouton radio au repos.
    ///
    /// « Cluster » est le mot de Turbo Vision — `TCluster`, dont `TCheckBoxes` et
    /// `TRadioButtons` dérivent —, et les trois rôles sont ses trois premières entrées :
    /// `cpCluster "\x10\x11\x12…"` donne `cpGrayDialog[16..18]`, soit `cpAppColor[47..49]`,
    /// que `tvdemo2.cpp` nomme « Cluster normal », « Cluster selected » et « Cluster
    /// shortcut ».
    ClusterNormal,
    /// L'item sous le curseur, dans un groupe qui a le focus.
    ClusterSelected,
    /// La lettre d'accès rapide d'un item.
    ClusterHotkey,

    /// L'ombre portée d'une fenêtre ou d'un dialogue.
    ///
    /// Distincte d'[`AirRole::ButtonShadow`], qui est l'ombre d'un **bouton** —
    /// `cpButton[8]`, transcrite au jalon M5 et que personne ne lit encore.
    ///
    /// La valeur du thème Midnight Commander vient de `[core] shadow`, que trente et un
    /// des trente-neuf skins livrés définissent. Celle de Turbo Vision **n'est pas une
    /// transcription de chaîne de palette** : l'ombre d'une fenêtre n'a pas d'entrée dans
    /// `cpAppColor`. C'est l'attribut VGA `0x08`, gris sombre sur noir, c'est-à-dire
    /// exactement ce que `mc` nomme de son côté — deux apparences qui concordent ne font
    /// pas une source, mais elles font un repère (ADR-015).
    WindowShadow,
}

/// Ce qu'un rôle décrit : deux couleurs et des attributs.
///
/// Le nom dit « couple » parce que les deux couleurs sont ce qu'on lit d'abord, et il est
/// resté quand les attributs sont arrivés au jalon M7g. Les renommer aurait touché
/// soixante-dix sites pour ne rien apprendre à personne : un rôle décrit une
/// **présentation**, et c'est ce que ce type porte.
///
/// Les attributs sont venus des skins de `mc` : vingt des trente-neuf fournitures de la
/// version 4.8.33 posent `bold`, `underline` ou `italic` sur une clef que la table de
/// correspondance emploie, et l'ADR-008 les refusait toutes pour cette seule raison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AirColorPair {
    /// La couleur du caractère.
    pub foreground: AirColor,
    /// La couleur du fond.
    pub background: AirColor,
    /// Les attributs de rendu — graisse, italique, souligné.
    pub attributes: AirAttributes,
}

impl AirColorPair {
    /// Construit un couple de couleurs sans attribut.
    ///
    /// Sans attribut, et non « avec ceux qu'on précisera » : c'est le cas de l'immense
    /// majorité des rôles — le thème Turbo Vision n'en pose aucun, les seize couleurs du
    /// VGA portant déjà la distinction vive/sombre dans l'index lui-même.
    #[must_use]
    pub const fn new(foreground: AirColor, background: AirColor) -> Self {
        Self {
            foreground,
            background,
            attributes: AirAttributes::NONE,
        }
    }

    /// Le même couple, avec des attributs.
    #[must_use]
    pub const fn with_attributes(mut self, attributes: AirAttributes) -> Self {
        self.attributes = attributes;
        self
    }

    /// Une cellule portant un caractère, peinte à cette présentation.
    ///
    /// C'est par ici que les attributs d'un rôle atteignent la grille. Un widget qui
    /// construirait sa cellule avec `AirCell::new` et les deux champs de couleur
    /// perdrait l'attribut **en silence**, et le thème ne ressemblerait pas au skin : le
    /// défaut ne serait visible que sur les vingt skins qui en posent un, c'est-à-dire
    /// jamais pendant le développement.
    #[must_use]
    pub const fn cell(self, character: char) -> AirCell {
        AirCell::new(character, self.foreground, self.background).with_attributes(self.attributes)
    }
}

/// Convertit un indice de couleur VGA en indice ANSI.
///
/// Les deux numérotations ne diffèrent que par l'ordre des teintes primaires : le bleu et
/// le rouge sont échangés, et le cyan et le jaune avec eux. Transcrire un attribut VGA sans
/// cette conversion donne un bureau rouge et des lettres d'accès rapide bleues.
///
/// Un indice hors des seize est rendu tel quel : il n'y a rien à convertir dans la palette
/// étendue, et ce cas ne se produit pas dans une transcription d'attributs.
#[must_use]
pub const fn ansi_from_vga(vga: u8) -> u8 {
    match vga {
        0 => 0,   // noir
        1 => 4,   // bleu
        2 => 2,   // vert
        3 => 6,   // cyan
        4 => 1,   // rouge
        5 => 5,   // magenta
        6 => 3,   // brun / jaune sombre
        7 => 7,   // gris clair
        8 => 8,   // gris sombre
        9 => 12,  // bleu vif
        10 => 10, // vert vif
        11 => 14, // cyan vif
        12 => 9,  // rouge vif
        13 => 13, // magenta vif
        14 => 11, // jaune
        15 => 15, // blanc
        other => other,
    }
}

/// Le couple de couleurs décrit par un attribut VGA.
///
/// Quartet bas : l'avant-plan. Quartet haut : le fond. C'est la convention du mode texte
/// VGA, celle qu'emploient les palettes de Turbo Vision.
#[must_use]
pub const fn pair_from_vga_attribute(attribute: u8) -> AirColorPair {
    AirColorPair::new(
        AirColor::Indexed(ansi_from_vga(attribute & 0x0F)),
        AirColor::Indexed(ansi_from_vga(attribute >> 4)),
    )
}

/// Les assiettes d'une palette, nommées.
///
/// Elle existe pour que l'artefact généré nomme ce qu'il pose : douze assiettes passées à
/// la file se relisent mal, et un groupe échangé avec son voisin ne se verrait pas. Les
/// longueurs fixes gardent le décompte au compilateur — un groupe de cinq couples là où
/// il en faut six ne compile pas.
#[derive(Debug, Clone, Copy)]
pub(crate) struct AirPaletteParts {
    /// Le fond du bureau.
    pub desktop_background: AirColorPair,
    /// La barre de menu.
    pub menu: [AirColorPair; 6],
    /// La barre de statut.
    pub status: [AirColorPair; 3],
    /// Les fenêtres.
    pub window: [AirColorPair; 6],
    /// Les dialogues.
    pub dialog: [AirColorPair; 4],
    /// Les boutons.
    pub button: [AirColorPair; 6],
    /// Les ascenseurs.
    pub scroll_bar: [AirColorPair; 2],
    /// Les listes.
    pub list: [AirColorPair; 4],
    /// La vue d'édition.
    pub editor: [AirColorPair; 1],
    /// Les champs de saisie.
    pub input_line: [AirColorPair; 2],
    /// Les cases à cocher et les boutons radio.
    pub cluster: [AirColorPair; 3],
    /// L'ombre portée d'une fenêtre.
    pub shadow: AirColorPair,
}

/// La palette d'un thème.
#[derive(Debug, Clone)]
pub struct AirPalette {
    /// Le fond du bureau.
    desktop_background: AirColorPair,
    /// La barre de menu, du texte ordinaire à la lettre d'accès rapide sélectionnée.
    menu: [AirColorPair; 6],
    /// La barre de statut.
    status: [AirColorPair; 3],
    /// Les fenêtres : cadre actif, inactif, décorations, titre, intérieur, texte.
    window: [AirColorPair; 6],
    /// Les dialogues : cadre actif, inactif, décorations, intérieur.
    dialog: [AirColorPair; 4],
    /// Les boutons : repos, défaut, focus, désactivé, accès rapide, ombre.
    button: [AirColorPair; 6],
    /// Les ascenseurs : zone parcourue, commandes.
    scroll_bar: [AirColorPair; 2],
    /// Les listes : item ordinaire, item sous le curseur, item marqué, les deux.
    list: [AirColorPair; 4],
    /// La vue d'édition : le texte sélectionné. Le texte ordinaire est `WindowText`.
    editor: [AirColorPair; 1],
    /// Les champs de saisie : texte, texte sélectionné.
    input_line: [AirColorPair; 2],
    /// Les cases et les boutons radio : repos, sous le curseur, accès rapide.
    cluster: [AirColorPair; 3],
    /// L'ombre portée d'une fenêtre.
    shadow: AirColorPair,
}

impl AirPalette {
    /// Assemble une palette à partir de ses douze assiettes.
    ///
    /// Ce constructeur existe pour les thèmes **compilés** : l'artefact que produit
    /// `cargo xtask import-mc-skin` vit dans `theme/skins/` et n'a pas accès aux champs
    /// privés. Il est `pub(crate)` pour que l'assemblage reste une affaire interne — un
    /// thème s'obtient par son nom, jamais en posant trente-neuf couples à la main.
    pub(crate) const fn from_parts(parts: &AirPaletteParts) -> Self {
        Self {
            desktop_background: parts.desktop_background,
            menu: parts.menu,
            status: parts.status,
            window: parts.window,
            dialog: parts.dialog,
            button: parts.button,
            scroll_bar: parts.scroll_bar,
            list: parts.list,
            editor: parts.editor,
            input_line: parts.input_line,
            cluster: parts.cluster,
            shadow: parts.shadow,
        }
    }

    /// La palette de Midnight Commander, compilée depuis son skin par défaut.
    ///
    /// Les valeurs viennent de `crate::theme::skins::midnight_commander`, produit par
    /// `cargo xtask import-mc-skin` — voir `NOTICE` pour l'origine et sa licence.
    #[must_use]
    pub fn midnight_commander() -> Self {
        crate::theme::AirTheme::compiled(crate::theme::DEFAULT_SKIN)
            .map_or_else(Self::turbo_vision, |theme| theme.palette)
    }

    /// La palette de Turbo Vision, transcrite depuis `magiblot/tvision`.
    ///
    /// Chaque valeur porte l'attribut VGA d'origine et la chaîne qui y mène, pour que la
    /// transcription se vérifie ligne à ligne contre le source C++.
    #[must_use]
    pub const fn turbo_vision() -> Self {
        Self {
            // cpBackground → cpAppColor : gris clair sur bleu.
            desktop_background: pair_from_vga_attribute(0x17),
            menu: [
                pair_from_vga_attribute(0x70), // cpMenuView[1] → cpAppColor[2]
                pair_from_vga_attribute(0x78), // cpMenuView[2] → cpAppColor[3]
                pair_from_vga_attribute(0x74), // cpMenuView[3] → cpAppColor[4]
                pair_from_vga_attribute(0x20), // cpMenuView[4] → cpAppColor[5]
                pair_from_vga_attribute(0x28), // cpMenuView[5] → cpAppColor[6]
                pair_from_vga_attribute(0x24), // cpMenuView[6] → cpAppColor[7]
            ],
            status: [
                pair_from_vga_attribute(0x70), // cpStatusLine[1] → cpAppColor[2]
                pair_from_vga_attribute(0x78), // cpStatusLine[2] → cpAppColor[3]
                pair_from_vga_attribute(0x74), // cpStatusLine[3] → cpAppColor[4]
            ],
            window: [
                pair_from_vga_attribute(0x1F), // cpFrame[3] → cpBlueWindow[2] → cpAppColor[9]
                pair_from_vga_attribute(0x17), // cpFrame[1] → cpBlueWindow[1] → cpAppColor[8]
                pair_from_vga_attribute(0x1A), // cpFrame[5] → cpBlueWindow[3] → cpAppColor[10]
                pair_from_vga_attribute(0x1F), // le titre suit le cadre actif
                pair_from_vga_attribute(0x17), // cpAppColor[8]
                pair_from_vga_attribute(0x1E), // cpAppColor[13]
            ],
            dialog: [
                pair_from_vga_attribute(0x7F), // cpFrame[3] → cpGrayDialog[2] → cpAppColor[33]
                pair_from_vga_attribute(0x70), // cpFrame[1] → cpGrayDialog[1] → cpAppColor[32]
                pair_from_vga_attribute(0x7A), // cpFrame[5] → cpGrayDialog[3] → cpAppColor[34]
                pair_from_vga_attribute(0x70), // cpGrayDialog[6] → cpAppColor[37]
            ],
            button: [
                pair_from_vga_attribute(0x20), // cpButton[1] → cpGrayDialog[10] → cpAppColor[41]
                pair_from_vga_attribute(0x2B), // cpButton[2] → cpGrayDialog[11] → cpAppColor[42]
                pair_from_vga_attribute(0x2F), // cpButton[3] → cpGrayDialog[12] → cpAppColor[43]
                pair_from_vga_attribute(0x78), // cpButton[4] → cpGrayDialog[13] → cpAppColor[44]
                pair_from_vga_attribute(0x2E), // cpButton[5] → cpGrayDialog[14] → cpAppColor[45]
                pair_from_vga_attribute(0x70), // cpButton[8] → cpGrayDialog[15] → cpAppColor[46]
            ],
            scroll_bar: [
                pair_from_vga_attribute(0x31), // cpScrollBar[1] → cpBlueWindow[4] → cpAppColor[11]
                pair_from_vga_attribute(0x31), // cpScrollBar[2] → cpBlueWindow[5] → cpAppColor[12]
            ],
            list: [
                pair_from_vga_attribute(0x30), // cpListViewer[1] → cpGrayDialog[26] → cpAppColor[57]
                pair_from_vga_attribute(0x2F), // cpListViewer[3] → cpGrayDialog[27] → cpAppColor[58]
                pair_from_vga_attribute(0x3E), // cpListViewer[4] → cpGrayDialog[28] → cpAppColor[59]
                // Turbo Vision ne nomme pas ce rôle : l'avant-plan du marqué sur le fond
                // du curseur, soit jaune sur vert.
                AirColorPair::new(
                    pair_from_vga_attribute(0x3E).foreground,
                    pair_from_vga_attribute(0x2F).background,
                ),
            ],
            editor: [
                // cpEditor[2] → cpBlueWindow[7] → cpAppColor[14], que `tvdemo2.cpp` nomme
                // « Highlighted text ». Le texte ordinaire est `WindowText`, soit
                // cpAppColor[13], déjà transcrit.
                pair_from_vga_attribute(0x71),
            ],
            input_line: [
                pair_from_vga_attribute(0x1F), // cpInputLine[2] → cpAppColor[50]
                pair_from_vga_attribute(0x2F), // cpInputLine[3] → cpAppColor[51]
            ],
            cluster: [
                pair_from_vga_attribute(0x30), // cpCluster[1] → cpAppColor[47]
                pair_from_vga_attribute(0x3F), // cpCluster[2] → cpAppColor[48]
                pair_from_vga_attribute(0x3E), // cpCluster[3] → cpAppColor[49]
            ],
            // Pas une transcription de chaîne : l'ombre d'une fenêtre n'a pas d'entrée
            // dans `cpAppColor`. Gris sombre sur noir, ce que `mc` nomme aussi par
            // `[core] shadow` — le repère est écrit dans l'ADR-015.
            shadow: pair_from_vga_attribute(0x08),
        }
    }

    /// Lit un couple dans un tableau de rôles.
    ///
    /// Un indice hors du tableau rend le fond du bureau : c'est visible mais inoffensif, et
    /// le cas est inatteignable depuis [`Self::color_pair`], dont les indices sont des
    /// littéraux.
    /// `slice::get` n'étant pas encore `const`, cette fonction et [`Self::color_pair`]
    /// sont ordinaires. Ce n'est pas une perte : elles sont appelées au dessin, jamais
    /// dans un contexte constant.
    fn at(&self, group: &[AirColorPair], index: usize) -> AirColorPair {
        match group.get(index) {
            Some(pair) => *pair,
            None => self.desktop_background,
        }
    }

    /// Les couleurs d'un rôle.
    #[must_use]
    pub fn color_pair(&self, role: AirRole) -> AirColorPair {
        match role {
            AirRole::DesktopBackground => self.desktop_background,

            AirRole::MenuBar => self.at(&self.menu, 0),
            AirRole::MenuDisabled => self.at(&self.menu, 1),
            AirRole::MenuHotkey => self.at(&self.menu, 2),
            AirRole::MenuSelected => self.at(&self.menu, 3),
            AirRole::MenuSelectedDisabled => self.at(&self.menu, 4),
            AirRole::MenuSelectedHotkey => self.at(&self.menu, 5),

            AirRole::StatusLine => self.at(&self.status, 0),
            AirRole::StatusDisabled => self.at(&self.status, 1),
            AirRole::StatusHotkey => self.at(&self.status, 2),

            AirRole::WindowFrameActive => self.at(&self.window, 0),
            AirRole::WindowFrameInactive => self.at(&self.window, 1),
            AirRole::WindowFrameIcon => self.at(&self.window, 2),
            AirRole::WindowTitle => self.at(&self.window, 3),
            AirRole::WindowInterior => self.at(&self.window, 4),
            AirRole::WindowText => self.at(&self.window, 5),

            AirRole::DialogFrameActive => self.at(&self.dialog, 0),
            AirRole::DialogFrameInactive => self.at(&self.dialog, 1),
            AirRole::DialogFrameIcon => self.at(&self.dialog, 2),
            AirRole::DialogInterior => self.at(&self.dialog, 3),

            AirRole::ButtonNormal => self.at(&self.button, 0),
            AirRole::ButtonDefault => self.at(&self.button, 1),
            AirRole::ButtonFocused => self.at(&self.button, 2),
            AirRole::ButtonDisabled => self.at(&self.button, 3),
            AirRole::ButtonHotkey => self.at(&self.button, 4),
            AirRole::ButtonShadow => self.at(&self.button, 5),

            AirRole::ScrollBarPage => self.at(&self.scroll_bar, 0),
            AirRole::ScrollBarControls => self.at(&self.scroll_bar, 1),

            AirRole::ListNormal => self.at(&self.list, 0),
            AirRole::ListSelected => self.at(&self.list, 1),
            AirRole::ListMarked => self.at(&self.list, 2),
            AirRole::ListMarkedSelected => self.at(&self.list, 3),
            AirRole::EditorSelected => self.at(&self.editor, 0),
            AirRole::InputLineText => self.at(&self.input_line, 0),
            AirRole::InputLineSelected => self.at(&self.input_line, 1),
            AirRole::ClusterNormal => self.at(&self.cluster, 0),
            AirRole::ClusterSelected => self.at(&self.cluster, 1),
            AirRole::ClusterHotkey => self.at(&self.cluster, 2),
            AirRole::WindowShadow => self.shadow,
        }
    }
}

impl Default for AirPalette {
    fn default() -> Self {
        Self::turbo_vision()
    }
}

#[cfg(test)]
mod tests {
    use super::{AirPalette, AirRole, ansi_from_vga, pair_from_vga_attribute};
    use crate::cell::AirColor;

    /// Tous les rôles, pour les parcours exhaustifs.
    const ALL_ROLES: [AirRole; 39] = [
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
        AirRole::EditorSelected,
        AirRole::InputLineText,
        AirRole::InputLineSelected,
        AirRole::ClusterNormal,
        AirRole::ClusterSelected,
        AirRole::ClusterHotkey,
        AirRole::WindowShadow,
    ];

    #[test]
    fn la_conversion_vga_vers_ansi_est_une_bijection_sur_les_seize() {
        let mut seen = [false; 16];
        for vga in 0..16_u8 {
            let ansi = ansi_from_vga(vga);
            assert!(ansi < 16, "indice {ansi} hors des seize pour le VGA {vga}");
            let position = usize::from(ansi);
            assert!(
                !seen.get(position).copied().unwrap_or(true),
                "l'indice ANSI {ansi} est produit deux fois"
            );
            if let Some(slot) = seen.get_mut(position) {
                *slot = true;
            }
        }
        assert!(seen.iter().all(|found| *found), "un indice ANSI est absent");
    }

    #[test]
    fn la_conversion_echange_le_bleu_et_le_rouge() {
        // C'est le piège de la transcription : sans cet échange, le bureau serait rouge et
        // les lettres d'accès rapide bleues.
        assert_eq!(ansi_from_vga(1), 4, "le bleu VGA est le 4 ANSI");
        assert_eq!(ansi_from_vga(4), 1, "le rouge VGA est le 1 ANSI");
        assert_eq!(ansi_from_vga(3), 6, "le cyan VGA est le 6 ANSI");
        assert_eq!(ansi_from_vga(6), 3, "le brun VGA est le 3 ANSI");
        assert_eq!(ansi_from_vga(11), 14, "le cyan vif");
        assert_eq!(ansi_from_vga(14), 11, "le jaune");
    }

    #[test]
    fn les_couleurs_neutres_ne_bougent_pas() {
        for vga in [0_u8, 2, 5, 7, 8, 10, 13, 15] {
            assert_eq!(ansi_from_vga(vga), vga, "pour le VGA {vga}");
        }
    }

    #[test]
    fn un_indice_hors_des_seize_traverse_sans_conversion() {
        // Inatteignable depuis une transcription d'attribut ; couvert directement.
        assert_eq!(ansi_from_vga(200), 200);
    }

    #[test]
    fn un_attribut_vga_se_lit_quartet_bas_en_avant_plan() {
        // 0x1F : fond bleu (quartet haut), avant-plan blanc (quartet bas).
        let pair = pair_from_vga_attribute(0x1F);
        assert_eq!(pair.foreground, AirColor::Indexed(15));
        assert_eq!(pair.background, AirColor::Indexed(4));
    }

    #[test]
    fn le_bureau_est_gris_clair_sur_bleu() {
        // La valeur que le port vérifié par instantanés rend : fg gris clair, bg bleu.
        let pair = AirPalette::turbo_vision().color_pair(AirRole::DesktopBackground);
        assert_eq!(pair.foreground, AirColor::Indexed(7));
        assert_eq!(pair.background, AirColor::Indexed(4));
    }

    #[test]
    fn la_barre_de_statut_est_noir_sur_gris_clair_comme_le_menu() {
        // Correction de fidélité : cpStatusLine et cpMenuView partagent la même chaîne.
        // Les premières valeurs de ce projet donnaient la barre de statut sur cyan.
        let palette = AirPalette::turbo_vision();
        assert_eq!(
            palette.color_pair(AirRole::StatusLine),
            palette.color_pair(AirRole::MenuBar)
        );
        let pair = palette.color_pair(AirRole::StatusLine);
        assert_eq!(pair.foreground, AirColor::Indexed(0));
        assert_eq!(pair.background, AirColor::Indexed(7));
    }

    #[test]
    fn les_lettres_d_acces_rapide_sont_rouges() {
        let palette = AirPalette::turbo_vision();
        for role in [AirRole::MenuHotkey, AirRole::StatusHotkey] {
            assert_eq!(
                palette.color_pair(role).foreground,
                AirColor::Indexed(1),
                "pour {role:?}"
            );
        }
    }

    #[test]
    fn les_decorations_de_cadre_sont_vert_vif() {
        // La case de fermeture `[■]` : vert vif sur bleu dans une fenêtre, vert vif sur
        // gris clair dans un dialogue.
        let palette = AirPalette::turbo_vision();
        assert_eq!(
            palette.color_pair(AirRole::WindowFrameIcon).foreground,
            AirColor::Indexed(10)
        );
        assert_eq!(
            palette.color_pair(AirRole::DialogFrameIcon).foreground,
            AirColor::Indexed(10)
        );
        assert_eq!(
            palette.color_pair(AirRole::DialogFrameIcon).background,
            AirColor::Indexed(7)
        );
    }

    #[test]
    fn les_boutons_sont_noir_sur_vert_avec_un_accent_jaune() {
        let palette = AirPalette::turbo_vision();
        let normal = palette.color_pair(AirRole::ButtonNormal);
        assert_eq!(normal.foreground, AirColor::Indexed(0));
        assert_eq!(normal.background, AirColor::Indexed(2));
        let hotkey = palette.color_pair(AirRole::ButtonHotkey);
        assert_eq!(hotkey.foreground, AirColor::Indexed(11), "jaune");
        assert_eq!(hotkey.background, AirColor::Indexed(2));
        let focused = palette.color_pair(AirRole::ButtonFocused);
        assert_eq!(focused.foreground, AirColor::Indexed(15), "blanc");
    }

    #[test]
    fn un_cadre_actif_se_distingue_d_un_cadre_inactif() {
        let palette = AirPalette::turbo_vision();
        assert_ne!(
            palette.color_pair(AirRole::WindowFrameActive),
            palette.color_pair(AirRole::WindowFrameInactive)
        );
        assert_ne!(
            palette.color_pair(AirRole::DialogFrameActive),
            palette.color_pair(AirRole::DialogFrameInactive)
        );
    }

    #[test]
    fn tous_les_roles_rendent_une_couleur_indexee_dans_les_seize() {
        // Un thème transcrit de Borland n'emploie que les seize couleurs : une valeur hors
        // de ce domaine signalerait une erreur de transcription.
        let palette = AirPalette::turbo_vision();
        for role in ALL_ROLES {
            let pair = palette.color_pair(role);
            for color in [pair.foreground, pair.background] {
                assert!(
                    matches!(color, AirColor::Indexed(index) if index < 16),
                    "{role:?} emploie {color:?}, qui n'est pas un des seize index"
                );
            }
        }
    }

    #[test]
    fn la_palette_par_defaut_est_celle_de_turbo_vision() {
        let default = AirPalette::default();
        let reference = AirPalette::turbo_vision();
        for role in ALL_ROLES {
            assert_eq!(
                default.color_pair(role),
                reference.color_pair(role),
                "pour {role:?}"
            );
        }
    }

    #[test]
    fn un_indice_hors_du_tableau_retombe_sur_le_fond_du_bureau() {
        // Inatteignable depuis `color_pair`, dont les indices sont des littéraux ; couvert
        // directement pour que la branche existe dans la mesure.
        let palette = AirPalette::turbo_vision();
        assert_eq!(
            palette.at(&[], 0),
            palette.color_pair(AirRole::DesktopBackground)
        );
    }
}
