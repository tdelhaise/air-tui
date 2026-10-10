// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! La surface — la grille de cellules et son différentiel.
//!
//! Deux grilles sont tenues : celle que le code vient de composer, et celle qui est
//! actuellement à l'écran. L'écart entre les deux est ce qu'il faut envoyer au terminal.
//!
//! Les deux grilles sont allouées à la construction et réutilisées ensuite : aucune
//! allocation n'a lieu sur le chemin chaud (ADR-021 §4). Seul un redimensionnement
//! réalloue, et il est rare par nature.

use crate::cell::{AirAttributes, AirCell, AirColor};
use crate::geometry::{AirColumn, AirPoint, AirRectangle, AirRow, AirSize};

/// Une grille de cellules, avec le souvenir de ce qui est déjà affiché.
#[derive(Debug, Clone)]
pub struct AirSurface {
    /// La dimension de la grille.
    size: AirSize,
    /// La grille que le code vient de composer.
    composed: Vec<AirCell>,
    /// La grille actuellement à l'écran.
    presented: Vec<AirCell>,
}

impl AirSurface {
    /// Construit une surface de la dimension donnée, remplie de cellules par défaut.
    ///
    /// La grille affichée est initialisée différemment de la grille composée, afin que le
    /// premier différentiel contienne la totalité de l'écran : sans cela, le premier
    /// rendu n'enverrait rien et l'écran resterait vide.
    ///
    /// # Panics
    ///
    /// Si le produit des dimensions dépasse la capacité d'un `usize`, ce qui suppose une
    /// grille irréaliste et non allouable.
    #[must_use]
    pub fn new(size: AirSize) -> Self {
        let count = size
            .cell_count()
            .unwrap_or_else(|| panic!("dimension de surface non allouable : {size:?}"));
        let composed = vec![AirCell::default(); count];
        // Une cellule impossible à composer par le dessin, pour forcer un premier rendu
        // complet.
        let never_composed = AirCell::new('\u{0}', AirColor::BLACK, AirColor::BLACK);
        let presented = vec![never_composed; count];
        Self {
            size,
            composed,
            presented,
        }
    }

    /// La dimension de la surface.
    #[must_use]
    pub const fn size(&self) -> AirSize {
        self.size
    }

    /// Redimensionne la surface, en repartant d'une grille vierge.
    ///
    /// Le contenu est perdu : après un redimensionnement, le code appelant redessine. Ce
    /// choix évite une translation de contenu dont le résultat serait de toute façon
    /// faux, les mises en page dépendant de la taille.
    ///
    /// # Panics
    ///
    /// Si le produit des dimensions dépasse la capacité d'un `usize`.
    pub fn resize(&mut self, size: AirSize) {
        *self = Self::new(size);
    }

    /// L'indice interne d'une position, ou `None` si elle est hors de la surface.
    ///
    /// L'arithmétique est contrôlée : un produit qui déborderait rend `None` plutôt que de
    /// produire un indice replié (Principe 2 d'Air).
    fn index_of(&self, point: AirPoint) -> Option<usize> {
        if point.column.value() >= self.size.columns || point.row.value() >= self.size.rows {
            return None;
        }
        let row = usize::from(point.row.value());
        let columns = usize::from(self.size.columns);
        let column = usize::from(point.column.value());
        row.checked_mul(columns)?.checked_add(column)
    }

    /// Lit une cellule, ou `None` si la position est hors de la surface.
    #[must_use]
    pub fn cell(&self, point: AirPoint) -> Option<&AirCell> {
        let index = self.index_of(point)?;
        self.composed.get(index)
    }

    /// Écrit une cellule. Rend `false` si la position est hors de la surface.
    ///
    /// Le dépassement n'est pas une erreur : écrire hors de la surface est le cas normal
    /// d'un contenu plus grand que sa fenêtre, et le découpage consiste précisément à
    /// laisser tomber ces écritures.
    pub fn set_cell(&mut self, point: AirPoint, cell: AirCell) -> bool {
        let Some(index) = self.index_of(point) else {
            return false;
        };
        match self.composed.get_mut(index) {
            Some(slot) => {
                *slot = cell;
                true
            }
            None => false,
        }
    }

    /// Remplit un rectangle avec la même cellule, en découpant sur les bords.
    pub fn fill_rectangle(&mut self, rectangle: AirRectangle, cell: AirCell) {
        let last_row = rectangle.bottom_exclusive().min(self.size.rows);
        let last_column = rectangle.right_exclusive().min(self.size.columns);
        let mut row = rectangle.top();
        while row < last_row {
            let mut column = rectangle.left();
            while column < last_column {
                let point = AirPoint::new(AirColumn::new(column), AirRow::new(row));
                self.set_cell(point, cell);
                column = column.saturating_add(1);
            }
            row = row.saturating_add(1);
        }
    }

    /// Écrit une chaîne à partir d'une position, et rend la colonne qui suit le dernier
    /// caractère écrit.
    ///
    /// Un caractère par cellule : la largeur d'affichage réelle — idéogrammes, emoji,
    /// combinants — relève des tables UCD du jalon M3 (ADR-006) et n'est **pas** prise en
    /// compte ici.
    pub fn draw_text(
        &mut self,
        origin: AirPoint,
        text: &str,
        foreground: AirColor,
        background: AirColor,
        attributes: AirAttributes,
    ) -> AirColumn {
        let mut column = origin.column;
        for character in text.chars() {
            let cell = AirCell::new(character, foreground, background).with_attributes(attributes);
            let point = AirPoint::new(column, origin.row);
            if !self.set_cell(point, cell) && column.value() >= self.size.columns {
                // Sorti par la droite : les caractères suivants ne peuvent pas rentrer.
                break;
            }
            column = column.next();
        }
        column
    }

    /// Parcourt les cellules qui ont changé depuis le dernier [`Self::mark_presented`].
    #[must_use]
    pub const fn difference(&self) -> AirDifference<'_> {
        AirDifference {
            surface: self,
            index: 0,
            column: 0,
            row: 0,
        }
    }

    /// Déclare que la grille composée est désormais celle affichée.
    ///
    /// À appeler après avoir envoyé le différentiel au terminal. Oublier cet appel rend
    /// le différentiel suivant identique au précédent.
    pub fn mark_presented(&mut self) {
        self.presented.copy_from_slice(&self.composed);
    }
}

/// Le parcours des cellules modifiées d'une surface.
///
/// La position est maintenue par incréments plutôt que recalculée par division : c'est
/// moins coûteux, et cela évite d'avoir à traiter une division par une largeur nulle.
#[derive(Debug)]
pub struct AirDifference<'surface> {
    /// La surface parcourue.
    surface: &'surface AirSurface,
    /// L'indice linéaire de la prochaine cellule à examiner.
    index: usize,
    /// La colonne correspondant à `index`.
    column: u16,
    /// La ligne correspondant à `index`.
    row: u16,
}

impl<'surface> Iterator for AirDifference<'surface> {
    type Item = (AirPoint, &'surface AirCell);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let composed = self.surface.composed.get(self.index)?;
            let presented = self.surface.presented.get(self.index)?;
            let point = AirPoint::new(AirColumn::new(self.column), AirRow::new(self.row));

            self.index = self.index.checked_add(1)?;
            self.column = self.column.saturating_add(1);
            if self.column >= self.surface.size.columns {
                self.column = 0;
                self.row = self.row.saturating_add(1);
            }

            if composed != presented {
                return Some((point, composed));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AirSurface;
    use crate::cell::{AirAttributes, AirCell, AirColor};
    use crate::geometry::{AirColumn, AirPoint, AirRectangle, AirRow, AirSize};

    fn point(column: u16, row: u16) -> AirPoint {
        AirPoint::new(AirColumn::new(column), AirRow::new(row))
    }

    #[test]
    fn le_premier_differentiel_couvre_toute_la_surface() {
        let surface = AirSurface::new(AirSize::new(4, 3));
        assert_eq!(surface.difference().count(), 12);
    }

    #[test]
    fn apres_mark_presented_le_differentiel_est_vide() {
        let mut surface = AirSurface::new(AirSize::new(4, 3));
        surface.mark_presented();
        assert_eq!(surface.difference().count(), 0);
    }

    #[test]
    fn une_ecriture_produit_exactement_une_cellule_modifiee() {
        let mut surface = AirSurface::new(AirSize::new(4, 3));
        surface.mark_presented();
        assert!(surface.set_cell(
            point(1, 2),
            AirCell::new('X', AirColor::WHITE, AirColor::BLUE)
        ));

        let changed: Vec<_> = surface.difference().collect();
        assert_eq!(changed.len(), 1);
        let (changed_point, changed_cell) = *changed.first().expect("une cellule modifiée");
        assert_eq!(changed_point, point(1, 2));
        assert_eq!(changed_cell.character, 'X');
    }

    #[test]
    fn ecrire_hors_surface_est_refuse_sans_paniquer() {
        let mut surface = AirSurface::new(AirSize::new(4, 3));
        assert!(!surface.set_cell(point(4, 0), AirCell::default()));
        assert!(!surface.set_cell(point(0, 3), AirCell::default()));
        assert!(surface.cell(point(4, 0)).is_none());
    }

    #[test]
    fn fill_rectangle_decoupe_sur_les_bords() {
        let mut surface = AirSurface::new(AirSize::new(4, 3));
        surface.mark_presented();
        let cell = AirCell::new('#', AirColor::WHITE, AirColor::BLUE);
        surface.fill_rectangle(AirRectangle::from_parts(2, 1, 10, 10), cell);

        // Colonnes 2 et 3, lignes 1 et 2 : quatre cellules, le reste est hors surface.
        assert_eq!(surface.difference().count(), 4);
        assert_eq!(
            surface.cell(point(3, 2)).map(|found| found.character),
            Some('#')
        );
    }

    #[test]
    fn draw_text_rend_la_colonne_suivante() {
        let mut surface = AirSurface::new(AirSize::new(10, 2));
        let next = surface.draw_text(
            point(1, 0),
            "abc",
            AirColor::WHITE,
            AirColor::BLUE,
            AirAttributes::NONE,
        );
        assert_eq!(next, AirColumn::new(4));
        assert_eq!(
            surface.cell(point(2, 0)).map(|found| found.character),
            Some('b')
        );
    }

    #[test]
    fn draw_text_s_arrete_au_bord_droit() {
        let mut surface = AirSurface::new(AirSize::new(3, 1));
        surface.draw_text(
            point(1, 0),
            "abcdef",
            AirColor::WHITE,
            AirColor::BLUE,
            AirAttributes::NONE,
        );
        assert_eq!(
            surface.cell(point(2, 0)).map(|found| found.character),
            Some('b')
        );
    }

    #[test]
    fn un_redimensionnement_repart_d_une_grille_vierge() {
        let mut surface = AirSurface::new(AirSize::new(4, 3));
        surface.mark_presented();
        surface.resize(AirSize::new(2, 2));
        assert_eq!(surface.size(), AirSize::new(2, 2));
        assert_eq!(surface.difference().count(), 4);
    }
}
