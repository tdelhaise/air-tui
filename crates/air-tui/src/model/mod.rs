// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! La couche modèle — le MVC de l'ADR-007, sans un seul observateur.
//!
//! # L'arête de notification est une donnée
//!
//! Le MVC canonique fait notifier les vues par le modèle. Cette arête est une référence
//! **arrière**, du modèle vers les vues, et c'est précisément ce que Rust rend coûteux :
//! `Rc<RefCell<..>>` transforme une erreur de compilation en panique de production dès
//! qu'une vue modifie le modèle pendant qu'elle est notifiée.
//!
//! Ici, le modèle ne connaît aucune vue et n'appelle personne. Il reçoit une
//! [`AirModel::Action`] et **consigne** ce qui a changé dans un [`AirChangeSet`] que
//! l'appelant lui tend. Le contrôleur, qui tient déjà les deux côtés, lit ce journal et en
//! informe les vues. La réentrance devient impossible par construction.
//!
//! # Deux canaux, et pourquoi deux
//!
//! L'ADR-007 en distingue quatre ; ce module en porte deux, et ils ne servent pas à la
//! même chose :
//!
//! - le **journal de changements** ([`AirChangeSet`]) porte les faits **structurels et
//!   rares** : un fichier chargé, un nombre de lignes qui change, un drapeau de
//!   modification qui bascule. Ils méritent une entrée, parce que réagir à chacun demande
//!   de savoir lequel s'est produit ;
//! - les **compteurs de génération** ([`AirGeneration`]) portent l'état **continu et
//!   fréquent** : le texte lui-même, la position du curseur. Une frappe les incrémente, et
//!   une vue compare en O(1) ce qu'elle a vu à ce qui est. Journaliser chaque frappe
//!   remplirait un `Vec` pour une information qu'un entier suffit à donner.
//!
//! Le partage entre les deux n'est pas une question de goût : un journal de ce qui est
//! fréquent devient une liste que personne ne relit, et un compteur sur ce qui est
//! structurel ne dit pas **quoi** a changé.
//!
//! # Le tour de boucle
//!
//! ```text
//! 1. entrée → intention   les vues lisent le modèle, produisent des `Action`
//! 2. intention → mutation model.apply(action, &mut changes) — les vues sont hors de portée
//! 3. faits → réaction     les vues lisent `changes` et les générations
//! 4. rendu différentiel
//! ```
//!
//! Un seul emprunt mutable est vivant à la fois, à chaque étape.

pub mod text;

pub use text::{AirTextAction, AirTextBuffer, AirTextChange, AirTextPosition};

/// Un compteur de génération monotone.
///
/// Il ne sert qu'à répondre « est-ce que ça a changé depuis la dernière fois ? » en une
/// comparaison. Soixante-quatre bits : à une frappe par nanoseconde, il faudrait cinq
/// siècles pour en faire le tour, donc la saturation n'est pas un cas à traiter mais une
/// impossibilité à documenter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct AirGeneration(u64);

impl AirGeneration {
    /// La génération initiale.
    #[must_use]
    pub const fn new() -> Self {
        Self(0)
    }

    /// Avance d'une génération.
    pub const fn advance(&mut self) {
        self.0 = self.0.saturating_add(1);
    }

    /// La valeur brute.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Le journal des changements d'un tour de boucle.
///
/// Fourni par l'appelant et **réutilisé** d'un tour à l'autre : [`Self::clear`] garde la
/// capacité, donc le chemin chaud n'alloue pas après les premiers tours.
#[derive(Debug, Clone, Default)]
pub struct AirChangeSet<Change> {
    /// Les changements consignés, dans l'ordre où ils se sont produits.
    changes: Vec<Change>,
}

impl<Change> AirChangeSet<Change> {
    /// Un journal vide.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            changes: Vec::new(),
        }
    }

    /// Un journal vide, de capacité donnée.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            changes: Vec::with_capacity(capacity),
        }
    }

    /// Consigne un changement.
    pub fn record(&mut self, change: Change) {
        self.changes.push(change);
    }

    /// Les changements consignés.
    #[must_use]
    pub fn as_slice(&self) -> &[Change] {
        &self.changes
    }

    /// Le nombre de changements consignés.
    #[must_use]
    pub fn len(&self) -> usize {
        self.changes.len()
    }

    /// Vrai si rien n'a été consigné.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// Vide le journal **sans** rendre sa capacité.
    pub fn clear(&mut self) {
        self.changes.clear();
    }
}

impl<Change: PartialEq> AirChangeSet<Change> {
    /// Consigne un changement s'il n'y est pas déjà.
    ///
    /// Une seule action peut toucher plusieurs fois la même structure — coller un texte de
    /// dix lignes change dix fois le nombre de lignes. Le contrôleur n'a besoin de le
    /// savoir qu'une fois, et dédupliquer ici évite à chaque réaction de le faire.
    pub fn record_once(&mut self, change: Change) {
        if !self.changes.contains(&change) {
            self.changes.push(change);
        }
    }

    /// Vrai si ce changement a été consigné.
    #[must_use]
    pub fn contains(&self, change: &Change) -> bool {
        self.changes.contains(change)
    }
}

/// Ce que tout modèle sait faire.
///
/// Une seule méthode, et c'est volontaire : tout ce qui modifie le modèle passe par une
/// action, donc l'ensemble de ce qui peut arriver à l'état est auditable par un `grep` sur
/// l'énumération des actions.
pub trait AirModel {
    /// Ce qu'on peut demander au modèle.
    type Action;
    /// Ce que le modèle rapporte avoir changé.
    type Change;

    /// Applique `action` et consigne les changements structurels dans `changes`.
    ///
    /// Le journal est fourni par l'appelant, jamais tenu par le modèle : c'est ce qui
    /// empêche le modèle de se mettre à connaître qui l'écoute.
    fn apply(&mut self, action: Self::Action, changes: &mut AirChangeSet<Self::Change>);
}

#[cfg(test)]
mod tests {
    use super::{AirChangeSet, AirGeneration, AirModel};

    /// Un changement d'essai, comparable pour éprouver la déduplication.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Change {
        /// Le seul changement de ce modèle d'essai.
        Touched,
        /// Un second, pour vérifier que la déduplication ne confond pas.
        Reset,
    }

    /// Un modèle d'essai minimal : un compteur.
    #[derive(Default)]
    struct Counter {
        /// La valeur courante.
        value: u32,
        /// La génération de la valeur.
        generation: AirGeneration,
    }

    impl AirModel for Counter {
        type Action = u32;
        type Change = Change;

        fn apply(&mut self, action: Self::Action, changes: &mut AirChangeSet<Self::Change>) {
            self.value = self.value.saturating_add(action);
            self.generation.advance();
            changes.record_once(Change::Touched);
        }
    }

    #[test]
    fn une_generation_neuve_vaut_zero() {
        assert_eq!(AirGeneration::new().value(), 0);
        assert_eq!(AirGeneration::default(), AirGeneration::new());
    }

    #[test]
    fn une_generation_avance_d_un_pas() {
        let mut generation = AirGeneration::new();
        generation.advance();
        assert_eq!(generation.value(), 1);
        generation.advance();
        assert_eq!(generation.value(), 2);
    }

    #[test]
    fn une_generation_vue_se_compare_a_celle_du_modele() {
        // C'est tout l'usage : une vue garde ce qu'elle a vu et compare en O(1).
        let mut model = Counter::default();
        let mut changes = AirChangeSet::new();
        let seen = model.generation;
        model.apply(1, &mut changes);
        assert_ne!(seen, model.generation);
    }

    #[test]
    fn un_journal_neuf_est_vide() {
        let changes: AirChangeSet<Change> = AirChangeSet::new();
        assert_eq!(changes.len(), 0);
        assert!(changes.is_empty());
        assert_eq!(changes.as_slice(), [] as [Change; 0]);
    }

    #[test]
    fn un_journal_consigne_dans_l_ordre() {
        let mut changes = AirChangeSet::with_capacity(4);
        changes.record(Change::Touched);
        changes.record(Change::Reset);
        assert_eq!(changes.as_slice(), [Change::Touched, Change::Reset]);
    }

    #[test]
    fn un_journal_dedoublonne_a_la_demande() {
        let mut changes = AirChangeSet::new();
        changes.record_once(Change::Touched);
        changes.record_once(Change::Touched);
        changes.record_once(Change::Reset);
        assert_eq!(changes.len(), 2);
        assert!(changes.contains(&Change::Reset));
    }

    #[test]
    fn un_journal_consigne_deux_fois_quand_on_ne_dedoublonne_pas() {
        let mut changes = AirChangeSet::new();
        changes.record(Change::Touched);
        changes.record(Change::Touched);
        assert_eq!(changes.len(), 2);
    }

    #[test]
    fn vider_un_journal_garde_sa_capacite() {
        // C'est la raison pour laquelle le journal est fourni par l'appelant : le chemin
        // chaud ne doit pas allouer à chaque tour de boucle.
        let mut changes = AirChangeSet::with_capacity(8);
        changes.record(Change::Touched);
        let capacity = changes.changes.capacity();
        changes.clear();
        assert!(changes.is_empty());
        assert_eq!(changes.changes.capacity(), capacity);
    }

    #[test]
    fn un_modele_ne_connait_personne_et_rend_ses_faits() {
        let mut model = Counter::default();
        let mut changes = AirChangeSet::new();
        model.apply(2, &mut changes);
        model.apply(3, &mut changes);
        assert_eq!(model.value, 5);
        // Deux actions, un seul fait structurel : c'est la déduplication qui travaille.
        assert_eq!(changes.as_slice(), [Change::Touched]);
    }
}
