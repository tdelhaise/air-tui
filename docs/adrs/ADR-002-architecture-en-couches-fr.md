# ADR-002 — Architecture en couches et frontière `unsafe`

**Statut :** Accepté — 2026-10-10

## Contexte

`CLAUDE.md` d'Air est catégorique : « `unsafe` : en couche 0, et NULLE PART ailleurs. La
couche 0 est mince et son unique raison d'être est d'isoler les appels `unsafe`. Au-dessus :
Rust pur et idiomatique — pas d'assembleur, pas d'appel à du C, pas d'`unsafe`. »

air-tui a besoin de six appels système que `std` n'expose pas. Il faut donc une couche 0, et
il faut que sa frontière soit effective et non déclarative.

## Décision

Deux crates, et la frontière est le système de crates lui-même :

| Crate | Couche | Contenu |
|---|---|---|
| `air-tui-sys` | 0 | les déclarations `extern "C"`, tout le `unsafe`, les enveloppes typées |
| `air-tui` | 1 | `#![forbid(unsafe_code)]` — tout le reste |

`air-tui` ne *peut pas* appeler de C : son attribut le refuse à la compilation. La règle
cesse d'être une discipline.

La couche 0 respecte les conventions d'ADR-021 d'Air :

1. `Option<T>` au lieu des sentinelles ;
2. EINTR **remonté**, jamais retenté automatiquement — la boucle d'événements de la couche
   1 écrit son retry explicitement, et c'est utile : sur `SIGWINCH` on veut justement ne pas
   retenter mais aller relire la taille ;
3. **une fonction dédiée typée par opération** — pas de `ioctl(fd, request, …)` générique
   exposé ;
4. pas d'allocation sur le happy path ;
5. tout bloc `unsafe` porte un `// SAFETY:`.

## Conséquences

- Aucun `#[cfg(target_os)]` au-dessus de la couche 0.
- Les types de la couche 0 (`Errno`, `WindowSize`, `TerminalMode`) ne portent **pas** le
  préfixe `Air` : le chemin de module suffit, et ils ne font pas partie du modèle d'objets
  de l'interface. Même choix qu'Air avec `air_sys_types::Errno`.
- L'objectif de couverture de 100 % s'applique d'abord à cette crate : elle est petite, et
  c'est elle qui peut corrompre la mémoire.

## Alternatives écartées

**Un seul crate avec un module `sys`.** La frontière redevient déclarative : rien
n'empêche un `unsafe` de migrer vers un module voisin. Air a mesuré le coût de ce régime —
1909 blocs `unsafe` de production, dont 46 sans justification.
