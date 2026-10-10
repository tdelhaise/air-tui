// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Le peintre — le dessin en coordonnées de vue, découpé pour de vrai.
//!
//! # Le modèle de coordonnées relatives au propriétaire
//!
//! Une vue dessine à partir de `(0, 0)`, **la sienne**. Elle ne sait pas où elle se trouve
//! sur l'écran, et c'est voulu : un bouton placé dans une fenêtre déplacée n'a aucune
//! raison d'être réécrit. Le peintre traduit, et c'est le seul endroit qui connaît la
//! position absolue.
//!
//! C'est le modèle de Turbo Vision (`TView::origin`), et la raison est la même : sans lui,
//! chaque vue recalculerait son décalage et une erreur d'un sur l'une les décalerait
//! toutes.
//!
//! # Le découpage, et pourquoi il manquait
//!
//! Au jalon M0, `AirSurface::draw_text` découpait sur le bord de la **surface**, pas sur
//! celui d'une fenêtre : un texte trop long écrasait le cadre qui l'entourait. Le peintre
//! corrige cela à la racine — toute écriture hors du rectangle de découpage est
//! **abandonnée**, et un peintre d'enfant intersecte son découpage avec celui de son
//! parent. Une vue ne peut pas déborder de son propriétaire, même en essayant.

use crate::cell::{AirAttributes, AirCell, AirColor};
use crate::geometry::{AirColumn, AirPoint, AirRectangle, AirRow, AirSize};
use crate::surface::AirSurface;
use crate::unicode::{grapheme_width, graphemes};

/// Le droit de dessiner dans une zone, aux coordonnées de celle-ci.
#[derive(Debug)]
pub struct AirPainter<'surface> {
    /// La grille où tout finit par s'écrire.
    surface: &'surface mut AirSurface,
    /// Les limites de la zone, en coordonnées **absolues**.
    ///
    /// Leur dimension est ce que la vue croit avoir ; leur origine, ce que le peintre
    /// seul connaît.
    bounds: AirRectangle,
    /// Le rectangle hors duquel aucune écriture ne passe, en coordonnées absolues.
    ///
    /// C'est l'intersection des limites de la vue avec celles de tous ses ancêtres.
    clip: AirRectangle,
}

impl<'surface> AirPainter<'surface> {
    /// Un peintre pour la totalité d'une surface.
    pub fn new(surface: &'surface mut AirSurface) -> Self {
        let size = surface.size();
        let whole = AirRectangle::new(AirPoint::ORIGIN, size);
        Self {
            surface,
            bounds: whole,
            clip: whole,
        }
    }

    /// La dimension de la zone, telle que la vue la voit.
    ///
    /// C'est la dimension des **limites**, pas celle du découpage : une vue à moitié
    /// sortie de son propriétaire doit continuer de dessiner son cadre aux bons endroits,
    /// même si la moitié n'arrive pas à l'écran.
    #[must_use]
    pub const fn size(&self) -> AirSize {
        self.bounds.size
    }

    /// La position absolue d'une position de vue, si elle passe le découpage.
    fn absolute(&self, point: AirPoint) -> Option<AirPoint> {
        if point.column.value() >= self.bounds.size.columns
            || point.row.value() >= self.bounds.size.rows
        {
            return None;
        }
        let absolute = AirPoint::new(
            AirColumn::new(self.bounds.left().saturating_add(point.column.value())),
            AirRow::new(self.bounds.top().saturating_add(point.row.value())),
        );
        if self.clip.contains(absolute) {
            Some(absolute)
        } else {
            None
        }
    }

    /// Écrit une cellule. Rend `false` quand la position est hors de la vue ou du
    /// découpage.
    ///
    /// Le refus n'est pas une erreur : c'est le découpage qui fonctionne.
    pub fn set_cell(&mut self, point: AirPoint, cell: AirCell) -> bool {
        match self.absolute(point) {
            Some(absolute) => self.surface.set_cell(absolute, cell),
            None => false,
        }
    }

    /// Lit une cellule de la vue, si elle est visible.
    #[must_use]
    pub fn cell(&self, point: AirPoint) -> Option<&AirCell> {
        self.surface.cell(self.absolute(point)?)
    }

    /// Remplit un rectangle de la vue avec la même cellule.
    pub fn fill_rectangle(&mut self, rectangle: AirRectangle, cell: AirCell) {
        let last_row = rectangle.bottom_exclusive().min(self.bounds.size.rows);
        let last_column = rectangle.right_exclusive().min(self.bounds.size.columns);
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

    /// Écrit une chaîne et rend la colonne qui suit le dernier caractère écrit.
    ///
    /// Le texte est parcouru par **grappes de graphème** et les largeurs d'affichage sont
    /// respectées (ADR-006) : une grappe large occupe deux colonnes dont la seconde reçoit
    /// une cellule de continuation, et une grappe de largeur nulle n'avance pas la
    /// colonne.
    ///
    /// Une grappe large qui ne tient pas entière n'est **pas** écrite : en afficher la
    /// moitié produirait un glyphe tronqué à la place d'un vide propre.
    pub fn draw_text(
        &mut self,
        origin: AirPoint,
        text: &str,
        foreground: AirColor,
        background: AirColor,
        attributes: AirAttributes,
    ) -> AirColumn {
        let mut column = origin.column;
        for cluster in graphemes(text) {
            let width = grapheme_width(cluster);
            if width == 0 {
                continue;
            }
            let advance = u16::try_from(width).unwrap_or(u16::MAX);
            let last_needed = column.value().saturating_add(advance.saturating_sub(1));
            if last_needed >= self.bounds.size.columns {
                break;
            }
            let character = cluster.chars().next().unwrap_or(' ');
            let cell = AirCell::new(character, foreground, background).with_attributes(attributes);
            // Le découpage peut refuser l'écriture sans que la grappe sorte de la vue :
            // on avance quand même, sinon le texte se tasserait à la frontière.
            self.set_cell(AirPoint::new(column, origin.row), cell);
            column = column.next();
            if advance == 2 {
                self.set_cell(
                    AirPoint::new(column, origin.row),
                    AirCell::continuation_of(cell),
                );
                column = column.next();
            }
        }
        column
    }

    /// Un peintre pour une zone intérieure, aux coordonnées de celle-ci.
    ///
    /// `bounds` est exprimé dans les coordonnées de **cette** vue. Rend `None` quand la
    /// zone est entièrement hors du découpage : l'appelant n'a alors rien à dessiner, et
    /// le savoir évite de parcourir un arbre invisible.
    #[must_use]
    pub fn child(&mut self, bounds: AirRectangle) -> Option<AirPainter<'_>> {
        let absolute = bounds.translated(self.bounds.origin);
        let clip = self.clip.intersection(absolute)?;
        Some(AirPainter {
            surface: self.surface,
            bounds: absolute,
            clip,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::AirPainter;
    use crate::cell::{AirAttributes, AirCell, AirColor};
    use crate::geometry::{AirColumn, AirPoint, AirRectangle, AirRow, AirSize};
    use crate::surface::AirSurface;

    fn point(column: u16, row: u16) -> AirPoint {
        AirPoint::new(AirColumn::new(column), AirRow::new(row))
    }

    fn marker() -> AirCell {
        AirCell::new('#', AirColor::WHITE, AirColor::BLUE)
    }

    #[test]
    fn un_peintre_de_surface_couvre_toute_la_grille() {
        let mut surface = AirSurface::new(AirSize::new(8, 4));
        let painter = AirPainter::new(&mut surface);
        assert_eq!(painter.size(), AirSize::new(8, 4));
    }

    #[test]
    fn une_ecriture_est_traduite_vers_la_position_absolue() {
        let mut surface = AirSurface::new(AirSize::new(10, 10));
        {
            let mut painter = AirPainter::new(&mut surface);
            let mut inner = painter
                .child(AirRectangle::from_parts(3, 2, 4, 4))
                .expect("la zone est dans la surface");
            // La vue croit écrire en (0, 0) ; c'est (3, 2) qui reçoit la cellule.
            assert!(inner.set_cell(point(0, 0), marker()));
        }
        assert_eq!(
            surface.cell(point(3, 2)).map(|found| found.character),
            Some('#')
        );
        assert_eq!(
            surface.cell(point(0, 0)).map(|found| found.character),
            Some(' ')
        );
    }

    #[test]
    fn une_ecriture_hors_des_limites_de_la_vue_est_refusee() {
        let mut surface = AirSurface::new(AirSize::new(10, 10));
        let mut painter = AirPainter::new(&mut surface);
        let mut inner = painter
            .child(AirRectangle::from_parts(0, 0, 3, 3))
            .expect("zone valide");
        assert!(!inner.set_cell(point(3, 0), marker()));
        assert!(!inner.set_cell(point(0, 3), marker()));
    }

    #[test]
    fn une_vue_ne_peut_pas_deborder_de_son_proprietaire() {
        // C'est la correction du défaut du jalon M0 : au niveau de la surface seule, cette
        // écriture passait et écrasait ce qui entourait la fenêtre.
        let mut surface = AirSurface::new(AirSize::new(20, 5));
        let border = AirCell::new('|', AirColor::WHITE, AirColor::BLUE);
        assert!(surface.set_cell(point(8, 1), border));
        {
            let mut painter = AirPainter::new(&mut surface);
            let mut window = painter
                .child(AirRectangle::from_parts(2, 0, 6, 3))
                .expect("zone valide");
            // La vue tente d'écrire bien au-delà de ses six colonnes.
            window.draw_text(
                point(0, 1),
                "AAAAAAAAAAAAAA",
                AirColor::WHITE,
                AirColor::BLUE,
                AirAttributes::NONE,
            );
        }
        assert_eq!(
            surface.cell(point(8, 1)).map(|found| found.character),
            Some('|')
        );
    }

    #[test]
    fn un_enfant_herite_du_decoupage_de_son_parent() {
        let mut surface = AirSurface::new(AirSize::new(20, 20));
        {
            let mut painter = AirPainter::new(&mut surface);
            let mut parent = painter
                .child(AirRectangle::from_parts(0, 0, 4, 4))
                .expect("zone valide");
            // L'enfant se croit large de dix colonnes, mais le parent n'en a que quatre.
            let mut child = parent
                .child(AirRectangle::from_parts(0, 0, 10, 10))
                .expect("zone recouvrante");
            assert!(child.set_cell(point(3, 3), marker()));
            assert!(!child.set_cell(point(4, 0), marker()));
        }
        assert_eq!(
            surface.cell(point(3, 3)).map(|found| found.character),
            Some('#')
        );
        assert_eq!(
            surface.cell(point(4, 0)).map(|found| found.character),
            Some(' ')
        );
    }

    #[test]
    fn une_zone_entierement_hors_du_decoupage_ne_donne_pas_de_peintre() {
        // Le savoir évite de parcourir un arbre invisible.
        let mut surface = AirSurface::new(AirSize::new(10, 10));
        let mut painter = AirPainter::new(&mut surface);
        let mut inner = painter
            .child(AirRectangle::from_parts(0, 0, 3, 3))
            .expect("zone valide");
        assert!(inner.child(AirRectangle::from_parts(5, 5, 2, 2)).is_none());
    }

    #[test]
    fn une_zone_hors_de_la_surface_ne_donne_pas_de_peintre() {
        let mut surface = AirSurface::new(AirSize::new(4, 4));
        let mut painter = AirPainter::new(&mut surface);
        assert!(
            painter
                .child(AirRectangle::from_parts(10, 10, 2, 2))
                .is_none()
        );
    }

    #[test]
    fn la_dimension_d_une_vue_reste_la_sienne_meme_tronquee() {
        // Une vue à moitié sortie doit continuer de dessiner son cadre aux bons endroits.
        let mut surface = AirSurface::new(AirSize::new(6, 6));
        let mut painter = AirPainter::new(&mut surface);
        let inner = painter
            .child(AirRectangle::from_parts(4, 4, 8, 8))
            .expect("zone partiellement recouvrante");
        assert_eq!(inner.size(), AirSize::new(8, 8));
    }

    #[test]
    fn le_remplissage_respecte_les_limites_de_la_vue() {
        let mut surface = AirSurface::new(AirSize::new(10, 4));
        {
            let mut painter = AirPainter::new(&mut surface);
            let mut inner = painter
                .child(AirRectangle::from_parts(2, 1, 3, 2))
                .expect("zone valide");
            inner.fill_rectangle(AirRectangle::from_parts(0, 0, 100, 100), marker());
        }
        assert_eq!(
            surface.cell(point(4, 2)).map(|found| found.character),
            Some('#')
        );
        assert_eq!(
            surface.cell(point(5, 2)).map(|found| found.character),
            Some(' ')
        );
        assert_eq!(
            surface.cell(point(1, 1)).map(|found| found.character),
            Some(' ')
        );
    }

    #[test]
    fn la_lecture_d_une_cellule_passe_par_les_memes_coordonnees() {
        let mut surface = AirSurface::new(AirSize::new(10, 10));
        let mut painter = AirPainter::new(&mut surface);
        let mut inner = painter
            .child(AirRectangle::from_parts(3, 3, 4, 4))
            .expect("zone valide");
        assert!(inner.set_cell(point(1, 1), marker()));
        assert_eq!(
            inner.cell(point(1, 1)).map(|found| found.character),
            Some('#')
        );
        assert!(inner.cell(point(9, 9)).is_none());
    }

    #[test]
    fn un_texte_large_est_decoupe_sur_la_vue_et_non_sur_la_surface() {
        let mut surface = AirSurface::new(AirSize::new(20, 2));
        {
            let mut painter = AirPainter::new(&mut surface);
            let mut inner = painter
                .child(AirRectangle::from_parts(0, 0, 3, 1))
                .expect("zone valide");
            // Trois colonnes : un idéogramme tient, le second non.
            inner.draw_text(
                point(0, 0),
                "漢字",
                AirColor::WHITE,
                AirColor::BLUE,
                AirAttributes::NONE,
            );
        }
        assert_eq!(
            surface.cell(point(0, 0)).map(|found| found.character),
            Some('漢')
        );
        assert_eq!(
            surface.cell(point(2, 0)).map(|found| found.character),
            Some(' ')
        );
    }
}
