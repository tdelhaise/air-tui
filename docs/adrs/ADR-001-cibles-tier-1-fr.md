# ADR-001 — Cibles tier-1 : Linux et macOS

**Statut :** Accepté — 2026-10-10

## Contexte

air-tui reprend les conventions de code du projet Air. Or ADR-004 d'Air pose « Linux
EXCLUSIVEMENT, tier-1 » et interdit explicitement tout repli macOS dans le code. Cette
règle est cohérente pour Air, dont les choix structurants — io_uring, Landlock,
seccomp-bpf, eBPF, cgroups, systemd — n'ont pas d'équivalent ailleurs.

air-tui n'a aucune de ces dépendances. Son interface avec le système se réduit à six
symboles POSIX présents sur les deux plateformes, et son cahier des charges vise les deux.

## Décision

**Linux et macOS sont tous deux tier-1.** ADR-004 d'Air n'est pas reprise.

| Plateforme | Plancher | Architectures |
|---|---|---|
| Linux | noyau **6.1** | `x86_64`, `aarch64` |
| macOS | **Sequoia 15** | `x86_64`, `aarch64` |

Le plancher Linux est fixé à 6.1 et non à 7.0 : les ABI employées — `termios`,
`TIOCGWINSZ`, `TIOCLINUX`, evdev, le socket gpm — sont stables depuis plus de vingt ans.
Un plancher 7.x n'apporterait rien techniquement et écarterait tout le parc LTS courant
(Debian 13 en 6.12, Ubuntu 24.04 LTS en 6.8, RHEL 9 en 5.14). Linux 7.x est testé, pas
exigé.

Les deux architectures sont visées dès le premier jour, comme dans Air (Principe 9) : une
particularité d'ABI qui n'apparaît que sur arm64 — le passage des arguments variadiques par
la pile sur l'ABI d'Apple — est précisément le genre de défaut qu'on ne découvre pas en
différant le portage.

## Conséquences

- La clause Linux-only est **retirée** du `CLAUDE.md` copié d'Air, pas laissée à
  l'interprétation. La laisser installerait dans ce dépôt une règle fausse pour ce projet,
  exactement ce que le NO GO n° 1 existe pour empêcher.
- Aucun `#[cfg(target_os)]` ne doit apparaître au-dessus de `air-tui-sys`. La divergence
  entre plateformes est absorbée par la couche 0 ; `air-tui` ne la voit pas.
- macOS n'a **pas** de console texte : pas de VT, pas de `/dev/input`. La cible macOS est
  un pty dans un terminal, jamais une console. Conséquence traitée par ADR-003.

## Alternatives écartées

**Reprendre le Linux-only d'Air.** Contredit le cahier des charges ; aurait rendu la moitié
du projet irréalisable par règle plutôt que par technique.

**Faire de macOS un tier-2.** Rien ne le justifie : la surface système est la même à six
symboles près, et un tier-2 non testé en continu dérive en quelques semaines.
