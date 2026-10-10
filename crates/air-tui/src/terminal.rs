// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Le terminal — ouverture, mode brut en RAII, rendu différentiel.
//!
//! Le terminal est ouvert sur `/dev/tty` et non sur l'entrée standard : l'application
//! garde ainsi la possibilité de voir ses flux redirigés par le shell sans que son
//! interface en souffre.
//!
//! Le mode brut est restauré par [`Drop`]. C'est la seule garantie qui tienne : un
//! `return` oublié sur un chemin d'erreur laisserait sinon le terminal inutilisable après
//! la sortie du programme.

use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::{AsFd, BorrowedFd};
use std::time::Duration;

use air_tui_sys::{
    Errno, TerminalMode, enter_raw_mode, is_terminal, restore_mode, terminal_window_size,
    wait_for_input,
};

/// Ce qu'a conclu une attente d'entrée — ré-exporté depuis la couche 0 pour que l'appelant
/// n'ait pas à dépendre de `air-tui-sys`.
pub use air_tui_sys::InputReadiness;

use crate::cell::{AirAttributes, AirCell, AirColor};
use crate::geometry::AirSize;
use crate::surface::AirSurface;

/// La dimension retenue quand le système annonce une taille inconnue.
///
/// Quatre-vingts colonnes sur vingt-cinq lignes : la taille d'un écran texte VGA, et le
/// repli qu'emploient les terminaux depuis quarante ans.
const FALLBACK_SIZE: AirSize = AirSize::new(80, 25);

/// Un terminal ouvert, en mode brut, dont l'état d'origine sera restauré.
#[derive(Debug)]
pub struct AirTerminal {
    /// Le terminal ouvert sur `/dev/tty`.
    teletype: File,
    /// Le mode à restaurer en sortie.
    original_mode: TerminalMode,
    /// Le tampon de sortie, alloué une fois et réutilisé à chaque rendu.
    output: Vec<u8>,
    /// Les attributs déjà écrits, pour n'émettre une séquence SGR que quand ils changent.
    last_written_attributes: Option<(AirColor, AirColor, AirAttributes)>,
}

impl AirTerminal {
    /// Ouvre `/dev/tty`, passe en mode brut, entre dans l'écran alterné et masque le
    /// curseur.
    ///
    /// # Errors
    ///
    /// Si `/dev/tty` n'est pas ouvrable, s'il ne désigne pas un terminal, ou si le
    /// passage en mode brut échoue.
    pub fn open() -> io::Result<Self> {
        let teletype = File::options().read(true).write(true).open("/dev/tty")?;
        if !is_terminal(teletype.as_fd()) {
            return Err(io::Error::from(Errno::NOT_A_TERMINAL));
        }
        let original_mode = enter_raw_mode(teletype.as_fd())?;

        let mut terminal = Self {
            teletype,
            original_mode,
            // Assez pour un écran complet sans réallouer : chaque cellule modifiée coûte
            // une poignée d'octets de positionnement et d'attributs.
            output: Vec::with_capacity(64 * 1024),
            last_written_attributes: None,
        };
        terminal.write_sequence("\x1b[?1049h\x1b[?25l")?;
        Ok(terminal)
    }

    /// Le descripteur du terminal, pour les appels de la couche 0.
    fn file_descriptor(&self) -> BorrowedFd<'_> {
        self.teletype.as_fd()
    }

    /// La taille courante du terminal.
    ///
    /// Interrogée à la demande, à chaque appel : c'est ce qui rend inutile la gestion du
    /// signal `SIGWINCH`, et ce qui garantit qu'aucune valeur périmée ne traîne après un
    /// redimensionnement.
    ///
    /// # Errors
    ///
    /// Si l'interrogation du terminal échoue.
    pub fn size(&self) -> io::Result<AirSize> {
        let measured = terminal_window_size(self.file_descriptor())?;
        Ok(match measured {
            Some(size) => AirSize::new(size.columns, size.rows),
            None => FALLBACK_SIZE,
        })
    }

    /// Attend qu'une entrée soit disponible.
    ///
    /// Une expiration est [`InputReadiness::TimedOut`], **pas** une erreur. Et
    /// [`Errno::INTERRUPTED`] est remonté tel quel : c'est à l'appelant de décider s'il
    /// reprend l'attente ou s'il va d'abord relire la taille du terminal.
    ///
    /// # Errors
    ///
    /// Si `poll` échoue, y compris sur interruption par un signal.
    pub fn wait_for_input(&self, timeout: Option<Duration>) -> io::Result<InputReadiness> {
        let readiness = wait_for_input(self.file_descriptor(), timeout)?;
        Ok(readiness)
    }

    /// Lit les octets disponibles dans `buffer` et rend leur nombre.
    ///
    /// Aucune interprétation : le parseur qui transformera ces octets en événements est
    /// l'objet du jalon M1 et n'est pas écrit.
    ///
    /// # Errors
    ///
    /// Si la lecture échoue. Une interruption par signal remonte en
    /// [`io::ErrorKind::Interrupted`], sans retry automatique.
    pub fn read_available(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.teletype.read(buffer)
    }

    /// Envoie au terminal les cellules qui ont changé, puis marque la surface comme
    /// affichée.
    ///
    /// Le différentiel est encadré par les séquences de mise à jour synchronisée : un
    /// terminal qui les comprend n'affiche rien avant la fin, ce qui supprime le
    /// scintillement sur les redessins larges ; un terminal qui ne les comprend pas les
    /// ignore.
    ///
    /// # Errors
    ///
    /// Si l'écriture sur le terminal échoue.
    pub fn present(&mut self, surface: &mut AirSurface) -> io::Result<()> {
        self.output.clear();
        self.last_written_attributes = None;
        self.output.extend_from_slice(b"\x1b[?2026h");

        for (point, cell) in surface.difference() {
            // Les coordonnées ANSI comptent depuis un, la grille depuis zéro.
            let row = point.row.value().saturating_add(1);
            let column = point.column.value().saturating_add(1);
            write!(self.output, "\x1b[{row};{column}H")?;
            self.write_cell_attributes(*cell)?;
            let mut encoded = [0_u8; 4];
            self.output
                .extend_from_slice(cell.character.encode_utf8(&mut encoded).as_bytes());
        }

        self.output.extend_from_slice(b"\x1b[0m\x1b[?2026l");
        self.teletype.write_all(&self.output)?;
        self.teletype.flush()?;
        surface.mark_presented();
        Ok(())
    }

    /// Écrit la séquence SGR d'une cellule, en sautant ce qui est déjà en vigueur.
    fn write_cell_attributes(&mut self, cell: AirCell) -> io::Result<()> {
        let wanted = (cell.foreground, cell.background, cell.attributes);
        if self.last_written_attributes == Some(wanted) {
            return Ok(());
        }
        self.output.extend_from_slice(b"\x1b[0");
        if cell.attributes.bold {
            self.output.extend_from_slice(b";1");
        }
        if cell.attributes.dim {
            self.output.extend_from_slice(b";2");
        }
        if cell.attributes.italic {
            self.output.extend_from_slice(b";3");
        }
        if cell.attributes.underline {
            self.output.extend_from_slice(b";4");
        }
        if cell.attributes.reverse {
            self.output.extend_from_slice(b";7");
        }
        if cell.attributes.strikethrough {
            self.output.extend_from_slice(b";9");
        }
        self.write_color(cell.foreground, true)?;
        self.write_color(cell.background, false)?;
        self.output.extend_from_slice(b"m");
        self.last_written_attributes = Some(wanted);
        Ok(())
    }

    /// Écrit un paramètre SGR de couleur.
    fn write_color(&mut self, color: AirColor, is_foreground: bool) -> io::Result<()> {
        let selector = if is_foreground { 38 } else { 48 };
        match color {
            AirColor::Indexed(index) => write!(self.output, ";{selector};5;{index}"),
            AirColor::Rgb { red, green, blue } => {
                write!(self.output, ";{selector};2;{red};{green};{blue}")
            }
        }
    }

    /// Écrit une séquence de contrôle immédiatement.
    fn write_sequence(&mut self, sequence: &str) -> io::Result<()> {
        self.teletype.write_all(sequence.as_bytes())?;
        self.teletype.flush()
    }
}

impl Drop for AirTerminal {
    fn drop(&mut self) {
        // Aucune erreur n'est remontée : on est en train de partir, et laisser le
        // terminal en mode brut serait pire que d'ignorer un échec d'écriture. Chaque
        // étape est donc tentée indépendamment des précédentes.
        let _ = self.write_sequence("\x1b[0m\x1b[?25h\x1b[?1049l");
        let _ = restore_mode(self.teletype.as_fd(), &self.original_mode);
    }
}
