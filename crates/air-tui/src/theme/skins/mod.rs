// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Les thèmes **compilés** depuis un skin de Midnight Commander.
//!
//! # Pourquoi compilés
//!
//! L'ADR-008 tranche : « les thèmes sont compilés, pas analysés au runtime ». Un skin est
//! un texte, et le Principe 11 d'Air interdit qu'un texte entre par la porte d'exécution :
//! un outil dédié le compile en artefact vérifié. Un skin mal formé est donc un refus
//! de `cargo xtask import-mc-skin`, jamais un comportement dégradé dans une application.
//!
//! Conséquence directe : air-tui ne sait pas lire un `.ini`, n'embarque aucun analyseur,
//! et n'ouvre aucun fichier pour afficher un thème.
//!
//! # Ce que contient ce répertoire
//!
//! Des fichiers **générés**, qui ne s'éditent jamais à la main — la règle des fichiers
//! générés de `docs/CONVENTIONS-fr.md` s'y applique telle quelle, `--check` compris. Leur
//! origine et sa licence sont dans `NOTICE`.

pub(crate) mod midnight_commander;
