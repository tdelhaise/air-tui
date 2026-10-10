// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Le presse-papiers interne.
//!
//! # Interne, et pourquoi seulement interne
//!
//! Ce presse-papiers vit dans le processus. Il ne parle ni à `pbcopy`, ni à
//! `wl-clipboard`, ni au protocole OSC 52 : ce sont trois mécanismes différents, dont le
//! dernier est refusé par défaut par la plupart des terminaux, et les deux premiers
//! demandent de lancer un programme. Un presse-papiers interne fait fonctionner le
//! couper-coller **dans** l'application, ce qui est ce qu'un framework doit garantir ;
//! l'échange avec le système est un sujet distinct, et il n'est pas traité.
//!
//! # Possédé, jamais global
//!
//! L'application le tient et le prête à son contrôleur. Pas de variable globale, pas de
//! `static` : deux fenêtres du même processus partagent celui que l'application leur
//! donne, et un essai en construit un à lui.

/// Le presse-papiers interne.
///
/// Un seul contenu textuel, écrasé à chaque copie — c'est le comportement de Turbo Vision
/// comme celui de tout éditeur courant. Un historique serait une autre fonction, avec sa
/// propre interface.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AirClipboard {
    /// Le contenu, ou `None` si rien n'a jamais été copié.
    content: Option<String>,
}

impl AirClipboard {
    /// Un presse-papiers vide.
    #[must_use]
    pub const fn new() -> Self {
        Self { content: None }
    }

    /// Y place un texte, en écrasant ce qu'il portait.
    ///
    /// Un texte **vide n'est pas copié** : couper une sélection vide ne doit pas effacer
    /// ce que le presse-papiers portait déjà, sinon un geste sans effet détruirait une
    /// copie que l'utilisateur croyait sûre.
    pub fn set(&mut self, text: impl Into<String>) {
        let text = text.into();
        if text.is_empty() {
            return;
        }
        self.content = Some(text);
    }

    /// Le contenu, ou `None` s'il est vide.
    #[must_use]
    pub fn content(&self) -> Option<&str> {
        self.content.as_deref()
    }

    /// Vrai s'il ne porte rien.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.content.is_none()
    }

    /// Le vide.
    pub fn clear(&mut self) {
        self.content = None;
    }
}

#[cfg(test)]
mod tests {
    use super::AirClipboard;

    #[test]
    fn un_presse_papiers_neuf_est_vide() {
        let clipboard = AirClipboard::new();
        assert!(clipboard.is_empty());
        assert_eq!(clipboard.content(), None);
    }

    #[test]
    fn ce_qui_est_pose_se_relit() {
        let mut clipboard = AirClipboard::new();
        clipboard.set("abc");
        assert_eq!(clipboard.content(), Some("abc"));
        assert!(!clipboard.is_empty());
    }

    #[test]
    fn une_copie_ecrase_la_precedente() {
        let mut clipboard = AirClipboard::new();
        clipboard.set("abc");
        clipboard.set("def");
        assert_eq!(clipboard.content(), Some("def"));
    }

    #[test]
    fn un_texte_vide_ne_detruit_pas_le_contenu() {
        // Couper une sélection vide ne doit pas effacer une copie que l'utilisateur
        // croyait sûre.
        let mut clipboard = AirClipboard::new();
        clipboard.set("abc");
        clipboard.set("");
        assert_eq!(clipboard.content(), Some("abc"));
    }

    #[test]
    fn le_vider_le_rend_vide() {
        let mut clipboard = AirClipboard::new();
        clipboard.set("abc");
        clipboard.clear();
        assert!(clipboard.is_empty());
    }

    #[test]
    fn un_presse_papiers_par_defaut_vaut_un_presse_papiers_neuf() {
        assert_eq!(AirClipboard::default(), AirClipboard::new());
    }
}
