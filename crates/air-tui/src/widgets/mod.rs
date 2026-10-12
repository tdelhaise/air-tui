// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Les widgets.
//!
//! # État
//!
//! Treize widgets, tous colorés par les rôles du thème et jamais par une couleur écrite en
//! dur :
//!
//! - [`AirPanel`], un conteneur bordé à titre, et [`AirWindow`], une fenêtre à case de
//!   fermeture, en variante fenêtre ou dialogue ;
//! - [`AirStaticText`], une ligne de texte, et [`AirButton`], un bouton à lettre d'accès
//!   rapide et à commande ;
//! - [`AirMenuBar`] et ses [`AirMenu`]/[`AirMenuItem`], à menus déroulants ;
//! - [`AirStatusLine`] et ses [`AirStatusItem`], à raccourcis cliquables ;
//! - [`AirInputLine`], un champ de saisie d'une ligne ;
//! - [`AirScrollBar`], un ascenseur qui **demande** une position sans rien défiler ;
//! - [`AirListBox`], une liste à **une colonne ou plusieurs** qui tient son curseur et
//!   son défilement, et qui demande au contrôleur d'ouvrir ou de marquer un item ;
//! - [`AirCluster`], un groupe de **cases à cocher** ou de **boutons radio** — un seul
//!   type pour les deux, comme `TCluster` chez Borland, parce qu'ils ne diffèrent que par
//!   trois choses ;
//! - [`AirEditorView`], le reflet d'un [`crate::model::AirTextBuffer`] — la seule vue qui
//!   produit des **intentions** au lieu de commandes, parce qu'éditer du texte ne se
//!   réduit pas à un numéro de commande (ADR-007) ;
//! - [`AirPromptDialog`], une question et sa réponse, à poser en modal par
//!   [`crate::application::AirScreen::set_modal`].
//!
//! Ce qui manque encore : une liste à plusieurs colonnes, et le défilement d'un groupe
//! d'items plus long que sa place.
//!
//! # La sélection, et qui la tient
//!
//! Faite au jalon M7d, et pas au même endroit dans les deux widgets :
//!
//! - [`AirEditorView`] la lit dans son [`crate::model::AirTextBuffer`], parce que le
//!   texte est au modèle et que la sélection en fait partie ;
//! - [`AirInputLine`] la tient **elle-même**, parce qu'elle tient déjà son texte. Lui
//!   donner un modèle pour une ligne serait payer l'ADR-007 sans rien en retirer.
//!
//! Les deux partagent la même ancre plutôt que deux bornes ordonnées — c'est ce qui
//! permet de réduire une sélection en revenant en arrière —, les mêmes gestes
//! (`Maj`+flèches, `Maj`+clic, glisser, `Ctrl`+`A`, **double et triple clic**) et les
//! mêmes trois commandes de presse-papiers, qu'elles **demandent** sans jamais le
//! toucher : il est au contrôleur (voir [`crate::clipboard::AirClipboard`]).
//!
//! Le double clic sélectionne le segment sous le pointeur et le triple la ligne entière —
//! dans un champ d'une ligne, tout le texte. [`AirListBox`] l'entend autrement, et c'est
//! normal : un double clic y **ouvre** l'item, comme dans Midnight Commander. Ni l'un ni
//! l'autre ne détecte le geste : la boucle d'application le synthétise, parce qu'un double
//! clic est deux séquences et un délai (voir [`crate::input::clicks`]).
//!
//! # La bordure, et qui la retire
//!
//! Toute vue est bordée par défaut (ADR-010). Les **petits contrôles** la retirent dans
//! leur constructeur : [`AirStaticText`], [`AirButton`], [`AirInputLine`], [`AirMenuBar`],
//! [`AirStatusLine`], [`AirScrollBar`] et [`AirCluster`] le font, parce qu'un contrôle
//! d'une ligne encadré n'a plus de place pour son contenu — et qu'un groupe encadré n'a
//! plus la place de ses libellés. [`AirEditorView`] et [`AirListBox`] la retirent
//! aussi, pour une raison voisine : elles sont le contenu d'une fenêtre qui porte déjà la
//! sienne, et deux cadres emboîtés mangeraient deux lignes. Les **conteneurs** la gardent : [`AirPanel`]
//! et [`AirWindow`] la gardent.
//!
//! C'est un point de revue, pas une habitude : un widget qui laisse le défaut doit avoir
//! une raison.
//!
//! # Deux widgets composent eux-mêmes
//!
//! [`AirMenuBar`] dessine son menu déroulant et [`AirPromptDialog`] ses trois contrôles
//! **sans passer par `children`** : ils tiennent leurs pièces dans des champs typés, les
//! dessinent par des peintres fils et traduisent les coordonnées comme la traversée le
//! fait. La raison est la même dans les deux cas : `children` rend des
//! `Box<dyn AirView>`, et un dialogue dont on ne peut plus lire la réponse ne sert à rien.
//!
//! # Les phases, et qui les demande
//!
//! Deux widgets sortent de la phase du focus, et pour des raisons opposées :
//!
//! - [`AirMenuBar`] est en **pré-traitement**. `Alt`+lettre et `Échap` doivent lui
//!   parvenir avant le champ de saisie qui a le focus, car aucun champ n'a d'usage pour
//!   `Alt`+lettre.
//! - [`AirStatusLine`] est en **post-traitement**. Elle publie des touches de fonction :
//!   les prendre avant la vue focalisée reviendrait à les lui voler.

pub mod button;
pub mod cluster;
pub mod editor_view;
pub mod hotkey;
pub mod input_line;
pub mod list_box;
pub mod menu_bar;
pub mod panel;
pub mod prompt_dialog;
pub mod scroll_bar;
pub mod static_text;
pub mod status_line;
pub mod window;

pub use button::AirButton;
pub use cluster::{AirCluster, AirClusterItem, AirClusterKind};
pub use editor_view::AirEditorView;
pub use hotkey::{AirHotkeyLabel, split_hotkey};
pub use input_line::AirInputLine;
pub use list_box::{AirListBox, AirListItem, AirListRequest};
pub use menu_bar::{AirMenu, AirMenuBar, AirMenuItem};
pub use panel::AirPanel;
pub use prompt_dialog::AirPromptDialog;
pub use scroll_bar::{AirOrientation, AirScrollBar};
pub use static_text::AirStaticText;
pub use status_line::{AirStatusItem, AirStatusLine};
pub use window::AirWindow;
