// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Les thèmes **compilés** depuis un skin de Midnight Commander — **fichier
//! généré, ne pas éditer**.
//!
//! # Pourquoi compilés
//!
//! L'ADR-008 tranche : « les thèmes sont compilés, pas analysés au runtime ». Un
//! skin est un texte, et le Principe 11 d'Air interdit qu'un texte entre par la
//! porte d'exécution : un outil dédié le compile en artefact vérifié. Un skin mal
//! formé est donc un refus de `cargo xtask import-mc-skin`, jamais un comportement
//! dégradé dans une application.
//!
//! Conséquence directe : air-tui ne sait pas lire un `.ini`, n'embarque aucun
//! analyseur, et n'ouvre aucun fichier pour afficher un thème.
//!
//! # Plusieurs skins à la fois
//!
//! Ce fichier est le **registre** : il déclare les modules et les nomme. Jusqu'au
//! jalon M7i, le dépôt n'en portait qu'un, et importer le second effaçait le
//! premier. Le choix se fait désormais par un nom — `mc:<nom>` sur la ligne de
//! commande des applications —, ce qui lève la limite sans rien lire à
//! l'exécution.
//!
//! Le registre est généré plutôt qu'écrit : la liste des modules et la table des
//! noms disent la même chose deux fois, et deux listes tenues à la main
//! divergent.
//!
//! # Ce que contient ce répertoire
//!
//! Des fichiers **générés**, qui ne s'éditent jamais à la main — la règle des
//! fichiers générés de `docs/CONVENTIONS-fr.md` s'y applique telle quelle,
//! `--check` compris. Leur origine et sa licence sont dans `NOTICE`.

pub(crate) mod default;
pub(crate) mod modarcon16_defbg;
pub(crate) mod seasons_autumn16m;

use crate::theme::AirCompiledSkin;

/// Les thèmes compilés que ce dépôt porte, par ordre de nom.
///
/// Une table de pointeurs de fonction plutôt qu'un `match` : la table se parcourt
/// pour **énumérer** les noms, ce qu'un `match` ne permet pas, et c'est ce dont
/// `--list-themes` a besoin.
pub(crate) const COMPILED: &[AirCompiledSkin] = &[
    AirCompiledSkin {
        name: "default",
        description: default::DESCRIPTION,
        palette: default::palette,
        glyphs: default::glyphs,
    },
    AirCompiledSkin {
        name: "modarcon16_defbg",
        description: modarcon16_defbg::DESCRIPTION,
        palette: modarcon16_defbg::palette,
        glyphs: modarcon16_defbg::glyphs,
    },
    AirCompiledSkin {
        name: "seasons_autumn16m",
        description: seasons_autumn16m::DESCRIPTION,
        palette: seasons_autumn16m::palette,
        glyphs: seasons_autumn16m::glyphs,
    },
];
