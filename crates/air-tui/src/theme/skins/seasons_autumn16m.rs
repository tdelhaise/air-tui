// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Le thème Midnight Commander — **fichier généré, ne pas éditer**.
//!
//! Produit par `cargo xtask import-mc-skin` depuis « Four Seasons – Autumn », fichier
//! `/usr/local/share/mc/skins/seasons-autumn16M.ini`. 37 rôles transcrits d'une clef, 3 composés de deux.
//!
//! Empreinte FNV-1a du skin d'origine : `0x59f52bde5d5b591a`. C'est un repère de
//! dérive, pas une garantie cryptographique : `cargo xtask import-mc-skin --check`
//! la recalcule et distingue « ce n'est pas le même skin » de « l'artefact ne
//! correspond plus à ce skin ».
//!
//! Les index sont ceux de la palette du **terminal** : `mc` y a toujours tourné,
//! il n'y a donc aucune conversion à faire — contrairement aux attributs VGA de
//! Turbo Vision, où oublier l'échange du bleu et du rouge donnait un bureau rouge.
//!
//! # Les correspondances qui ont demandé un choix
//!
//! `mc` nomme ses couleurs par élément d'interface, air-tui par rôle sémantique :
//! les deux ensembles ne se recouvrent pas entièrement. Là où le choix de la clef
//! se discute, ou là où `mc` ne nomme pas le rôle du tout, la règle est écrite
//! ici — une dérivation nommée se discute, une dérivation silencieuse se
//! découvre des mois plus tard devant un écran qui ne ressemble pas au skin.
//!
//! - **`DesktopBackground`** — mc n'a pas de bureau — ses panneaux couvrent l'écran.
//!   `[core] _default_` est la couleur de fond de l'application.
//! - **`MenuSelectedDisabled`** — mc ne nomme pas ce rôle : l'avant-plan éteint de `[core]
//!   disabled` sur le fond de l'item sélectionné.
//! - **`StatusLine`** — la barre de statut d'air-tui publie des touches de fonction : c'est
//!   la barre F1-F10 de mc, donc `[buttonbar] et` non [statusbar].
//! - **`StatusDisabled`** — mc ne nomme pas ce rôle : l'avant-plan éteint de `[core]
//!   disabled` sur le fond de la barre.
//! - **`WindowFrameActive`** — une fenêtre air-tui est l'équivalent de la fenêtre d'édition
//!   de mc, pas de son dialogue.
//! - **`WindowFrameInactive`** — mc n'a qu'une fenêtre d'édition à la fois : son cadre
//!   inactif est le défaut de la section.
//! - **`WindowFrameIcon`** — mc donne le caractère de la case de fermeture ([widget-editor]
//!   window-close-char) mais pas sa couleur : celle du cadre actif.
//! - **`WindowTitle`** — mc écrit le nom du fichier dans le cadre, à la couleur du cadre.
//! - **`DialogFrameInactive`** — mc n'empile pas ses dialogues : l'avant-plan éteint sur le
//!   fond du dialogue.
//! - **`DialogFrameIcon`** — la couleur que mc réserve au titre d'un dialogue.
//! - **`ButtonNormal`** — un bouton de mc est un élément de dialogue : il en porte le
//!   défaut.
//! - **`ButtonDefault`** — mc ne distingue pas le bouton par défaut du bouton focalisé.
//! - **`ScrollBarPage`** — mc n'a pas d'ascenseur : la zone parcourue prend le fond de
//!   l'application.
//! - **`ScrollBarControls`** — mc n'a pas d'ascenseur : ses flèches prennent la couleur du
//!   curseur.
//! - **`ListNormal`** — un panneau de mc est l'application elle-même : ses lignes portent
//!   `[core] _default_`.
//! - **`ListDivider`** — mc ne nomme pas le séparateur de colonnes d'un panneau bref, ni sa
//!   couleur ni son caractère : le trait prend celle des lignes ordinaires, et son
//!   caractère est le [Lines] vert du skin.
//! - **`ClusterNormal`** — mc ne distingue pas les cases à cocher du fond de leur dialogue.
//! - **`WindowShadow`** — mc nomme l'ombre, et trente et un des trente-neuf skins livrés la
//!   définissent. Les huit autres l'héritent de [core] _default_, ce qui la rend invisible
//!   — c'est ce que mc fait aussi.
//! # Ce que ce skin demande au-delà de deux couleurs
//!
//! 7 rôles portent un attribut du skin, et 0 ont une
//! couleur laissée au terminal. Les deux étaient des motifs de refus avant le
//! jalon M7g ; ils sont désormais transcrits. Pour un rôle composé de deux clefs,
//! les attributs retenus sont ceux de la clef qui donne l'avant-plan.
//!

use crate::cell::{AirAttributes, AirColor};
use crate::theme::glyphs::{AirClusterGlyphs, AirFrameGlyphs, AirGlyphSet};
use crate::theme::palette::{AirColorPair, AirPalette, AirPaletteParts};

/// La description que porte la section `[skin]` du fichier d'origine.
pub(crate) const DESCRIPTION: &str = "Four Seasons – Autumn";

/// Une vraie couleur, telle que le skin l'écrit en `#rrggbb`.
const fn rgb(red: u8, green: u8, blue: u8) -> AirColor {
    AirColor::Rgb { red, green, blue }
}

/// La palette, compilée depuis le skin.
pub(crate) const fn palette() -> AirPalette {
    AirPalette::from_parts(&AirPaletteParts {
        desktop_background: AirColorPair::new(rgb(0xff, 0xff, 0xaa), rgb(0x99, 0x20, 0x05)), // DesktopBackground ← [core] _default_
        menu: [
            AirColorPair::new(rgb(0x00, 0x00, 0x00), rgb(0xff, 0xc9, 0x0c)), // MenuBar ← [menu] _default_
            AirColorPair::new(rgb(0x00, 0x00, 0x00), rgb(0xff, 0x99, 0x09)), // MenuDisabled ← [menu] menuinactive
            AirColorPair::new(rgb(0x00, 0x00, 0x00), rgb(0xff, 0xc9, 0x0c)).with_attributes(
                AirAttributes {
                    underline: true,
                    ..AirAttributes::NONE
                },
            ), // MenuHotkey ← [menu] menuhot
            AirColorPair::new(rgb(0x00, 0x00, 0x00), rgb(0xff, 0x99, 0x09)), // MenuSelected ← [menu] menusel
            AirColorPair::new(rgb(0x55, 0x55, 0x55), rgb(0xff, 0x99, 0x09)), // MenuSelectedDisabled ← [core] disabled + [menu] menusel
            AirColorPair::new(rgb(0x00, 0x00, 0x00), rgb(0xff, 0x99, 0x09)).with_attributes(
                AirAttributes {
                    underline: true,
                    ..AirAttributes::NONE
                },
            ), // MenuSelectedHotkey ← [menu] menuhotsel
        ],
        status: [
            AirColorPair::new(rgb(0x00, 0x00, 0x00), rgb(0xff, 0xc9, 0x0c)), // StatusLine ← [buttonbar] button
            AirColorPair::new(rgb(0x55, 0x55, 0x55), rgb(0xff, 0xc9, 0x0c)), // StatusDisabled ← [core] disabled + [buttonbar] button
            AirColorPair::new(rgb(0xff, 0xff, 0xaa), rgb(0x99, 0x20, 0x05)), // StatusHotkey ← [buttonbar] hotkey
        ],
        window: [
            AirColorPair::new(rgb(0xff, 0xff, 0x55), rgb(0x99, 0x20, 0x05)), // WindowFrameActive ← [editor] editframeactive
            AirColorPair::new(rgb(0xff, 0xff, 0xaa), rgb(0x99, 0x20, 0x05)), // WindowFrameInactive ← [editor] _default_
            AirColorPair::new(rgb(0xff, 0xff, 0x55), rgb(0x99, 0x20, 0x05)), // WindowFrameIcon ← [editor] editframeactive
            AirColorPair::new(rgb(0xff, 0xff, 0x55), rgb(0x99, 0x20, 0x05)), // WindowTitle ← [editor] editframeactive
            AirColorPair::new(rgb(0xff, 0xff, 0xaa), rgb(0x99, 0x20, 0x05)), // WindowInterior ← [editor] _default_
            AirColorPair::new(rgb(0xff, 0xff, 0xaa), rgb(0x99, 0x20, 0x05)), // WindowText ← [editor] _default_
        ],
        dialog: [
            AirColorPair::new(rgb(0x00, 0x00, 0x00), rgb(0x85, 0xa9, 0x18)), // DialogFrameActive ← [dialog] _default_
            AirColorPair::new(rgb(0x55, 0x55, 0x55), rgb(0x85, 0xa9, 0x18)), // DialogFrameInactive ← [core] disabled + [dialog] _default_
            AirColorPair::new(rgb(0x00, 0x00, 0x00), rgb(0x85, 0xa9, 0x18)).with_attributes(
                AirAttributes {
                    bold: true,
                    ..AirAttributes::NONE
                },
            ), // DialogFrameIcon ← [dialog] dtitle
            AirColorPair::new(rgb(0x00, 0x00, 0x00), rgb(0x85, 0xa9, 0x18)), // DialogInterior ← [dialog] _default_
        ],
        button: [
            AirColorPair::new(rgb(0x00, 0x00, 0x00), rgb(0x85, 0xa9, 0x18)), // ButtonNormal ← [dialog] _default_
            AirColorPair::new(rgb(0x00, 0x00, 0x00), rgb(0x69, 0x88, 0x0c)), // ButtonDefault ← [dialog] dfocus
            AirColorPair::new(rgb(0x00, 0x00, 0x00), rgb(0x69, 0x88, 0x0c)), // ButtonFocused ← [dialog] dfocus
            AirColorPair::new(rgb(0x55, 0x55, 0x55), rgb(0x85, 0xa9, 0x18)), // ButtonDisabled ← [core] disabled
            AirColorPair::new(rgb(0x00, 0x00, 0x00), rgb(0x85, 0xa9, 0x18)).with_attributes(
                AirAttributes {
                    underline: true,
                    ..AirAttributes::NONE
                },
            ), // ButtonHotkey ← [dialog] dhotnormal
            AirColorPair::new(rgb(0x7f, 0x7f, 0x55), rgb(0x4c, 0x10, 0x02)), // ButtonShadow ← [core] shadow
        ],
        scroll_bar: [
            AirColorPair::new(rgb(0xff, 0xff, 0xaa), rgb(0x99, 0x20, 0x05)), // ScrollBarPage ← [core] _default_
            AirColorPair::new(rgb(0xff, 0xff, 0xaa), rgb(0x68, 0x14, 0x00)), // ScrollBarControls ← [core] selected
        ],
        list: [
            AirColorPair::new(rgb(0xff, 0xff, 0xaa), rgb(0x99, 0x20, 0x05)), // ListNormal ← [core] _default_
            AirColorPair::new(rgb(0xff, 0xff, 0xaa), rgb(0x68, 0x14, 0x00)), // ListSelected ← [core] selected
            AirColorPair::new(rgb(0xff, 0xff, 0x55), rgb(0x99, 0x20, 0x05)).with_attributes(
                AirAttributes {
                    bold: true,
                    ..AirAttributes::NONE
                },
            ), // ListMarked ← [core] marked
            AirColorPair::new(rgb(0xff, 0xff, 0x55), rgb(0x68, 0x14, 0x00)).with_attributes(
                AirAttributes {
                    bold: true,
                    ..AirAttributes::NONE
                },
            ), // ListMarkedSelected ← [core] markselect
            AirColorPair::new(rgb(0xff, 0xff, 0xaa), rgb(0x99, 0x20, 0x05)), // ListDivider ← [core] _default_
        ],
        editor: [
            AirColorPair::new(rgb(0xff, 0xff, 0xaa), rgb(0x68, 0x14, 0x00)), // EditorSelected ← [editor] editmarked
        ],
        input_line: [
            AirColorPair::new(rgb(0x00, 0x00, 0x00), rgb(0xb5, 0xc4, 0x00)), // InputLineText ← [core] input
            AirColorPair::new(rgb(0x00, 0x00, 0x00), rgb(0x69, 0x88, 0x0c)), // InputLineSelected ← [core] inputmark
        ],
        cluster: [
            AirColorPair::new(rgb(0x00, 0x00, 0x00), rgb(0x85, 0xa9, 0x18)), // ClusterNormal ← [dialog] _default_
            AirColorPair::new(rgb(0x00, 0x00, 0x00), rgb(0x69, 0x88, 0x0c)), // ClusterSelected ← [dialog] dfocus
            AirColorPair::new(rgb(0x00, 0x00, 0x00), rgb(0x85, 0xa9, 0x18)).with_attributes(
                AirAttributes {
                    underline: true,
                    ..AirAttributes::NONE
                },
            ), // ClusterHotkey ← [dialog] dhotnormal
        ],
        shadow: AirColorPair::new(rgb(0x7f, 0x7f, 0x55), rgb(0x4c, 0x10, 0x02)), // WindowShadow ← [core] shadow
    })
}

/// Le jeu de glyphes, compilé depuis la section `[Lines]` du skin.
///
/// **Douze** glyphes ne viennent pas du skin, parce qu'aucun skin ne les décrit :
/// le remplissage du bureau — `mc` n'a pas de bureau, ses panneaux couvrent
/// l'écran —, les deux crochets de la case de fermeture, dont `mc` ne donne que la
/// marque, les deux flèches d'ascenseur et son curseur, que `mc` n'a pas, et les
/// six caractères des cases à cocher et des boutons radio.
///
/// Ces six derniers restent donc ceux du thème par défaut, et ce n'est **pas** une
/// transcription : aucune des trente-neuf fournitures de `mc` 4.8.33 ne porte de
/// clef pour eux, ce qui a été vérifié et non supposé.
pub(crate) const fn glyphs() -> AirGlyphSet {
    AirGlyphSet {
        window_frame: AirFrameGlyphs {
            top_left: '┌',
            top_right: '┐',
            bottom_left: '└',
            bottom_right: '┘',
            horizontal: '─',
            vertical: '│',
        },
        dialog_frame: AirFrameGlyphs {
            top_left: '╔',
            top_right: '╗',
            bottom_left: '╚',
            bottom_right: '╝',
            horizontal: '═',
            vertical: '║',
        },
        desktop_fill: ' ',
        close_box_left: '[',
        close_box_mark: '✕', // [widget-editor] window-close-char
        close_box_right: ']',
        scroll_arrow_up: '▲',
        scroll_arrow_down: '▼',
        scroll_thumb: '█',
        check_box: AirClusterGlyphs::CHECK_BOX,
        radio_button: AirClusterGlyphs::RADIO_BUTTON,
    }
}
