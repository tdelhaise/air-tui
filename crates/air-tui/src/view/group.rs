// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Le groupe — une vue qui en contient d'autres.
//!
//! C'est le patron Composite de Turbo Vision : un bureau est un groupe, une fenêtre est un
//! groupe, un dialogue est un groupe. Le groupe ne sait rien de ce qu'il contient, et ce
//! qu'il contient ne sait rien de lui.

use crate::view::painter::AirPainter;
use crate::view::{AirDrawContext, AirView, AirViewId, AirViewState};

/// Une vue qui contient d'autres vues.
///
/// Les enfants sont rangés de **l'arrière vers l'avant** : le dernier est au premier plan.
/// C'est l'ordre du dessin ; le routage des clics le parcourt à l'envers, parce que c'est
/// la fenêtre du dessus qui reçoit le clic.
pub struct AirGroup {
    /// L'état commun.
    state: AirViewState,
    /// Les vues contenues, de l'arrière vers l'avant.
    children: Vec<Box<dyn AirView>>,
}

impl AirGroup {
    /// Un groupe vide.
    #[must_use]
    pub const fn new(state: AirViewState) -> Self {
        Self {
            state,
            children: Vec::new(),
        }
    }

    /// Ajoute une vue au premier plan et rend son identifiant.
    pub fn insert(&mut self, view: Box<dyn AirView>) -> AirViewId {
        let identifier = view.state().identifier();
        self.children.push(view);
        identifier
    }

    /// Retire la vue d'identifiant donné, si elle est là.
    pub fn remove(&mut self, identifier: AirViewId) -> Option<Box<dyn AirView>> {
        let position = self
            .children
            .iter()
            .position(|child| child.state().identifier() == identifier)?;
        Some(self.children.remove(position))
    }

    /// Le nombre de vues contenues.
    #[must_use]
    pub fn len(&self) -> usize {
        self.children.len()
    }

    /// Vrai si le groupe est vide.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.children.is_empty()
    }

    /// Amène une vue au premier plan.
    ///
    /// Rend `false` quand l'identifiant est inconnu. C'est ce qui fait qu'un clic sur une
    /// fenêtre en arrière-plan la fait passer devant.
    pub fn bring_to_front(&mut self, identifier: AirViewId) -> bool {
        let Some(position) = self
            .children
            .iter()
            .position(|child| child.state().identifier() == identifier)
        else {
            return false;
        };
        let child = self.children.remove(position);
        self.children.push(child);
        true
    }

    /// Donne le focus à une vue et le retire à ses sœurs.
    ///
    /// Rend `false` quand l'identifiant est inconnu. Le focus est **exclusif entre
    /// sœurs** : le tenir ailleurs qu'à un seul endroit par groupe est la façon la plus
    /// sûre de le voir se dédoubler.
    pub fn focus(&mut self, identifier: AirViewId) -> bool {
        let mut found = false;
        for child in &mut self.children {
            let is_target = child.state().identifier() == identifier;
            child.state_mut().set_focused(is_target);
            found |= is_target;
        }
        found
    }

    /// L'identifiant de la vue qui a le focus, s'il y en a une.
    #[must_use]
    pub fn focused(&self) -> Option<AirViewId> {
        self.children
            .iter()
            .find(|child| child.state().is_focused())
            .map(|child| child.state().identifier())
    }

    /// La vue d'identifiant donné, si elle est là.
    #[must_use]
    pub fn child(&self, identifier: AirViewId) -> Option<&dyn AirView> {
        self.children
            .iter()
            .find(|child| child.state().identifier() == identifier)
            .map(Box::as_ref)
    }

    /// La vue d'identifiant donné, en écriture.
    pub fn child_mut(&mut self, identifier: AirViewId) -> Option<&mut (dyn AirView + 'static)> {
        self.children
            .iter_mut()
            .find(|child| child.state().identifier() == identifier)
            .map(Box::as_mut)
    }
}

impl AirView for AirGroup {
    fn state(&self) -> &AirViewState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut AirViewState {
        &mut self.state
    }

    fn draw_content(&self, painter: &mut AirPainter<'_>, context: &AirDrawContext<'_>) {
        // Un groupe n'a pas de contenu propre : ses enfants sont dessinés par la
        // traversée, après cet appel.
        let _ = (painter, context);
    }

    fn children(&self) -> &[Box<dyn AirView>] {
        &self.children
    }

    fn children_mut(&mut self) -> &mut [Box<dyn AirView>] {
        &mut self.children
    }
}

impl core::fmt::Debug for AirGroup {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // `dyn AirView` n'impose pas `Debug` : l'exiger du trait le restreindrait sans
        // rien apporter.
        formatter
            .debug_struct("AirGroup")
            .field("identifier", &self.state.identifier())
            .field("children", &self.children.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::AirGroup;
    use crate::geometry::AirRectangle;
    use crate::view::painter::AirPainter;
    use crate::view::{AirDrawContext, AirView, AirViewId, AirViewState};

    fn identifier(value: u32) -> AirViewId {
        AirViewId::new(value).expect("un identifiant non nul")
    }

    /// Une vue minimale, juste pour peupler un groupe.
    struct Leaf(AirViewState);

    impl AirView for Leaf {
        fn state(&self) -> &AirViewState {
            &self.0
        }
        fn state_mut(&mut self) -> &mut AirViewState {
            &mut self.0
        }
        fn draw_content(&self, _painter: &mut AirPainter<'_>, _context: &AirDrawContext<'_>) {}
    }

    fn leaf(value: u32) -> Box<dyn AirView> {
        Box::new(Leaf(AirViewState::new(
            identifier(value),
            AirRectangle::from_parts(0, 0, 4, 4),
        )))
    }

    fn group_with(values: &[u32]) -> AirGroup {
        let mut group = AirGroup::new(AirViewState::new(
            identifier(100),
            AirRectangle::from_parts(0, 0, 20, 20),
        ));
        for value in values {
            group.insert(leaf(*value));
        }
        group
    }

    fn order(group: &AirGroup) -> Vec<u32> {
        group
            .children()
            .iter()
            .map(|child| child.state().identifier().value())
            .collect()
    }

    #[test]
    fn un_groupe_neuf_est_vide() {
        let group = group_with(&[]);
        assert!(group.is_empty());
        assert_eq!(group.len(), 0);
        assert_eq!(group.focused(), None);
    }

    #[test]
    fn l_insertion_place_au_premier_plan_et_rend_l_identifiant() {
        let mut group = group_with(&[1, 2]);
        assert_eq!(group.insert(leaf(3)), identifier(3));
        // Rangés de l'arrière vers l'avant : le dernier inséré est devant.
        assert_eq!(order(&group), vec![1, 2, 3]);
        assert_eq!(group.len(), 3);
        assert!(!group.is_empty());
    }

    #[test]
    fn le_retrait_rend_la_vue_et_conserve_l_ordre_des_autres() {
        let mut group = group_with(&[1, 2, 3]);
        let removed = group.remove(identifier(2)).expect("la vue est là");
        assert_eq!(removed.state().identifier(), identifier(2));
        assert_eq!(order(&group), vec![1, 3]);
    }

    #[test]
    fn le_retrait_d_un_inconnu_ne_rend_rien() {
        let mut group = group_with(&[1]);
        assert!(group.remove(identifier(9)).is_none());
        assert_eq!(group.len(), 1);
    }

    #[test]
    fn amener_au_premier_plan_deplace_sans_perdre_les_autres() {
        // C'est ce qui fait qu'un clic sur une fenêtre en arrière-plan la fait passer
        // devant.
        let mut group = group_with(&[1, 2, 3]);
        assert!(group.bring_to_front(identifier(1)));
        assert_eq!(order(&group), vec![2, 3, 1]);
    }

    #[test]
    fn amener_au_premier_plan_celle_qui_y_est_deja_ne_change_rien() {
        let mut group = group_with(&[1, 2, 3]);
        assert!(group.bring_to_front(identifier(3)));
        assert_eq!(order(&group), vec![1, 2, 3]);
    }

    #[test]
    fn amener_au_premier_plan_un_inconnu_echoue_sans_rien_toucher() {
        let mut group = group_with(&[1, 2]);
        assert!(!group.bring_to_front(identifier(9)));
        assert_eq!(order(&group), vec![1, 2]);
    }

    #[test]
    fn le_focus_est_exclusif_entre_soeurs() {
        // Le tenir à un seul endroit par groupe est la façon la plus sûre de ne pas le
        // voir se dédoubler.
        let mut group = group_with(&[1, 2, 3]);
        assert!(group.focus(identifier(2)));
        assert_eq!(group.focused(), Some(identifier(2)));
        assert!(group.focus(identifier(3)));
        assert_eq!(group.focused(), Some(identifier(3)));
        let focused: Vec<u32> = group
            .children()
            .iter()
            .filter(|child| child.state().is_focused())
            .map(|child| child.state().identifier().value())
            .collect();
        assert_eq!(focused, vec![3]);
    }

    #[test]
    fn donner_le_focus_a_un_inconnu_echoue_et_le_retire_a_toutes() {
        // Le comportement est voulu : un identifiant inconnu signifie « personne », et
        // laisser l'ancienne focalisée en place donnerait un focus fantôme.
        let mut group = group_with(&[1, 2]);
        assert!(group.focus(identifier(1)));
        assert!(!group.focus(identifier(9)));
        assert_eq!(group.focused(), None);
    }

    #[test]
    fn une_vue_se_retrouve_par_son_identifiant() {
        let mut group = group_with(&[1, 2]);
        assert_eq!(
            group
                .child(identifier(2))
                .map(|child| child.state().identifier()),
            Some(identifier(2))
        );
        assert!(group.child(identifier(9)).is_none());
        let found = group.child_mut(identifier(1)).expect("la vue est là");
        found.state_mut().set_visible(false);
        assert_eq!(
            group
                .child(identifier(1))
                .map(|child| child.state().is_visible()),
            Some(false)
        );
        assert!(group.child_mut(identifier(9)).is_none());
    }

    #[test]
    fn un_groupe_n_a_pas_de_contenu_propre() {
        // Ses enfants sont dessinés par la traversée, pas par lui.
        let mut group = group_with(&[1]);
        assert_eq!(group.children().len(), 1);
        assert_eq!(group.children_mut().len(), 1);
    }

    #[test]
    fn l_affichage_d_un_groupe_resume_sans_exiger_debug_des_enfants() {
        let group = group_with(&[1, 2]);
        let rendered = format!("{group:?}");
        assert!(rendered.contains("AirGroup"), "rendu : {rendered}");
        assert!(rendered.contains('2'), "rendu : {rendered}");
    }
}
