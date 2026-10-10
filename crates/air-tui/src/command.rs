// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Les commandes, et leur jeu d'activation.
//!
//! # Pourquoi un jeu d'activation plutôt que des observateurs
//!
//! Griser un item de menu, un bouton et un item de barre de statut quand « Enregistrer »
//! n'a pas de sens est le cas d'usage pour lequel on serait tenté d'écrire un
//! observateur. Turbo Vision l'a résolu autrement en 1990, et mieux : **personne n'est
//! notifié**. Chaque vue interroge le jeu au moment où elle se dessine.
//!
//! C'est du *pull*, et c'est le mécanisme 3 de l'ADR-007 : il n'y a aucune arête à tenir à
//! jour, donc aucune à oublier. Désactiver une commande grise d'un coup tout ce qui la
//! porte, y compris les widgets créés après la désactivation.

/// Le nombre de mots de soixante-quatre bits nécessaires pour couvrir tous les `u16`.
const WORD_COUNT: usize = 1024;

/// L'identifiant d'une commande.
///
/// Un entier plutôt qu'une énumération : une application définit ses propres commandes, et
/// une énumération fermée par le framework l'en empêcherait. C'est le choix de Turbo
/// Vision, et il n'a pas vieilli.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AirCommand(u16);

impl AirCommand {
    /// Construit une commande.
    #[must_use]
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    /// La valeur brute.
    #[must_use]
    pub const fn value(self) -> u16 {
        self.0
    }
}

// Les commandes que le framework reconnaît lui-même. Une application est libre d'employer
// tout autre numéro ; ceux-ci sont réservés pour que deux applications ne donnent pas deux
// sens au même geste.
//
// Elles vivent directement dans ce module plutôt que dans un sous-module : le chemin
// `SAVE` est alors celui du module lui-même, sans redondance.

/// Valider et fermer.
pub const OK: AirCommand = AirCommand::new(1);
/// Annuler et fermer.
pub const CANCEL: AirCommand = AirCommand::new(2);
/// Quitter l'application.
pub const QUIT: AirCommand = AirCommand::new(3);
/// Fermer la fenêtre courante.
pub const CLOSE: AirCommand = AirCommand::new(4);
/// Enregistrer.
pub const SAVE: AirCommand = AirCommand::new(5);
/// Ouvrir.
pub const OPEN: AirCommand = AirCommand::new(6);
/// Le premier numéro laissé aux applications.
pub const FIRST_APPLICATION_COMMAND: AirCommand = AirCommand::new(256);

/// L'ensemble des commandes désactivées.
///
/// On stocke les **désactivées** et non les activées : une commande inconnue du jeu est
/// donc active, ce qui est le bon défaut — une application n'a pas à déclarer tout ce
/// qu'elle autorise.
#[derive(Clone)]
pub struct AirCommandSet {
    /// Un bit par commande ; à un quand la commande est désactivée.
    disabled: [u64; WORD_COUNT],
}

impl AirCommandSet {
    /// Un jeu où toutes les commandes sont actives.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            disabled: [0; WORD_COUNT],
        }
    }

    /// La place d'une commande dans le tableau de bits : le mot, et le bit dans ce mot.
    #[allow(
        clippy::arithmetic_side_effects,
        reason = "le reste d'une division par soixante-quatre est toujours un décalage valide"
    )]
    fn position(command: AirCommand) -> (usize, u64) {
        let value = usize::from(command.value());
        let word = value / 64;
        let bit = 1_u64 << (value % 64);
        (word, bit)
    }

    /// Vrai si la commande est active.
    #[must_use]
    pub fn is_enabled(&self, command: AirCommand) -> bool {
        let (word, bit) = Self::position(command);
        match self.disabled.get(word) {
            Some(bits) => bits & bit == 0,
            // Inatteignable : le tableau couvre tous les `u16`. Une commande qu'on ne sait
            // pas situer est traitée comme active, ce qui est le défaut du jeu.
            None => true,
        }
    }

    /// Active une commande.
    pub fn enable(&mut self, command: AirCommand) {
        let (word, bit) = Self::position(command);
        if let Some(bits) = self.disabled.get_mut(word) {
            *bits &= !bit;
        }
    }

    /// Désactive une commande.
    pub fn disable(&mut self, command: AirCommand) {
        let (word, bit) = Self::position(command);
        if let Some(bits) = self.disabled.get_mut(word) {
            *bits |= bit;
        }
    }

    /// Active ou désactive une commande selon `enabled`.
    ///
    /// La forme qu'une application emploie le plus : `set_enabled(SAVE, document.is_dirty())`
    /// se relit mieux qu'un `if`.
    pub fn set_enabled(&mut self, command: AirCommand, enabled: bool) {
        if enabled {
            self.enable(command);
        } else {
            self.disable(command);
        }
    }
}

impl Default for AirCommandSet {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for AirCommandSet {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Afficher huit kilooctets de bits n'aiderait personne : on compte.
        let disabled_count: u32 = self.disabled.iter().map(|bits| bits.count_ones()).sum();
        formatter
            .debug_struct("AirCommandSet")
            .field("disabled_count", &disabled_count)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AirCommand, AirCommandSet, CANCEL, CLOSE, FIRST_APPLICATION_COMMAND, OK, OPEN, QUIT, SAVE,
        WORD_COUNT,
    };

    #[test]
    fn toutes_les_commandes_sont_actives_par_defaut() {
        // Une application n'a pas à déclarer tout ce qu'elle autorise.
        let set = AirCommandSet::new();
        assert!(set.is_enabled(SAVE));
        assert!(set.is_enabled(AirCommand::new(0)));
        assert!(set.is_enabled(AirCommand::new(u16::MAX)));
        assert!(AirCommandSet::default().is_enabled(OK));
    }

    #[test]
    fn desactiver_puis_reactiver_rend_l_etat_initial() {
        let mut set = AirCommandSet::new();
        set.disable(SAVE);
        assert!(!set.is_enabled(SAVE));
        set.enable(SAVE);
        assert!(set.is_enabled(SAVE));
    }

    #[test]
    fn desactiver_une_commande_n_affecte_pas_ses_voisines() {
        // Le piège d'un masque de bits mal placé : désactiver 5 désactive aussi 69.
        let mut set = AirCommandSet::new();
        set.disable(AirCommand::new(5));
        assert!(!set.is_enabled(AirCommand::new(5)));
        assert!(set.is_enabled(AirCommand::new(4)));
        assert!(set.is_enabled(AirCommand::new(6)));
        assert!(set.is_enabled(AirCommand::new(69)));
        assert!(set.is_enabled(AirCommand::new(5 + 64)));
    }

    #[test]
    fn set_enabled_suit_son_argument() {
        let mut set = AirCommandSet::new();
        set.set_enabled(SAVE, false);
        assert!(!set.is_enabled(SAVE));
        set.set_enabled(SAVE, true);
        assert!(set.is_enabled(SAVE));
    }

    #[test]
    fn tout_le_domaine_des_commandes_est_couvert() {
        // Le tableau doit couvrir les 65 536 valeurs : une commande hors du tableau serait
        // silencieusement toujours active.
        assert_eq!(WORD_COUNT * 64, 65_536);
        let mut set = AirCommandSet::new();
        for value in [0_u16, 1, 63, 64, 65, 1000, 32_767, 65_534, u16::MAX] {
            let command = AirCommand::new(value);
            set.disable(command);
            assert!(!set.is_enabled(command), "pour {value}");
            set.enable(command);
            assert!(set.is_enabled(command), "pour {value}");
        }
    }

    #[test]
    fn les_commandes_reservees_ont_des_numeros_distincts() {
        let reserved = [OK, CANCEL, QUIT, CLOSE, SAVE, OPEN];
        let mut values: Vec<u16> = reserved.iter().map(|command| command.value()).collect();
        values.sort_unstable();
        let before = values.len();
        values.dedup();
        assert_eq!(values.len(), before, "deux commandes partagent un numéro");
        // Toutes sous le premier numéro laissé aux applications.
        assert!(
            reserved
                .iter()
                .all(|command| command.value() < FIRST_APPLICATION_COMMAND.value())
        );
    }

    #[test]
    fn une_commande_conserve_sa_valeur() {
        assert_eq!(AirCommand::new(42).value(), 42);
    }

    #[test]
    fn l_affichage_compte_les_desactivees_sans_deverser_les_bits() {
        let mut set = AirCommandSet::new();
        set.disable(SAVE);
        set.disable(OPEN);
        let rendered = format!("{set:?}");
        assert!(rendered.contains("disabled_count"), "rendu : {rendered}");
        assert!(rendered.contains('2'), "rendu : {rendered}");
    }

    #[test]
    fn un_jeu_clone_est_independant() {
        let mut set = AirCommandSet::new();
        set.disable(SAVE);
        let mut cloned = set.clone();
        cloned.enable(SAVE);
        assert!(!set.is_enabled(SAVE));
        assert!(cloned.is_enabled(SAVE));
    }
}
