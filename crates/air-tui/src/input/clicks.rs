// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! La synthèse du double et du triple clic.
//!
//! # Pourquoi ce n'est pas dans le décodeur
//!
//! Un double clic n'est pas une séquence d'octets : c'est **deux séquences et un délai**.
//! Le décodeur est pur — mêmes octets, mêmes événements, toujours —, et c'est cette
//! pureté qui permet au fuzzing de vérifier qu'un découpage arbitraire de l'entrée produit
//! la même suite d'événements (ADR-012). Y mettre une horloge la détruirait : le même flux
//! rejoué donnerait deux résultats.
//!
//! # Pourquoi ce n'est pas dans les widgets
//!
//! Parce que ce serait la même règle écrite douze fois. Le jalon M7c a payé cette leçon
//! sur le filtrage des relâchements de touche : onze widgets exposés et un protégé pour
//! rien. La synthèse vit donc **dans la boucle d'application**, seul endroit qui voit tous
//! les événements avant l'arbre et qui connaît déjà le temps.
//!
//! # Le seuil vient de Turbo Vision, il n'est pas inventé
//!
//! `TEventQueue::doubleDelay` vaut **8 ticks** de l'horloge BIOS, à 18,2 Hz, soit environ
//! **440 ms**. Les trois conditions de `tevent.cpp` sont reprises telles quelles : le
//! **même bouton**, **exactement la même cellule**, et le délai. La condition de position
//! est stricte chez Borland — pas de tolérance d'une cellule —, et elle le reste ici : une
//! cellule de terminal est assez grande pour qu'un double clic y tienne.

use core::time::Duration;

use crate::geometry::AirPoint;
use crate::input::event::{AirMouseButton, AirMouseEventKind};

/// Le délai maximal entre deux enfoncements d'un même clic multiple.
///
/// Transcrit de `TEventQueue::doubleDelay` : 8 ticks à 18,2 Hz. La valeur exacte serait
/// 439,56 ms ; 440 ms est la même à la milliseconde près, et elle se lit.
pub const DEFAULT_MULTIPLE_CLICK_DELAY: Duration = Duration::from_millis(440);

/// Le nombre de clics consécutifs qu'un enfoncement termine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AirClickCount {
    /// Un clic isolé.
    Single,
    /// Le deuxième d'une suite rapide, au même endroit.
    Double,
    /// Le troisième, et au delà — un quatrième clic rapide reste un triple.
    Triple,
}

/// Suit les enfoncements pour en déduire les clics multiples.
///
/// Ne tient **pas** d'horloge : l'instant lui est donné. C'est ce qui rend la règle
/// vérifiable par des essais déterministes, là où un appel à `Instant::now()` à
/// l'intérieur demanderait de faire dormir un test — donc un test lent et instable.
#[derive(Debug, Clone)]
pub struct AirClickTracker<Moment> {
    /// Le délai au delà duquel un enfoncement recommence à zéro.
    delay: Duration,
    /// Le dernier enfoncement retenu : son bouton, sa cellule, son instant, son rang.
    last: Option<(AirMouseButton, AirPoint, Moment, AirClickCount)>,
}

impl<Moment> Default for AirClickTracker<Moment> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Moment> AirClickTracker<Moment> {
    /// Un suiveur au délai de Turbo Vision.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            delay: DEFAULT_MULTIPLE_CLICK_DELAY,
            last: None,
        }
    }

    /// Un suiveur à un autre délai.
    ///
    /// Borland rendait `doubleDelay` réglable par son dialogue de souris ; la même liberté
    /// est laissée ici, sans aller jusqu'à un dialogue.
    #[must_use]
    pub const fn with_delay(delay: Duration) -> Self {
        Self { delay, last: None }
    }

    /// Le délai en vigueur.
    #[must_use]
    pub const fn delay(&self) -> Duration {
        self.delay
    }

    /// Oublie ce qui précède.
    ///
    /// À appeler quand l'enchaînement perd son sens : un changement de dimension du
    /// terminal déplace tout, et la cellule d'avant ne désigne plus la même chose.
    pub fn forget(&mut self) {
        self.last = None;
    }
}

impl<Moment: Copy> AirClickTracker<Moment> {
    /// Enregistre un enfoncement et rend son rang.
    ///
    /// `elapsed` dit combien de temps sépare deux instants ; elle est passée plutôt que
    /// déduite pour que `Moment` ne soit pas forcément un `Instant` — un essai emploie un
    /// compteur de millisecondes, et la boucle d'application un vrai instant.
    pub fn press(
        &mut self,
        button: AirMouseButton,
        position: AirPoint,
        moment: Moment,
        elapsed: impl Fn(Moment, Moment) -> Duration,
    ) -> AirClickCount {
        let count = match self.last {
            // Les trois conditions de Borland, dans son ordre : le bouton, la cellule,
            // le délai.
            Some((last_button, last_position, last_moment, last_count))
                if last_button == button
                    && last_position == position
                    && elapsed(moment, last_moment) <= self.delay =>
            {
                match last_count {
                    AirClickCount::Single => AirClickCount::Double,
                    // Un quatrième clic reste un triple : Borland fait de même, et
                    // remonter plus haut demanderait un geste que personne n'emploie.
                    AirClickCount::Double | AirClickCount::Triple => AirClickCount::Triple,
                }
            }
            _ => AirClickCount::Single,
        };
        self.last = Some((button, position, moment, count));
        count
    }
}

/// L'événement supplémentaire qu'un rang de clic demande, s'il en demande un.
///
/// Un clic isolé n'en demande aucun : il est déjà passé en [`AirMouseEventKind::Pressed`].
/// Les deux autres s'ajoutent **après** lui, et c'est ce qui rend le geste inoffensif pour
/// un widget qui l'ignore — il garde le comportement du simple clic au lieu de ne plus
/// rien recevoir.
#[must_use]
pub const fn extra_event(
    count: AirClickCount,
    button: AirMouseButton,
) -> Option<AirMouseEventKind> {
    match count {
        AirClickCount::Single => None,
        AirClickCount::Double => Some(AirMouseEventKind::DoubleClicked(button)),
        AirClickCount::Triple => Some(AirMouseEventKind::TripleClicked(button)),
    }
}

#[cfg(test)]
mod tests {
    use core::time::Duration;

    use super::{AirClickCount, AirClickTracker, DEFAULT_MULTIPLE_CLICK_DELAY, extra_event};
    use crate::geometry::{AirColumn, AirPoint, AirRow};
    use crate::input::event::{AirMouseButton, AirMouseEventKind};

    /// Un instant d'essai : des millisecondes, pour que la règle soit déterministe.
    type Moment = u64;

    fn elapsed(now: Moment, before: Moment) -> Duration {
        Duration::from_millis(now.saturating_sub(before))
    }

    fn point(column: u16, row: u16) -> AirPoint {
        AirPoint::new(AirColumn::new(column), AirRow::new(row))
    }

    fn tracker() -> AirClickTracker<Moment> {
        AirClickTracker::new()
    }

    #[test]
    fn un_clic_isole_est_simple() {
        let mut tracker = tracker();
        let count = tracker.press(AirMouseButton::Left, point(1, 1), 0, elapsed);
        assert_eq!(count, AirClickCount::Single);
    }

    #[test]
    fn deux_clics_rapides_au_meme_endroit_font_un_double() {
        let mut tracker = tracker();
        tracker.press(AirMouseButton::Left, point(1, 1), 0, elapsed);
        let count = tracker.press(AirMouseButton::Left, point(1, 1), 200, elapsed);
        assert_eq!(count, AirClickCount::Double);
    }

    #[test]
    fn trois_clics_rapides_font_un_triple() {
        let mut tracker = tracker();
        tracker.press(AirMouseButton::Left, point(1, 1), 0, elapsed);
        tracker.press(AirMouseButton::Left, point(1, 1), 200, elapsed);
        let count = tracker.press(AirMouseButton::Left, point(1, 1), 400, elapsed);
        assert_eq!(count, AirClickCount::Triple);
    }

    #[test]
    fn un_quatrieme_clic_reste_un_triple() {
        // Borland fait de même : remonter plus haut demanderait un geste que personne
        // n'emploie.
        let mut tracker = tracker();
        for moment in [0, 200, 400, 600] {
            tracker.press(AirMouseButton::Left, point(1, 1), moment, elapsed);
        }
        let count = tracker.press(AirMouseButton::Left, point(1, 1), 800, elapsed);
        assert_eq!(count, AirClickCount::Triple);
    }

    #[test]
    fn un_clic_trop_tardif_recommence_a_zero() {
        let mut tracker = tracker();
        tracker.press(AirMouseButton::Left, point(1, 1), 0, elapsed);
        let late = u64::try_from(DEFAULT_MULTIPLE_CLICK_DELAY.as_millis())
            .unwrap_or(u64::MAX)
            .saturating_add(1);
        let count = tracker.press(AirMouseButton::Left, point(1, 1), late, elapsed);
        assert_eq!(count, AirClickCount::Single);
    }

    #[test]
    fn un_clic_pile_au_seuil_compte_encore() {
        // La comparaison est `<=`, comme chez Borland : `ev.what - downTicks <= doubleDelay`.
        let mut tracker = tracker();
        tracker.press(AirMouseButton::Left, point(1, 1), 0, elapsed);
        let at_limit = u64::try_from(DEFAULT_MULTIPLE_CLICK_DELAY.as_millis()).unwrap_or(u64::MAX);
        let count = tracker.press(AirMouseButton::Left, point(1, 1), at_limit, elapsed);
        assert_eq!(count, AirClickCount::Double);
    }

    #[test]
    fn un_clic_dans_une_autre_cellule_recommence_a_zero() {
        // La condition de position est stricte chez Borland : pas de tolérance d'une
        // cellule, et une cellule de terminal est assez grande pour qu'un double clic y
        // tienne.
        let mut tracker = tracker();
        tracker.press(AirMouseButton::Left, point(1, 1), 0, elapsed);
        let count = tracker.press(AirMouseButton::Left, point(2, 1), 100, elapsed);
        assert_eq!(count, AirClickCount::Single);
    }

    #[test]
    fn un_clic_d_un_autre_bouton_recommence_a_zero() {
        let mut tracker = tracker();
        tracker.press(AirMouseButton::Left, point(1, 1), 0, elapsed);
        let count = tracker.press(AirMouseButton::Right, point(1, 1), 100, elapsed);
        assert_eq!(count, AirClickCount::Single);
    }

    #[test]
    fn un_double_clic_d_un_autre_bouton_est_possible() {
        let mut tracker = tracker();
        tracker.press(AirMouseButton::Right, point(1, 1), 0, elapsed);
        let count = tracker.press(AirMouseButton::Right, point(1, 1), 100, elapsed);
        assert_eq!(count, AirClickCount::Double);
    }

    #[test]
    fn oublier_coupe_l_enchainement() {
        // Un changement de dimension déplace tout : la cellule d'avant ne désigne plus la
        // même chose.
        let mut tracker = tracker();
        tracker.press(AirMouseButton::Left, point(1, 1), 0, elapsed);
        tracker.forget();
        let count = tracker.press(AirMouseButton::Left, point(1, 1), 100, elapsed);
        assert_eq!(count, AirClickCount::Single);
    }

    #[test]
    fn un_autre_delai_est_respecte() {
        let mut tracker: AirClickTracker<Moment> =
            AirClickTracker::with_delay(Duration::from_millis(50));
        assert_eq!(tracker.delay(), Duration::from_millis(50));
        tracker.press(AirMouseButton::Left, point(1, 1), 0, elapsed);
        let count = tracker.press(AirMouseButton::Left, point(1, 1), 100, elapsed);
        assert_eq!(count, AirClickCount::Single);
    }

    #[test]
    fn un_suiveur_par_defaut_vaut_un_suiveur_neuf() {
        let tracker: AirClickTracker<Moment> = AirClickTracker::default();
        assert_eq!(tracker.delay(), DEFAULT_MULTIPLE_CLICK_DELAY);
    }

    #[test]
    fn seuls_le_double_et_le_triple_ajoutent_un_evenement() {
        // Un clic isolé est déjà passé en `Pressed` : en ajouter un second le dédoublerait.
        assert_eq!(
            extra_event(AirClickCount::Single, AirMouseButton::Left),
            None
        );
        assert_eq!(
            extra_event(AirClickCount::Double, AirMouseButton::Left),
            Some(AirMouseEventKind::DoubleClicked(AirMouseButton::Left))
        );
        assert_eq!(
            extra_event(AirClickCount::Triple, AirMouseButton::Right),
            Some(AirMouseEventKind::TripleClicked(AirMouseButton::Right))
        );
    }
}
