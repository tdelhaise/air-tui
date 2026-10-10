// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Le thème Midnight Commander — **fichier généré, ne pas éditer**.
//!
//! Produit par `cargo xtask import-mc-skin` depuis « MoDarCon16-DefaultBG (user) », fichier
//! `/usr/local/share/mc/skins/modarcon16-defbg.ini`. 35 rôles transcrits d'une clef, 3 composés de deux.
//!
//! Empreinte FNV-1a du skin d'origine : `0x716076b9631746f6`. C'est un repère de
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
//! - **`ClusterNormal`** — mc ne distingue pas les cases à cocher du fond de leur dialogue.
//! # Ce que ce skin demande au-delà de deux couleurs
//!
//! 9 rôles portent un attribut du skin, et 15 ont une
//! couleur laissée au terminal. Les deux étaient des motifs de refus avant le
//! jalon M7g ; ils sont désormais transcrits. Pour un rôle composé de deux clefs,
//! les attributs retenus sont ceux de la clef qui donne l'avant-plan.
//!

use crate::cell::{AirAttributes, AirColor};
use crate::theme::glyphs::{AirClusterGlyphs, AirFrameGlyphs, AirGlyphSet};
use crate::theme::palette::{AirColorPair, AirPalette, AirPaletteParts};

/// La description que porte la section `[skin]` du fichier d'origine.
pub(crate) const DESCRIPTION: &str = "MoDarCon16-DefaultBG (user)";

/// Un couple de couleurs de la palette du terminal.
const fn pair(foreground: u8, background: u8) -> AirColorPair {
    AirColorPair::new(AirColor::Indexed(foreground), AirColor::Indexed(background))
}

/// Une couleur de la palette du terminal.
const fn index(value: u8) -> AirColor {
    AirColor::Indexed(value)
}

/// La couleur que le terminal emploie déjà — le `default` de mc.
const fn terminal() -> AirColor {
    AirColor::TerminalDefault
}

/// La palette, compilée depuis le skin.
pub(crate) const fn palette() -> AirPalette {
    AirPalette::from_parts(&AirPaletteParts {
        desktop_background: AirColorPair::new(index(7), terminal()), // DesktopBackground ← [core] _default_
        menu: [
            AirColorPair::new(index(7), terminal()), // MenuBar ← [menu] _default_
            AirColorPair::new(index(8), terminal()), // MenuDisabled ← [menu] menuinactive
            AirColorPair::new(index(3), terminal()), // MenuHotkey ← [menu] menuhot
            AirColorPair::new(index(11), index(2)).with_attributes(AirAttributes {
                bold: true,
                ..AirAttributes::NONE
            }), // MenuSelected ← [menu] menusel
            pair(8, 2), // MenuSelectedDisabled ← [core] disabled + [menu] menusel
            pair(3, 2), // MenuSelectedHotkey ← [menu] menuhotsel
        ],
        status: [
            AirColorPair::new(index(7), terminal()), // StatusLine ← [buttonbar] button
            AirColorPair::new(index(8), terminal()), // StatusDisabled ← [core] disabled + [buttonbar] button
            pair(15, 3),                             // StatusHotkey ← [buttonbar] hotkey
        ],
        window: [
            AirColorPair::new(index(7), terminal()), // WindowFrameActive ← [editor] editframeactive
            AirColorPair::new(index(7), terminal()), // WindowFrameInactive ← [editor] _default_
            AirColorPair::new(index(7), terminal()), // WindowFrameIcon ← [editor] editframeactive
            AirColorPair::new(index(7), terminal()), // WindowTitle ← [editor] editframeactive
            AirColorPair::new(index(7), terminal()), // WindowInterior ← [editor] _default_
            AirColorPair::new(index(7), terminal()), // WindowText ← [editor] _default_
        ],
        dialog: [
            pair(0, 7), // DialogFrameActive ← [dialog] _default_
            pair(8, 7), // DialogFrameInactive ← [core] disabled + [dialog] _default_
            AirColorPair::new(index(11), index(7)).with_attributes(AirAttributes {
                bold: true,
                ..AirAttributes::NONE
            }), // DialogFrameIcon ← [dialog] dtitle
            pair(0, 7), // DialogInterior ← [dialog] _default_
        ],
        button: [
            pair(0, 7), // ButtonNormal ← [dialog] _default_
            AirColorPair::new(index(11), index(2)).with_attributes(AirAttributes {
                bold: true,
                ..AirAttributes::NONE
            }), // ButtonDefault ← [dialog] dfocus
            AirColorPair::new(index(11), index(2)).with_attributes(AirAttributes {
                bold: true,
                ..AirAttributes::NONE
            }), // ButtonFocused ← [dialog] dfocus
            pair(8, 7), // ButtonDisabled ← [core] disabled
            pair(3, 7), // ButtonHotkey ← [dialog] dhotnormal
            pair(7, 0), // ButtonShadow ← [core] shadow
        ],
        scroll_bar: [
            AirColorPair::new(index(7), terminal()), // ScrollBarPage ← [core] _default_
            pair(15, 2),                             // ScrollBarControls ← [core] selected
        ],
        list: [
            AirColorPair::new(index(7), terminal()), // ListNormal ← [core] _default_
            pair(15, 2),                             // ListSelected ← [core] selected
            AirColorPair::new(index(11), terminal()).with_attributes(AirAttributes {
                bold: true,
                ..AirAttributes::NONE
            }), // ListMarked ← [core] marked
            AirColorPair::new(index(11), index(2)).with_attributes(AirAttributes {
                bold: true,
                ..AirAttributes::NONE
            }), // ListMarkedSelected ← [core] markselect
        ],
        editor: [
            AirColorPair::new(index(11), index(2)).with_attributes(AirAttributes {
                bold: true,
                ..AirAttributes::NONE
            }), // EditorSelected ← [editor] editmarked
        ],
        input_line: [
            pair(15, 0), // InputLineText ← [core] input
            AirColorPair::new(index(11), index(2)).with_attributes(AirAttributes {
                bold: true,
                ..AirAttributes::NONE
            }), // InputLineSelected ← [core] inputmark
        ],
        cluster: [
            pair(0, 7), // ClusterNormal ← [dialog] _default_
            AirColorPair::new(index(11), index(2)).with_attributes(AirAttributes {
                bold: true,
                ..AirAttributes::NONE
            }), // ClusterSelected ← [dialog] dfocus
            pair(3, 7), // ClusterHotkey ← [dialog] dhotnormal
        ],
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
        close_box_mark: 'X', // [widget-editor] window-close-char
        close_box_right: ']',
        scroll_arrow_up: '▲',
        scroll_arrow_down: '▼',
        scroll_thumb: '█',
        check_box: AirClusterGlyphs::CHECK_BOX,
        radio_button: AirClusterGlyphs::RADIO_BUTTON,
    }
}
