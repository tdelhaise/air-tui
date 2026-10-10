// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Le thème Midnight Commander — **fichier généré, ne pas éditer**.
//!
//! Produit par `cargo xtask import-mc-skin` depuis « Standard skin », fichier
//! `/usr/local/share/mc/skins/default.ini`. 36 rôles transcrits d'une clef, 3 composés de deux.
//!
//! Empreinte FNV-1a du skin d'origine : `0xefc735864405630c`. C'est un repère de
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
//! - **`WindowShadow`** — mc nomme l'ombre, et trente et un des trente-neuf skins livrés la
//!   définissent. Les huit autres l'héritent de [core] _default_, ce qui la rend invisible
//!   — c'est ce que mc fait aussi.

use crate::cell::AirColor;
use crate::theme::glyphs::{AirClusterGlyphs, AirFrameGlyphs, AirGlyphSet};
use crate::theme::palette::{AirColorPair, AirPalette, AirPaletteParts};

/// La description que porte la section `[skin]` du fichier d'origine.
pub(crate) const DESCRIPTION: &str = "Standard skin";

/// Un couple de couleurs de la palette du terminal.
const fn pair(foreground: u8, background: u8) -> AirColorPair {
    AirColorPair::new(AirColor::Indexed(foreground), AirColor::Indexed(background))
}

/// La palette, compilée depuis le skin.
pub(crate) const fn palette() -> AirPalette {
    AirPalette::from_parts(&AirPaletteParts {
        desktop_background: pair(7, 4), // DesktopBackground ← [core] _default_
        menu: [
            pair(15, 6), // MenuBar ← [menu] _default_
            pair(0, 6),  // MenuDisabled ← [menu] menuinactive
            pair(11, 6), // MenuHotkey ← [menu] menuhot
            pair(15, 0), // MenuSelected ← [menu] menusel
            pair(8, 0),  // MenuSelectedDisabled ← [core] disabled + [menu] menusel
            pair(11, 0), // MenuSelectedHotkey ← [menu] menuhotsel
        ],
        status: [
            pair(0, 6),  // StatusLine ← [buttonbar] button
            pair(8, 6),  // StatusDisabled ← [core] disabled + [buttonbar] button
            pair(15, 0), // StatusHotkey ← [buttonbar] hotkey
        ],
        window: [
            pair(15, 4), // WindowFrameActive ← [editor] editframeactive
            pair(7, 4),  // WindowFrameInactive ← [editor] _default_
            pair(15, 4), // WindowFrameIcon ← [editor] editframeactive
            pair(15, 4), // WindowTitle ← [editor] editframeactive
            pair(7, 4),  // WindowInterior ← [editor] _default_
            pair(7, 4),  // WindowText ← [editor] _default_
        ],
        dialog: [
            pair(0, 7), // DialogFrameActive ← [dialog] _default_
            pair(8, 7), // DialogFrameInactive ← [core] disabled + [dialog] _default_
            pair(4, 7), // DialogFrameIcon ← [dialog] dtitle
            pair(0, 7), // DialogInterior ← [dialog] _default_
        ],
        button: [
            pair(0, 7), // ButtonNormal ← [dialog] _default_
            pair(0, 6), // ButtonDefault ← [dialog] dfocus
            pair(0, 6), // ButtonFocused ← [dialog] dfocus
            pair(8, 7), // ButtonDisabled ← [core] disabled
            pair(4, 7), // ButtonHotkey ← [dialog] dhotnormal
            pair(8, 0), // ButtonShadow ← [core] shadow
        ],
        scroll_bar: [
            pair(7, 4), // ScrollBarPage ← [core] _default_
            pair(0, 6), // ScrollBarControls ← [core] selected
        ],
        list: [
            pair(7, 4),  // ListNormal ← [core] _default_
            pair(0, 6),  // ListSelected ← [core] selected
            pair(11, 4), // ListMarked ← [core] marked
            pair(11, 6), // ListMarkedSelected ← [core] markselect
        ],
        editor: [
            pair(0, 6), // EditorSelected ← [editor] editmarked
        ],
        input_line: [
            pair(0, 6), // InputLineText ← [core] input
            pair(6, 0), // InputLineSelected ← [core] inputmark
        ],
        cluster: [
            pair(0, 7), // ClusterNormal ← [dialog] _default_
            pair(0, 6), // ClusterSelected ← [dialog] dfocus
            pair(4, 7), // ClusterHotkey ← [dialog] dhotnormal
        ],
        shadow: pair(8, 0), // WindowShadow ← [core] shadow
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
            top_left: '┌',
            top_right: '┐',
            bottom_left: '└',
            bottom_right: '┘',
            horizontal: '─',
            vertical: '│',
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
