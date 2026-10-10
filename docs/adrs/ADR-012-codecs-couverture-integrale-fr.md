# ADR-012 — Les codecs, et leur couverture intégrale

**Statut :** Accepté — 2026-10-10

## Contexte

Un framework d'interface texte passe son temps à convertir des flux : des octets
d'échappement en événements clavier, une frappe en séquence SGR, des octets UTF-8 en
graphèmes, un fichier de thème en palette. C'est là que les défauts coûtent le plus cher,
parce qu'une conversion fausse ne lève pas d'erreur : elle produit un résultat plausible.

## Décision

**Toute conversion d'un flux de données passe par un codec, et tout codec est tenu à
100 % de couverture de code.**

Un codec est un [`Decoder`], un [`Encoder`], ou les deux — définis dans
`air-tui-platform`. Beaucoup n'ont besoin que d'un sens : l'encodeur de séquences SGR n'a
aucune raison de savoir les relire, et exiger un décodeur factice n'ajouterait que du code
non couvert.

### Ce que la forme du contrat garantit

`Decoder::decode` rend le nombre d'octets **consommés**, et trois règles le rendent
composable :

- un préfixe **incomplet** consomme zéro octet et ne produit rien — l'appelant rappellera
  avec davantage de données. C'est indispensable sur un terminal, où une séquence
  d'échappement peut être coupée entre deux lectures ;
- un préfixe **invalide** est une erreur, jamais une devinette. La tolérance est la faille
  (Principe 11 d'Air) ;
- les éléments produits sont poussés dans une fermeture, pas rendus dans un conteneur :
  une frappe ne doit pas provoquer d'allocation (ADR-021 §4 d'Air).

`Encoder::encode` **ajoute** à un tampon et ne le vide jamais : c'est l'appelant qui le
possède et le réutilise d'un rendu à l'autre.

### Pourquoi 100 %, et ce que ça implique sur l'écriture

Un codec est le point où des octets venus de l'extérieur acquièrent un sens. Une branche
non couverte y est une interprétation que personne n'a vérifiée.

La contrainte a un effet concret sur le style, et c'est l'effet voulu : **une étape
partielle écrite en ligne laisse une branche inatteignable**, donc impossible à couvrir.
Un `unwrap_or` sur une conversion qui ne peut pas échouer, un `if let` sur un index
toujours valide — chacun crée un angle mort que la mesure signale sans qu'on puisse le
combler.

La réponse est d'extraire chaque étape partielle dans une fonction **totale**, dont le cas
dégénéré est inatteignable depuis l'appelant mais directement testable. Exemple appliqué
dans `air-tui/src/codec/decimal.rs` : `digit_at(value, power)` rend zéro pour une
puissance nulle, ce que `push_decimal` ne produit jamais, et qu'un test couvre en une
ligne. La couverture cesse d'être une pénitence et redevient la preuve que chaque chemin a
été pensé.

### Fuzzing

Tout **décodeur** reçoit des octets hostiles par définition : il porte une cible
`cargo-fuzz`, conformément au Principe 1 d'Air. Un encodeur, dont les entrées sont des
types du framework, n'en a pas besoin.

## État mesuré au 2026-10-10

| Codec | Régions | Fonctions | Lignes |
|---|---|---|---|
| `air-tui/src/codec/sgr.rs` | 100 % | 100 % | 100 % |
| `air-tui/src/codec/decimal.rs` | 100 % | 100 % | 100 % |

Aucun décodeur n'existe encore, donc aucune cible `cargo-fuzz` n'est écrite. Le premier
sera le décodeur de séquences ANSI d'entrée, au jalon M1.

## Conséquences

- Le reste du projet n'est **pas** à 100 % et ne le prétend pas : 91,2 % de régions sur
  l'ensemble à cette date. La règle des codecs est une exigence locale et vérifiable, pas
  une annonce globale.
- Un gate `check-codec-coverage` est **à écrire** : mesurer par fichier et refuser qu'un
  module de `codec/` descende sous 100 %. Tant qu'il n'existe pas, la vérification est
  manuelle, par `cargo llvm-cov --summary-only`.

## Alternatives écartées

**Un seul trait `Codec` bidirectionnel.** Aurait forcé un sens factice sur la moitié des
codecs — et du code non couvert, ce que cet ADR interdit par ailleurs.

**Un objectif de 90 % sur les codecs.** Le dernier dixième est précisément celui des
chemins d'erreur et des préfixes tronqués, c'est-à-dire ce qu'un flux hostile exerce en
premier.
