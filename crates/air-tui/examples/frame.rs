// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Exemple du jalon M0 — la preuve que le socle fonctionne.
//!
//! Dessine un bureau hachuré, une barre de menu, une fenêtre à cadre et case de
//! fermeture, et une barre de statut, aux couleurs et aux glyphes de Turbo Vision. Puis
//! attend une frappe et restaure le terminal.
//!
//! Ce n'est **pas** un widget : l'arbre de vues n'existe pas encore. Tout est dessiné à
//! la main sur la surface, précisément pour montrer ce que le socle sait faire sans rien
//! de plus.
//!
//! ```sh
//! cargo run --example frame
//! ```

use std::io;
use std::time::Duration;

use air_tui::cell::{AirAttributes, AirCell};
use air_tui::geometry::{AirColumn, AirPoint, AirRectangle, AirRow};
use air_tui::surface::AirSurface;
use air_tui::terminal::{AirTerminal, InputReadiness};
use air_tui::theme::{AirColorPair, AirFrameGlyphs, AirRole, AirTheme};

fn main() -> io::Result<()> {
    let mut terminal = AirTerminal::open()?;
    let theme = AirTheme::turbo_vision();
    let size = terminal.size()?;
    let mut surface = AirSurface::new(size);

    draw_desktop(&mut surface, &theme);
    draw_menu_bar(&mut surface, &theme);
    draw_status_line(&mut surface, &theme);

    // Une fenêtre centrée, assez grande pour que le cadre et le titre se lisent.
    let window_columns = size.columns.saturating_sub(20).clamp(20, 54);
    let window_rows = size.rows.saturating_sub(8).clamp(6, 14);
    let window_column = size.columns.saturating_sub(window_columns) / 2;
    let window_row = size.rows.saturating_sub(window_rows) / 2;
    draw_window(
        &mut surface,
        &theme,
        AirRectangle::from_parts(window_column, window_row, window_columns, window_rows),
        " air-tui ",
    );

    terminal.present(&mut surface)?;

    // Attendre une frappe, sans interpréter quoi que ce soit : le parseur d'entrée est
    // l'objet du jalon M1.
    let mut input = [0_u8; 32];
    loop {
        match terminal.wait_for_input(Some(Duration::from_millis(250))) {
            Ok(InputReadiness::Ready) => {
                let read = terminal.read_available(&mut input)?;
                if read > 0 {
                    break;
                }
            }
            // Le terminal a disparu : on s'en va, et sans bruit. C'est le cas quand la
            // session est fermée pendant que l'application tourne.
            Ok(InputReadiness::Closed) => break,
            Ok(InputReadiness::TimedOut) => {}
            // Une interruption par signal n'est pas une raison d'abandonner : on reprend
            // l'attente, explicitement, comme l'exige la convention de la couche 0.
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            // `EBADF` signifie ici que le pty a été révoqué sous nos pieds — constaté en
            // lançant cet exemple sous `script(1)`, qui démonte le pty dès que son entrée
            // se tarit. Ce n'est pas une panne de l'application : elle s'arrête.
            Err(error) if error.raw_os_error() == Some(9) => break,
            Err(error) => return Err(error),
        }
    }

    Ok(())
}

/// Remplit le bureau du motif hachuré de Turbo Vision.
fn draw_desktop(surface: &mut AirSurface, theme: &AirTheme) {
    let colors = theme.palette.color_pair(AirRole::DesktopBackground);
    let fill = AirCell::new(
        theme.glyphs.desktop_fill,
        colors.foreground,
        colors.background,
    );
    let size = surface.size();
    surface.fill_rectangle(
        AirRectangle::from_parts(0, 1, size.columns, size.rows.saturating_sub(2)),
        fill,
    );
}

/// Dessine la barre de menu, avec ses lettres d'accès rapide en rouge.
fn draw_menu_bar(surface: &mut AirSurface, theme: &AirTheme) {
    let colors = theme.palette.color_pair(AirRole::MenuBar);
    let hotkey = theme.palette.color_pair(AirRole::MenuHotkey);
    let size = surface.size();
    surface.fill_rectangle(
        AirRectangle::from_parts(0, 0, size.columns, 1),
        AirCell::new(' ', colors.foreground, colors.background),
    );

    let mut column = AirColumn::new(2);
    for (hotkey_letter, remainder) in [("F", "ile"), ("E", "dit"), ("S", "earch"), ("W", "indows")]
    {
        column = surface.draw_text(
            AirPoint::new(column, AirRow::FIRST),
            hotkey_letter,
            hotkey.foreground,
            hotkey.background,
            AirAttributes::NONE,
        );
        column = surface.draw_text(
            AirPoint::new(column, AirRow::FIRST),
            remainder,
            colors.foreground,
            colors.background,
            AirAttributes::NONE,
        );
        column = AirColumn::new(column.value().saturating_add(2));
    }
}

/// Dessine la barre de statut en bas de l'écran.
fn draw_status_line(surface: &mut AirSurface, theme: &AirTheme) {
    let colors = theme.palette.color_pair(AirRole::StatusLine);
    let hotkey = theme.palette.color_pair(AirRole::StatusHotkey);
    let size = surface.size();
    let row = AirRow::new(size.rows.saturating_sub(1));
    surface.fill_rectangle(
        AirRectangle::from_parts(0, row.value(), size.columns, 1),
        AirCell::new(' ', colors.foreground, colors.background),
    );

    let mut column = surface.draw_text(
        AirPoint::new(AirColumn::new(1), row),
        "Toute touche",
        hotkey.foreground,
        hotkey.background,
        AirAttributes::NONE,
    );
    column = AirColumn::new(column.value().saturating_add(1));
    surface.draw_text(
        AirPoint::new(column, row),
        "Quitter",
        colors.foreground,
        colors.background,
        AirAttributes::NONE,
    );
}

/// Dessine une fenêtre : cadre, case de fermeture, titre, intérieur.
fn draw_window(surface: &mut AirSurface, theme: &AirTheme, bounds: AirRectangle, title: &str) {
    let frame_colors = theme.palette.color_pair(AirRole::WindowFrameActive);
    let interior = theme.palette.color_pair(AirRole::WindowInterior);

    surface.fill_rectangle(
        bounds,
        AirCell::new(' ', interior.foreground, interior.background),
    );
    draw_frame(surface, bounds, theme.glyphs.window_frame, frame_colors);
    draw_close_box(surface, theme, bounds);

    // Titre centré sur le bord supérieur.
    let title_length = u16::try_from(title.chars().count()).unwrap_or(u16::MAX);
    let free = bounds.size.columns.saturating_sub(title_length);
    let title_column = bounds.left().saturating_add(free / 2);
    let title_colors = theme.palette.color_pair(AirRole::WindowTitle);
    surface.draw_text(
        AirPoint::new(AirColumn::new(title_column), AirRow::new(bounds.top())),
        title,
        title_colors.foreground,
        title_colors.background,
        AirAttributes::NONE.with_bold(),
    );

    // Accentué à dessein : le rendu est en UTF-8 de bout en bout.
    //
    // La longueur est choisie pour tenir dans la fenêtre : `draw_text` découpe sur le
    // bord de la **surface**, pas sur celui de la fenêtre. Le découpage par vue vient
    // avec l'arbre de vues du jalon M4.
    let message = "Socle M0 : surface différentielle, mode brut.";
    surface.draw_text(
        AirPoint::new(
            AirColumn::new(bounds.left().saturating_add(2)),
            AirRow::new(bounds.top().saturating_add(2)),
        ),
        message,
        interior.foreground,
        interior.background,
        AirAttributes::NONE,
    );
}

/// Trace les quatre bords et les quatre angles d'un rectangle.
fn draw_frame(
    surface: &mut AirSurface,
    bounds: AirRectangle,
    glyphs: AirFrameGlyphs,
    colors: AirColorPair,
) {
    if bounds.size.columns < 2 || bounds.size.rows < 2 {
        return;
    }
    let left = bounds.left();
    let top = bounds.top();
    let right = bounds.right_exclusive().saturating_sub(1);
    let bottom = bounds.bottom_exclusive().saturating_sub(1);

    let mut put = |column: u16, row: u16, character: char| {
        surface.set_cell(
            AirPoint::new(AirColumn::new(column), AirRow::new(row)),
            AirCell::new(character, colors.foreground, colors.background),
        );
    };

    let mut column = left.saturating_add(1);
    while column < right {
        put(column, top, glyphs.horizontal);
        put(column, bottom, glyphs.horizontal);
        column = column.saturating_add(1);
    }
    let mut row = top.saturating_add(1);
    while row < bottom {
        put(left, row, glyphs.vertical);
        put(right, row, glyphs.vertical);
        row = row.saturating_add(1);
    }
    put(left, top, glyphs.top_left);
    put(right, top, glyphs.top_right);
    put(left, bottom, glyphs.bottom_left);
    put(right, bottom, glyphs.bottom_right);
}

/// Dessine la case de fermeture dans l'angle supérieur gauche du cadre.
fn draw_close_box(surface: &mut AirSurface, theme: &AirTheme, bounds: AirRectangle) {
    if bounds.size.columns < 7 {
        return;
    }
    let frame = theme.palette.color_pair(AirRole::WindowFrameActive);
    let mark = theme.palette.color_pair(AirRole::CloseBoxMark);
    let row = AirRow::new(bounds.top());
    let start = bounds.left().saturating_add(2);

    for (offset, character, colors) in [
        (0_u16, theme.glyphs.close_box_left, frame),
        (1, theme.glyphs.close_box_mark, mark),
        (2, theme.glyphs.close_box_right, frame),
    ] {
        surface.set_cell(
            AirPoint::new(AirColumn::new(start.saturating_add(offset)), row),
            AirCell::new(character, colors.foreground, colors.background),
        );
    }
}
