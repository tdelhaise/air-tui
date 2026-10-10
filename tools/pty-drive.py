#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.

"""Lance un binaire dans un pseudo-terminal, lui envoie des frappes, rend sa sortie.

Pourquoi cet outil existe
-------------------------
Une application air-tui a besoin d'un terminal. La vérifier sans en avoir un ne
vérifie que la compilation, et c'est ainsi qu'un défaut bloquant sur macOS a
survécu jusqu'au jalon M5b : `poll` sur un descripteur ouvert sur `/dev/tty` y
rend `POLLNVAL`, et toute application sortait aussitôt.

`script(1)` ne suffit pas : util-linux et la version BSD ne transmettent pas un
stdin redirigé de la même façon, et le même essai ne prouve donc pas la même
chose sur les deux plateformes. Cet outil fait le travail lui-même, à l'identique.

Pourquoi en Python
------------------
Il faut `forkpty`, que la bibliothèque standard de Rust n'offre pas : l'écrire en
Rust demanderait un appel C de plus, et il n'a rien à faire dans la couche 0 du
framework — c'est de l'outillage de test, pas du produit. Le porter dans `xtask`
avec sa propre frontière `unsafe` reste possible et n'est pas fait.

Usage
-----
    tools/pty-drive.py BINAIRE [DELAI_MS OCTETS_HEX]...

Chaque paire écrit des octets dans le terminal, puis lit la sortie pendant le
délai donné. La sortie collectée va sur la sortie standard, le verdict sur la
sortie d'erreur.

    # Alt+F ouvre un menu, F10 quitte
    tools/pty-drive.py ./target/debug/examples/frame 800 1b66 500 1b5b32317e

Verdicts : `exit:<code>` quand le programme est sorti, `timeout` quand il était
encore vivant au bout de dix secondes. Les deux sont des résultats utiles : un
essai **sans** la touche de sortie doit donner `timeout`, sans quoi le programme
sort pour une autre raison et l'essai avec la touche ne prouve rien.
"""

import os
import pty
import select
import sys
import time

#: Durée de lecture avant la première frappe, le temps que la première image sorte.
SETTLE_SECONDS = 0.8
#: Au delà, le programme est considéré comme encore vivant.
LIFETIME_SECONDS = 10.0
#: Granularité de l'attente sur le descripteur maître.
POLL_SECONDS = 0.05


def main(argv: list[str]) -> int:
    if len(argv) < 2 or len(argv) % 2 != 0:
        print(__doc__, file=sys.stderr)
        return 2
    binary = argv[1]
    steps = argv[2:]

    process, master = pty.fork()
    if process == 0:
        os.environ["TERM"] = "xterm-256color"
        os.execv(binary, [binary])
        return 127  # inatteignable : execv ne rend pas la main

    collected = bytearray()
    alive = True

    def drain(duration: float) -> None:
        nonlocal alive
        end = time.time() + duration
        while time.time() < end:
            readable, _, _ = select.select([master], [], [], POLL_SECONDS)
            if not readable:
                continue
            try:
                chunk = os.read(master, 65536)
            except OSError:
                # L'esclave est fermé : le programme est parti.
                alive = False
                return
            if not chunk:
                alive = False
                return
            collected.extend(chunk)

    drain(SETTLE_SECONDS)
    for index in range(0, len(steps), 2):
        if not alive:
            break
        try:
            os.write(master, bytes.fromhex(steps[index + 1]))
        except OSError:
            alive = False
            break
        drain(int(steps[index]) / 1000)

    deadline = time.time() + LIFETIME_SECONDS
    status = None
    while time.time() < deadline:
        finished, code = os.waitpid(process, os.WNOHANG)
        if finished:
            status = code
            break
        drain(0.2)

    if status is None:
        os.kill(process, 9)
        os.waitpid(process, 0)
        print("VERDICT=timeout", file=sys.stderr)
    else:
        print(f"VERDICT=exit:{os.waitstatus_to_exitcode(status)}", file=sys.stderr)
    print(f"BYTES={len(collected)}", file=sys.stderr)
    sys.stdout.buffer.write(bytes(collected))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
