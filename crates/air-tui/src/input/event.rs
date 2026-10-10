// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Le modèle d'événements d'entrée.
//!
//! La forme est celle posée par l'ADR-003 : une touche porte toujours son identité
//! **logique**, et seulement parfois son identité **physique**.

use crate::geometry::AirPoint;

/// Le code matériel d'une touche, tel que le noyau le rapporte.
///
/// Disponible uniquement sur la console Linux locale, via `evdev` (jalon M8). À travers un
/// pty — donc en SSH, et sur macOS — il n'existe pas : l'émulateur de terminal a déjà
/// traduit la frappe en séquence d'échappement et jeté le reste.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct AirScanCode(u32);

impl AirScanCode {
    /// Construit un code matériel.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// La valeur brute.
    #[must_use]
    pub const fn value(self) -> u32 {
        self.0
    }
}

/// L'identité logique d'une touche.
///
/// Les modificateurs n'y figurent pas : `Ctrl+C` est `Character('c')` accompagné de
/// [`AirModifiers`] dont `control` est vrai. Ce modèle décomposé évite l'explosion
/// combinatoire des variantes et rend les comparaisons évidentes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AirKey {
    /// Un caractère imprimable, ou le caractère de base d'une combinaison.
    Character(char),
    /// Entrée.
    Enter,
    /// Échappement.
    Escape,
    /// Retour arrière.
    Backspace,
    /// Tabulation. `Shift+Tab` est cette touche avec `shift` actif, et non une variante.
    Tab,
    /// Insertion.
    Insert,
    /// Suppression.
    Delete,
    /// Flèche vers la gauche.
    Left,
    /// Flèche vers la droite.
    Right,
    /// Flèche vers le haut.
    Up,
    /// Flèche vers le bas.
    Down,
    /// Début de ligne.
    Home,
    /// Fin de ligne.
    End,
    /// Page précédente.
    PageUp,
    /// Page suivante.
    PageDown,
    /// Une touche de fonction, de 1 à 35.
    ///
    /// Au-delà de `F12`, seul le protocole clavier Kitty sait les rapporter : le codage
    /// historique n'a pas de place pour elles.
    Function(u8),
}

/// L'état d'une touche au moment de l'événement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AirKeyState {
    /// Enfoncement.
    #[default]
    Press,
    /// Répétition automatique.
    Repeat,
    /// Relâchement.
    ///
    /// Un pty ordinaire ne le rapporte pas : il faut le protocole clavier Kitty (jalon M2)
    /// ou `evdev` (M8).
    Release,
}

/// Les modificateurs actifs.
///
/// Une structure de booléens nommés plutôt qu'un masque de bits, comme pour
/// [`crate::cell::AirAttributes`] : le Principe 7 d'Air préfère la verbosité qui clarifie.
/// Les noms sont ceux des touches, sans abréviation (ADR-004).
// Quatre booléens, et c'est la bonne forme : chacun correspond à une touche physique
// indépendante. Les regrouper en masque de bits rendrait `modifiers.control` illisible et
// ferait écrire des comparaisons de bits dans chaque widget.
#[allow(
    clippy::struct_excessive_bools,
    reason = "un booléen par touche modificatrice"
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AirModifiers {
    /// Majuscule.
    pub shift: bool,
    /// Contrôle.
    pub control: bool,
    /// Alternative — la touche marquée « Alt ».
    pub alternate: bool,
    /// Touche système, dite aussi « méta » ou « super ».
    pub super_key: bool,
}

impl AirModifiers {
    /// Aucun modificateur.
    pub const NONE: Self = Self {
        shift: false,
        control: false,
        alternate: false,
        super_key: false,
    };

    /// Les mêmes modificateurs, avec contrôle actif.
    #[must_use]
    pub const fn with_control(mut self) -> Self {
        self.control = true;
        self
    }

    /// Les mêmes modificateurs, avec majuscule active.
    #[must_use]
    pub const fn with_shift(mut self) -> Self {
        self.shift = true;
        self
    }

    /// Les mêmes modificateurs, avec alternative active.
    #[must_use]
    pub const fn with_alternate(mut self) -> Self {
        self.alternate = true;
        self
    }

    /// Vrai si aucun modificateur n'est actif.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        !self.shift && !self.control && !self.alternate && !self.super_key
    }
}

/// Un événement de clavier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AirKeyEvent {
    /// L'identité logique de la touche — **toujours présente**.
    pub logical: AirKey,
    /// Les modificateurs actifs.
    pub modifiers: AirModifiers,
    /// Enfoncement, répétition ou relâchement.
    pub state: AirKeyState,
    /// Le code matériel, quand la source peut le fournir.
    ///
    /// **Aucun widget d'air-tui ne doit en dépendre** (ADR-003) : un widget qui le ferait
    /// fonctionnerait sur la console de son auteur et casserait chez tout utilisateur
    /// distant.
    pub physical: Option<AirScanCode>,
    /// La touche telle que la **disposition de base** la nomme, quand la source la fournit.
    ///
    /// Seul le protocole clavier Kitty la rapporte. C'est ce qui permet à `Alt+F` d'ouvrir
    /// le menu « Fichier » sur un clavier AZERTY comme sur un QWERTY : un raccourci se
    /// compare à la disposition de base, pas au caractère produit.
    ///
    /// Contrairement à [`Self::physical`], un widget **peut** s'en servir — mais en
    /// repli sur [`Self::logical`] quand elle est absente, ce qui est le cas de tout
    /// terminal sans protocole Kitty.
    pub base_layout: Option<AirKey>,
}

impl AirKeyEvent {
    /// Un enfoncement de touche sans modificateur ni code matériel.
    #[must_use]
    pub const fn press(logical: AirKey) -> Self {
        Self {
            logical,
            modifiers: AirModifiers::NONE,
            state: AirKeyState::Press,
            physical: None,
            base_layout: None,
        }
    }

    /// La touche sur laquelle comparer un raccourci.
    ///
    /// La disposition de base quand la source la fournit, le caractère produit sinon.
    /// C'est la seule comparaison qui fonctionne à la fois sur un terminal moderne et sur
    /// un terminal historique.
    #[must_use]
    pub const fn shortcut_key(&self) -> AirKey {
        match self.base_layout {
            Some(key) => key,
            None => self.logical,
        }
    }

    /// Le même événement, avec d'autres modificateurs.
    #[must_use]
    pub const fn with_modifiers(mut self, modifiers: AirModifiers) -> Self {
        self.modifiers = modifiers;
        self
    }
}

/// Un bouton de souris.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AirMouseButton {
    /// Bouton gauche.
    Left,
    /// Bouton du milieu.
    Middle,
    /// Bouton droit.
    Right,
}

/// Ce qui est arrivé à la souris.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AirMouseEventKind {
    /// Un bouton vient d'être enfoncé.
    Pressed(AirMouseButton),
    /// Un bouton vient d'être relâché.
    ///
    /// Le bouton est optionnel : l'encodage SGR permet de signaler un relâchement sans
    /// dire lequel, et inventer une réponse serait une devinette.
    Released(Option<AirMouseButton>),
    /// Déplacement, un bouton éventuellement maintenu.
    Moved(Option<AirMouseButton>),
    /// Molette vers le haut.
    WheelUp,
    /// Molette vers le bas.
    WheelDown,
    /// Molette vers la gauche.
    WheelLeft,
    /// Molette vers la droite.
    WheelRight,
}

/// Un événement de souris.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AirMouseEvent {
    /// La position, en cellules, comptée depuis zéro.
    ///
    /// Le terminal compte depuis un ; la conversion est faite par le décodeur, pour que le
    /// reste du framework n'ait qu'un seul système de coordonnées.
    pub position: AirPoint,
    /// Ce qui est arrivé.
    pub kind: AirMouseEventKind,
    /// Les modificateurs actifs pendant l'événement.
    pub modifiers: AirModifiers,
}

/// Un événement d'entrée, tel que le décodeur le produit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AirInputEvent {
    /// Une touche.
    Key(AirKeyEvent),
    /// La souris.
    Mouse(AirMouseEvent),
    /// Un collage encadré, livré en un seul morceau.
    ///
    /// Le texte collé ne traverse **pas** le chemin des touches : le distinguer est ce qui
    /// permet à un éditeur de ne pas réindenter automatiquement cent lignes collées.
    Paste(String),
    /// Le terminal a reçu le focus.
    FocusGained,
    /// Le terminal a perdu le focus.
    FocusLost,
    /// Le terminal a rapporté les fonctions du protocole clavier Kitty qu'il applique.
    ///
    /// Réponse à [`crate::input::kitty::QUERY_SEQUENCE`]. Son arrivée **est** la preuve
    /// que le protocole est pris en charge.
    KeyboardProtocolReported(crate::input::kitty::AirKeyboardFlags),
    /// Le terminal a rapporté ses attributs de périphérique.
    ///
    /// Sert de **jalon** à la négociation : les terminaux répondent dans l'ordre, donc
    /// voir ce rapport sans avoir vu de réponse Kitty avant lui est une conclusion
    /// négative certaine — et non l'expiration d'un délai choisi au hasard.
    DeviceAttributesReported,
}

#[cfg(test)]
mod tests {
    use super::{
        AirKey, AirKeyEvent, AirKeyState, AirModifiers, AirMouseButton, AirMouseEvent,
        AirMouseEventKind, AirScanCode,
    };
    use crate::geometry::AirPoint;

    #[test]
    fn un_code_materiel_conserve_sa_valeur() {
        assert_eq!(AirScanCode::new(42).value(), 42);
    }

    #[test]
    fn une_frappe_simple_n_a_ni_modificateur_ni_code_materiel() {
        let event = AirKeyEvent::press(AirKey::Enter);
        assert_eq!(event.logical, AirKey::Enter);
        assert!(event.modifiers.is_empty());
        assert_eq!(event.state, AirKeyState::Press);
        assert_eq!(event.physical, None);
        assert_eq!(event.base_layout, None);
    }

    #[test]
    fn un_raccourci_se_compare_a_la_disposition_de_base_quand_elle_existe() {
        // Sur un AZERTY, `Alt+F` produit bien `f` ; mais sur une disposition où la touche
        // physique du `f` produit autre chose, c'est la disposition de base qui tranche.
        let without = AirKeyEvent::press(AirKey::Character('ф'));
        assert_eq!(without.shortcut_key(), AirKey::Character('ф'));

        let mut with_base = without;
        with_base.base_layout = Some(AirKey::Character('a'));
        assert_eq!(with_base.shortcut_key(), AirKey::Character('a'));
    }

    #[test]
    fn les_constructeurs_de_modificateurs_sont_cumulables() {
        let modifiers = AirModifiers::NONE
            .with_control()
            .with_shift()
            .with_alternate();
        assert!(modifiers.control && modifiers.shift && modifiers.alternate);
        assert!(!modifiers.super_key);
        assert!(!modifiers.is_empty());
    }

    #[test]
    fn with_modifiers_remplace_les_modificateurs() {
        let event = AirKeyEvent::press(AirKey::Character('c'))
            .with_modifiers(AirModifiers::NONE.with_control());
        assert!(event.modifiers.control);
    }

    #[test]
    fn l_etat_par_defaut_est_l_enfoncement() {
        assert_eq!(AirKeyState::default(), AirKeyState::Press);
    }

    #[test]
    fn un_evenement_de_souris_porte_sa_position_et_son_genre() {
        let event = AirMouseEvent {
            position: AirPoint::ORIGIN,
            kind: AirMouseEventKind::Pressed(AirMouseButton::Left),
            modifiers: AirModifiers::NONE,
        };
        assert_eq!(event.position, AirPoint::ORIGIN);
        assert_eq!(event.kind, AirMouseEventKind::Pressed(AirMouseButton::Left));
    }
}
