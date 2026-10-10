// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! La géométrie de la grille de cellules.
//!
//! Les coordonnées sont portées par des newtypes plutôt que par des `u16` nus. La raison
//! n'est pas cosmétique : l'arithmétique défensive exigée par le Principe 2 d'Air
//! interdit l'opération nue, et concentrer les conversions ici évite de semer des
//! `try_into()` dans tout le code de dessin.

/// Un indice de colonne, compté depuis zéro à partir du bord gauche.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct AirColumn(u16);

impl AirColumn {
    /// La colonne la plus à gauche.
    pub const FIRST: Self = Self(0);

    /// Construit un indice de colonne.
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    /// La valeur brute.
    pub const fn value(self) -> u16 {
        self.0
    }

    /// La colonne suivante, bornée à la valeur maximale représentable.
    ///
    /// La saturation est le comportement voulu : un parcours de ligne qui atteindrait le
    /// bord doit s'arrêter sur place, pas repartir à zéro.
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

/// Un indice de ligne, compté depuis zéro à partir du haut.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct AirRow(u16);

impl AirRow {
    /// La ligne la plus haute.
    pub const FIRST: Self = Self(0);

    /// Construit un indice de ligne.
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    /// La valeur brute.
    pub const fn value(self) -> u16 {
        self.0
    }

    /// La ligne suivante, bornée à la valeur maximale représentable.
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

/// Une position sur la grille.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AirPoint {
    /// La colonne.
    pub column: AirColumn,
    /// La ligne.
    pub row: AirRow,
}

impl AirPoint {
    /// L'angle supérieur gauche de la grille.
    pub const ORIGIN: Self = Self {
        column: AirColumn::FIRST,
        row: AirRow::FIRST,
    };

    /// Construit une position.
    pub const fn new(column: AirColumn, row: AirRow) -> Self {
        Self { column, row }
    }
}

/// Une dimension en cellules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AirSize {
    /// Nombre de colonnes.
    pub columns: u16,
    /// Nombre de lignes.
    pub rows: u16,
}

impl AirSize {
    /// Construit une dimension.
    pub const fn new(columns: u16, rows: u16) -> Self {
        Self { columns, rows }
    }

    /// Le nombre de cellules que couvre cette dimension.
    ///
    /// Rend `None` en cas de débordement — une grille dont le produit des dimensions
    /// dépasse `usize` ne peut pas être allouée, et c'est à l'appelant de le traiter plutôt
    /// qu'à cette fonction de choisir un repli.
    ///
    /// `usize::from` plutôt qu'un `as` : la conversion est sans perte et le Principe 2
    /// d'Air interdit `as`. Elle n'est pas disponible en contexte `const`, d'où une
    /// fonction ordinaire.
    pub fn cell_count(self) -> Option<usize> {
        let columns = usize::from(self.columns);
        let rows = usize::from(self.rows);
        columns.checked_mul(rows)
    }

    /// Vrai si l'une des deux dimensions est nulle.
    pub const fn is_empty(self) -> bool {
        self.columns == 0 || self.rows == 0
    }
}

/// Un rectangle de la grille, défini par son angle supérieur gauche et sa dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AirRectangle {
    /// L'angle supérieur gauche.
    pub origin: AirPoint,
    /// La dimension.
    pub size: AirSize,
}

impl AirRectangle {
    /// Construit un rectangle.
    pub const fn new(origin: AirPoint, size: AirSize) -> Self {
        Self { origin, size }
    }

    /// Construit un rectangle depuis ses quatre composantes.
    pub const fn from_parts(column: u16, row: u16, columns: u16, rows: u16) -> Self {
        Self {
            origin: AirPoint::new(AirColumn::new(column), AirRow::new(row)),
            size: AirSize::new(columns, rows),
        }
    }

    /// La première colonne occupée.
    pub const fn left(self) -> u16 {
        self.origin.column.value()
    }

    /// La première ligne occupée.
    pub const fn top(self) -> u16 {
        self.origin.row.value()
    }

    /// La première colonne **après** le rectangle, bornée par saturation.
    ///
    /// La saturation est délibérée : un rectangle qui dépasserait le bord droit de la
    /// grille est traité comme s'arrêtant au bord, ce qui est le comportement attendu d'un
    /// découpage.
    pub const fn right_exclusive(self) -> u16 {
        self.left().saturating_add(self.size.columns)
    }

    /// La première ligne **après** le rectangle, bornée par saturation.
    pub const fn bottom_exclusive(self) -> u16 {
        self.top().saturating_add(self.size.rows)
    }

    /// Le rectangle commun à deux rectangles, s'ils se recouvrent.
    ///
    /// Rend `None` quand ils sont disjoints, plutôt qu'un rectangle de dimension nulle :
    /// un découpage vide et un découpage inexistant ne demandent pas le même traitement,
    /// et l'ADR-021 §1 refuse les sentinelles.
    #[must_use]
    pub fn intersection(self, other: Self) -> Option<Self> {
        let left = self.left().max(other.left());
        let top = self.top().max(other.top());
        let right = self.right_exclusive().min(other.right_exclusive());
        let bottom = self.bottom_exclusive().min(other.bottom_exclusive());
        if right <= left || bottom <= top {
            return None;
        }
        Some(Self::from_parts(
            left,
            top,
            right.saturating_sub(left),
            bottom.saturating_sub(top),
        ))
    }

    /// Le même rectangle, déplacé de `delta` colonnes et lignes.
    ///
    /// Le déplacement sature : un rectangle poussé au-delà du bord de la grille s'y
    /// arrête, il ne repart pas de l'autre côté.
    #[must_use]
    pub fn translated(self, delta: AirPoint) -> Self {
        Self::new(
            AirPoint::new(
                AirColumn::new(self.left().saturating_add(delta.column.value())),
                AirRow::new(self.top().saturating_add(delta.row.value())),
            ),
            self.size,
        )
    }

    /// Le même rectangle, rétréci de `margin` cellules sur chaque bord.
    ///
    /// Rend `None` quand la marge ne laisse rien : c'est le cas d'une vue bordée trop
    /// petite pour avoir un intérieur, et l'appelant doit le savoir plutôt que de recevoir
    /// un rectangle vide (ADR-010).
    #[must_use]
    pub fn inset(self, margin: u16) -> Option<Self> {
        let doubled = margin.checked_mul(2)?;
        if self.size.columns <= doubled || self.size.rows <= doubled {
            return None;
        }
        Some(Self::from_parts(
            self.left().saturating_add(margin),
            self.top().saturating_add(margin),
            self.size.columns.saturating_sub(doubled),
            self.size.rows.saturating_sub(doubled),
        ))
    }

    /// Vrai si la position est à l'intérieur du rectangle.
    pub const fn contains(self, point: AirPoint) -> bool {
        let column = point.column.value();
        let row = point.row.value();
        column >= self.left()
            && column < self.right_exclusive()
            && row >= self.top()
            && row < self.bottom_exclusive()
    }
}

#[cfg(test)]
mod tests {
    use super::{AirColumn, AirPoint, AirRectangle, AirRow, AirSize};

    #[test]
    fn la_colonne_suivante_sature_au_maximum() {
        assert_eq!(AirColumn::new(u16::MAX).next(), AirColumn::new(u16::MAX));
        assert_eq!(AirColumn::new(3).next(), AirColumn::new(4));
    }

    #[test]
    fn la_ligne_suivante_sature_au_maximum() {
        assert_eq!(AirRow::new(u16::MAX).next(), AirRow::new(u16::MAX));
    }

    #[test]
    fn le_nombre_de_cellules_est_le_produit_des_dimensions() {
        assert_eq!(AirSize::new(80, 24).cell_count(), Some(1920));
        assert_eq!(AirSize::new(0, 24).cell_count(), Some(0));
    }

    #[test]
    fn une_dimension_nulle_est_vide() {
        assert!(AirSize::new(0, 24).is_empty());
        assert!(AirSize::new(80, 0).is_empty());
        assert!(!AirSize::new(80, 24).is_empty());
    }

    #[test]
    fn les_bords_exclusifs_saturent_au_lieu_de_deborder() {
        let rectangle = AirRectangle::from_parts(u16::MAX, u16::MAX, 10, 10);
        assert_eq!(rectangle.right_exclusive(), u16::MAX);
        assert_eq!(rectangle.bottom_exclusive(), u16::MAX);
    }

    #[test]
    fn deux_rectangles_qui_se_recouvrent_ont_une_intersection() {
        let first = AirRectangle::from_parts(0, 0, 10, 10);
        let second = AirRectangle::from_parts(5, 5, 10, 10);
        assert_eq!(
            first.intersection(second),
            Some(AirRectangle::from_parts(5, 5, 5, 5))
        );
    }

    #[test]
    fn l_intersection_est_commutative() {
        let first = AirRectangle::from_parts(2, 3, 8, 4);
        let second = AirRectangle::from_parts(4, 1, 3, 9);
        assert_eq!(first.intersection(second), second.intersection(first));
    }

    #[test]
    fn deux_rectangles_disjoints_n_ont_pas_d_intersection() {
        // `None` et non un rectangle vide : un découpage inexistant et un découpage vide
        // ne demandent pas le même traitement.
        let first = AirRectangle::from_parts(0, 0, 5, 5);
        let far = AirRectangle::from_parts(10, 10, 5, 5);
        assert_eq!(first.intersection(far), None);
        // Adjacents sans se toucher : le bord droit exclusif de l'un est l'origine de
        // l'autre.
        let adjacent = AirRectangle::from_parts(5, 0, 5, 5);
        assert_eq!(first.intersection(adjacent), None);
    }

    #[test]
    fn un_rectangle_contenu_est_sa_propre_intersection() {
        let outer = AirRectangle::from_parts(0, 0, 20, 20);
        let inner = AirRectangle::from_parts(5, 5, 3, 3);
        assert_eq!(outer.intersection(inner), Some(inner));
    }

    #[test]
    fn un_deplacement_translate_l_origine_sans_changer_la_dimension() {
        let rectangle = AirRectangle::from_parts(2, 3, 4, 5);
        let moved = rectangle.translated(AirPoint::new(AirColumn::new(10), AirRow::new(20)));
        assert_eq!(moved, AirRectangle::from_parts(12, 23, 4, 5));
    }

    #[test]
    fn un_deplacement_demesure_sature_au_lieu_de_repartir_a_zero() {
        let rectangle = AirRectangle::from_parts(u16::MAX, 0, 4, 5);
        let moved = rectangle.translated(AirPoint::new(AirColumn::new(10), AirRow::FIRST));
        assert_eq!(moved.left(), u16::MAX);
    }

    #[test]
    fn un_retrait_d_une_cellule_laisse_un_interieur() {
        let rectangle = AirRectangle::from_parts(0, 0, 10, 6);
        assert_eq!(
            rectangle.inset(1),
            Some(AirRectangle::from_parts(1, 1, 8, 4))
        );
    }

    #[test]
    fn un_retrait_nul_rend_le_meme_rectangle() {
        let rectangle = AirRectangle::from_parts(3, 4, 10, 6);
        assert_eq!(rectangle.inset(0), Some(rectangle));
    }

    #[test]
    fn un_retrait_qui_ne_laisse_rien_rend_none() {
        // Une vue bordée de deux cellules de large n'a pas d'intérieur : l'appelant doit
        // le savoir (ADR-010).
        assert_eq!(AirRectangle::from_parts(0, 0, 2, 10).inset(1), None);
        assert_eq!(AirRectangle::from_parts(0, 0, 10, 2).inset(1), None);
        assert_eq!(AirRectangle::from_parts(0, 0, 1, 1).inset(1), None);
    }

    #[test]
    fn un_retrait_demesure_rend_none_sans_deborder() {
        assert_eq!(AirRectangle::from_parts(0, 0, 10, 10).inset(u16::MAX), None);
    }

    #[test]
    fn contains_inclut_l_origine_et_exclut_le_bord_oppose() {
        let rectangle = AirRectangle::from_parts(2, 3, 4, 5);
        assert!(rectangle.contains(AirPoint::new(AirColumn::new(2), AirRow::new(3))));
        assert!(rectangle.contains(AirPoint::new(AirColumn::new(5), AirRow::new(7))));
        assert!(!rectangle.contains(AirPoint::new(AirColumn::new(6), AirRow::new(7))));
        assert!(!rectangle.contains(AirPoint::new(AirColumn::new(5), AirRow::new(8))));
        assert!(!rectangle.contains(AirPoint::new(AirColumn::new(1), AirRow::new(3))));
    }
}
