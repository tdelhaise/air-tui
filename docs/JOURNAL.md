# Journal de bord d'air-tui

Archive chronologique **append-only**, en ordre **anté-chronologique** : la plus récente en
tête. On ajoute, on ne réécrit jamais une entrée passée.

Une entrée par campagne, pas par commit. Répartition des rôles : `etat-avancement.md` est
l'état vivant, ce journal est la trace figée, `INDEX.md` est la carte structurelle.

## 2026-10-10 — publication et protection de `main`

- Dépôt publié : <https://github.com/tdelhaise/air-tui>, public, MPL-2.0.
- Identité du projet : les commits portent `thierry.delhaise@air-desktop.org`, signés par
  la clef `5C6F8A2E20E58061`, déclarée sur le compte GitHub. Les quatre commits d'amorçage
  sont vérifiés et rattachés au compte.
- Protection de `main` activée et **éprouvée par un push réel**, refusé avec
  `GH006 — Changes must be made through a pull request` : pull request obligatoire,
  applicable aux administrateurs, ni force-push ni suppression de branche, historique
  linéaire, résolution des fils requise.
- Deux protections volontairement **absentes**, et pour des raisons distinctes :
  les contrôles de CI requis, parce qu'aucune CI n'existe — en exiger un qui n'existe pas
  bloquerait toute fusion ; et `required_signatures`, qu'Air traite comme une décision du
  BDFL hors du pouvoir d'un agent.

## 2026-10-10 — amorçage du dépôt

- Création du dépôt et reprise des conventions du projet Air (`~/Code/air`), à une
  exception documentée près : la clause Linux-only d'ADR-004 d'Air est remplacée par
  ADR-001 d'air-tui (Linux **et** macOS en tier-1).
- Règle de nommage posée : aucun identifiant abrégé, glossaire contraignant et quatre
  exceptions nommées (`docs/CONVENTIONS-fr.md`).
- ADR-001 à ADR-012 rédigés en français ; traduction anglaise à produire. Quatre principes
  ajoutés en cours de session : versionnement semver par crate incrémenté à chaque PR
  (ADR-009), bordure par défaut sur les vues retirable par `border = false` (ADR-010),
  couture de plateforme par traits en vue du portage sur `linux-air` (ADR-011), et toute
  conversion de flux par un codec tenu à 100 % de couverture (ADR-012).
- Espace de travail recomposé en trois crates : `air-tui-platform` porte le contrat,
  `air-tui-platform-posix` l'implémentation POSIX et tout le `unsafe`, `air-tui` le cœur
  qui ne dépend que du contrat. Effet mesuré : la couverture du `terminal.rs` du cœur passe
  de 0 % à 93,8 %, le rendu devenant testable sans terminal.
- Socle M0 : `air-tui-sys` (six symboles C, enveloppes typées) et `air-tui` (géométrie,
  cellule, surface différentielle, terminal en RAII, thème minimal, exemple `frame`).
- Socle construit et exécuté sur macOS `x86_64` uniquement — Linux reste à vérifier.
  94 tests au vert, clippy propre avec `-D warnings`, 91,2 % de couverture de régions dont
  100 % sur les deux codecs, exemple `frame` vérifié dans un pty.
- Le contrôle du champ `revents` de `poll` a révélé au premier lancement une divergence
  réelle entre macOS et Linux sur `/dev/null` (`POLLNVAL` contre `POLLIN`), désormais
  couverte par des tests sous `cfg`.
