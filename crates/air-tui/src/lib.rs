// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! air-tui — un framework d'interface en mode texte, sans aucune dépendance.
//!
//! # État
//!
//! **Utilisable, incomplet.** Ce qui existe : la géométrie, la cellule, la surface
//! différentielle, le terminal en RAII, le décodeur d'entrée et le protocole clavier
//! Kitty, les tables Unicode, l'arbre de vues à découpage réel, le moteur de thèmes et
//! huit widgets. L'exemple `frame` en fait une application qui répond.
//!
//! Ce qui n'existe **pas** encore : `AirApplication`, `AirScreen` et `AirDesktop` —
//! l'application monte sa boucle et son arbre à la main —, le journal de changements de
//! la couche MVC, l'éditeur de texte, et les listes. Voir `docs/etat-avancement.md`.
//!
//! # Architecture
//!
//! Cette crate est le cœur du framework. Elle est en `#![forbid(unsafe_code)]` et ne
//! dépend **que** du contrat `air-tui-platform` : elle n'a aucune dépendance vers une
//! implémentation de plateforme, et ne peut donc pas atteindre le système, même par
//! accident (ADR-002, ADR-011). C'est le binaire final qui choisit un
//! [`TerminalDevice`] et l'injecte.
//!
//! # Conventions de nommage
//!
//! Les types du modèle d'objets public portent le préfixe `Air` — `AirWindow`,
//! `AirView`, `AirRectangle` — à la manière de Qt (`QWidget`) ou de Cocoa (`NSWindow`),
//! et aucun identifiant n'est abrégé : `column`, jamais `col` ; `foreground`, jamais
//! `fg` (ADR-004).

#![forbid(unsafe_code)]
// `expect` et `unwrap` sont interdits en production (lints de l'espace de travail) et
// légitimes dans un test, où une précondition non tenue doit faire échouer le test
// bruyamment.
#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]

pub mod cell;
pub mod codec;
pub mod command;
pub mod geometry;
pub mod input;
pub mod model;
pub mod surface;
pub mod terminal;
pub mod theme;
pub mod unicode;
pub mod view;
pub mod widgets;

pub use air_tui_platform::{
    Decoder, Encoder, InputReadiness, PlatformError, PlatformErrorKind, TerminalDevice,
};
pub use cell::{AirAttributes, AirCell, AirColor};
pub use codec::{AirCellPresentation, AirSgrEncoder};
pub use command::{AirCommand, AirCommandSet};
pub use geometry::{AirColumn, AirPoint, AirRectangle, AirRow, AirSize};
pub use input::{
    AirAnsiDecoder, AirDecodeError, AirInputEvent, AirKey, AirKeyEvent, AirKeyState, AirModifiers,
    AirMouseButton, AirMouseEvent, AirMouseEventKind, AirScanCode,
};
pub use model::{
    AirChangeSet, AirGeneration, AirModel, AirTextAction, AirTextBuffer, AirTextChange,
    AirTextPosition,
};
pub use surface::AirSurface;
pub use terminal::AirTerminal;
pub use theme::AirTheme;
pub use unicode::{
    AirGraphemeBreak, AirGraphemes, AirIndicConjunctBreak, AirWordBreak, AirWords, UNICODE_VERSION,
    display_width, grapheme_break, grapheme_width, graphemes, indic_conjunct_break,
    is_extended_pictographic, is_word, next_word_start, previous_word_start, text_width,
    word_break, words,
};
pub use view::{
    AirBorderStyle, AirContext, AirEvent, AirGroup, AirOutcome, AirPainter, AirPhase, AirView,
    AirViewId, AirViewState, dispatch_event, draw_view,
};
pub use widgets::{
    AirButton, AirInputLine, AirMenu, AirMenuBar, AirMenuItem, AirPanel, AirStaticText,
    AirStatusItem, AirStatusLine, AirWindow,
};
