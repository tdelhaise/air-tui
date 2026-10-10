# ADR-005 — Zéro dépendance

**Statut :** Accepté — 2026-10-10

## Contexte

L'auteur exige qu'air-tui ne dépende d'aucune crate tierce : seule la bibliothèque
standard. Or `std` n'expose ni `termios`, ni `ioctl`, ni `poll`. Trois voies existaient.

## Décision

**Aucune entrée dans `[dependencies]`.** Les appels système manquants sont obtenus par des
déclarations `extern "C"` vers la bibliothèque C **que `std` lie déjà** : glibc sur
`*-linux-gnu`, libSystem sur `*-apple-darwin`. Six symboles : `tcgetattr`, `tcsetattr`,
`cfmakeraw`, `ioctl`, `poll`, `isatty`.

Ce n'est pas un compromis sur la règle, c'est sa seule mise en œuvre possible pour deux
plateformes : Apple ne garantit aucune ABI d'appel système stable et impose le passage par
libSystem. La voie « appels système en `core::arch::asm!` » marche sur Linux et est
**impossible** sur macOS.

Trois besoins sont volontairement couverts par `std` plutôt que par du FFI :
l'ouverture de `/dev/tty` (`std::fs::File`), la lecture et l'écriture
(`std::io::Read`/`Write`), et `errno` (`std::io::Error::last_os_error()`).

## Conséquences

- `cargo tree` affiche un seul nœud par crate. La règle des 80 % d'Air (ADR-024) est sans
  objet.
- `deny.toml` conserve néanmoins les bornes de licences d'Air : si une dépendance devait un
  jour être introduite par RFC, elle partirait des mêmes limites.
- **La surface FFI est documentée comme une dépendance le serait** : elle est listée
  exhaustivement dans `docs/ARCHITECTURE-fr.md`, avec l'emploi de chaque symbole. C'est
  l'équivalent moral de l'audit qu'ADR-024 d'Air exige par dépendance.
- Deux pièges d'ABI sont traités explicitement plutôt que subis — `ioctl` déclaré
  variadique (ABI arm64 d'Apple), et `struct termios` traité en tampon opaque. Détail dans
  `docs/ARCHITECTURE-fr.md`.

## Alternatives écartées

**La crate `libc`.** Une seule dépendance, maintenue par le projet Rust lui-même — mais la
règle posée est « std uniquement », et les six déclarations tiennent en trente lignes.

**Appels système en assembleur.** Linux seulement ; aurait condamné la cible macOS.
