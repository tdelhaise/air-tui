# Journal de bord d'air-tui

Archive chronologique **append-only**, en ordre **anté-chronologique** : la plus récente en
tête. On ajoute, on ne réécrit jamais une entrée passée.

Une entrée par campagne, pas par commit. Répartition des rôles : `etat-avancement.md` est
l'état vivant, ce journal est la trace figée, `INDEX.md` est la carte structurelle.

## 2026-10-10 — Jalon M6b : les trois contrôles de conventions, et la CI qui les exige

- **Trois règles contraignantes cessent de reposer sur la bonne volonté.** `check-naming`
  (ADR-004), `check-versions` (ADR-009) et `check-codec-coverage` (ADR-012) étaient
  annoncées « à écrire » depuis le premier jour, et tenues à la main. Elles sont écrites,
  et l'intégration continue les exécute sur chaque PR. Le choix de les écrire **avant** M7
  n'est pas un détour : l'importeur de skins MC lit un format de fichier, donc c'est un
  codec au sens de l'ADR-012, donc 100 % de couverture — et sans gate, cette conformité
  n'aurait reposé que sur ma parole au moment où je l'écris.
- **Le premier résultat du contrôle de nommage est un non-événement, et c'est une bonne
  nouvelle** : 6866 noms déclarés inspectés dans 64 fichiers, aucune abréviation. La règle
  tenue à la main l'était réellement. Ce qui ne rend pas le gate inutile — il protège les
  quarante prochains fichiers, pas les soixante-quatre premiers.
- **La distinction qui rend le contrôle de nommage possible** : il inspecte ce que le dépôt
  **nomme**, pas ce qu'il **appelle**. `u32::MAX`, `.min(other)` et `.len()` sont des noms
  imposés par `std`, employés et non choisis ; `fn len()` serait un nom choisi ici. Sans
  cette frontière, le glossaire ferait échouer chaque fichier et le gate serait désactivé
  dans la semaine. Son angle mort — la liaison d'un bras de `match` — est écrit dans
  l'en-tête du module, dans l'ADR et dans les conventions : un contrôle dont on ignore la
  portée donne une assurance fausse.
- **Chaque gate est éprouvé sur un refus, pas seulement sur un succès.** Un gate qui dit
  toujours oui est pire qu'un gate absent, parce qu'il interdit de se poser la question. Le
  contrôle de nommage a été lancé contre un fichier fautif posé exprès dans
  `crates/air-tui/src/` — il l'a cité avec sa ligne et sorti en code 1, puis le fichier a
  été retiré. Le contrôle de versions a trouvé sa première vraie infraction sur sa propre
  branche : `xtask` touchée sans incrément, d'où 0.2.0 → 0.2.1.
- **Ce que cargo laisse passer et que le contrôle de versions attrape.** Déclarer
  `version = "0.6.0"` pour une crate en 0.7.0, cargo le refuse tout seul. Déclarer
  `"0.7.0"` pour une crate en 0.7.1, il l'accepte sans un mot — `^0.7.0` est satisfait —
  et c'est la publication qui livrerait une borne fausse, des semaines plus tard. C'est
  cette dérive **compatible** que le gate existe pour voir.
- **La mesure de couverture a tranché un point que je n'avais pas vu venir.** Deux modules
  nommés `codec` ne peuvent pas être soumis à la règle des 100 %, et il fallait le dire
  plutôt que de les glisser dans la liste : `codec/mod.rs` est une façade sans une ligne
  exécutable, que `llvm-cov` ne relève pas du tout — l'exiger reviendrait à exiger une
  mesure qui ne peut pas exister. Et `air-tui-platform/src/codec.rs`, qui porte le contrat,
  est à 84/90 régions : ses six régions manquantes sont les fermetures
  `|item| collected.push(item)` de deux essais où le décodeur ne doit **rien** produire,
  donc où la fermeture ne peut pas être appelée. Exiger 100 % là mesurerait le double
  d'essai et non un codec. Les deux exclusions sont motivées dans le gate, avec la mesure.
- **Un fichier absent du relevé est un manquement, pas un succès.** C'est le cas qui compte
  dans un contrôle de couverture par liste : un fichier renommé sans que la liste suive
  sortirait silencieusement de la règle, et le gate dirait que tout va bien. Il a son
  essai, et c'est lui qui a révélé que `codec/mod.rs` n'était pas mesurable.
- **Zéro dépendance, ici aussi** (ADR-005). Le balayage lexical remplace `syn`, et la
  lecture du relevé `llvm-cov` est un extracteur de trois résumés plutôt qu'un analyseur
  JSON général. Les deux pièges de la lecture par sous-chaîne ont leur essai :
  `rust-version` ne doit pas répondre pour `version`, et `notcovered` ne doit pas répondre
  pour `covered`.
- **La CI dit ce qu'elle ne fait pas.** Pas d'`aarch64` Linux — GitHub n'en offre pas de
  coureur gratuit, donc `neon` reste vérifiée à la main. Pas d'essai en pseudo-terminal —
  un coureur n'a pas de terminal de contrôle, et c'est l'essai qui avait trouvé le
  `POLLNVAL` de macOS. Pas de fuzzing — il exige nightly, et le dépôt est pinné sur stable.
  Ces trois absences sont dans l'en-tête du fichier de workflow, pour qu'on ne lise pas la
  CI comme une garantie qu'elle n'offre pas.
- **Le README corrigé d'une demi-ligne** : il écrivait au présent qu'« un second thème
  reproduit celui de Midnight Commander ». La section *État* démentait trois paragraphes
  plus bas, mais l'affirmation restait écrite au présent. Elle est passée au futur.
- Versions : `xtask` 0.2.0 → **0.2.1** (ajout de commandes, donc `z` — en `0.y.z`, seule
  une rupture avance `y`). 894 essais au vert, dont 49 nouveaux.

## 2026-10-10 — Jalon M6 : `redit`, la couche MVC, et la hiérarchie complète

- **L'ADR-007 a cessé d'être une intention.** La couche modèle existe : `AirModel` à une
  seule méthode, `AirChangeSet` pour les faits structurels, `AirGeneration` pour l'état
  continu, `AirTextBuffer` comme modèle de l'éditeur. Le partage entre le journal et les
  compteurs n'est pas cosmétique — un journal de ce qui est fréquent devient une liste
  que personne ne relit, et un compteur sur ce qui est structurel ne dit pas **quoi** a
  changé. Une frappe ne consigne donc rien ; elle fait avancer deux entiers.
- **La vue d'édition ne tient pas son modèle**, et c'est là que l'ADR se vérifie ou
  s'effondre. `AirEditorView` n'a ni `&`, ni `Rc`, ni pointeur vers le tampon : elle garde
  un reflet que le contrôleur lui pousse, et accumule des intentions qu'il relève. Une
  frappe ne peut pas muter le modèle depuis la vue, et il n'y a jamais deux emprunts
  mutables vivants.
- **Une erreur de conception, trouvée par `redit` avant sa première ligne.** J'avais écrit
  `AirApplication` en lui faisant **posséder** l'arbre, et une pile d'écrans par-dessus.
  C'est intenable : `children` rend des `Box<dyn AirView>`, donc une fois la vue d'édition
  confiée à l'application, plus personne ne pouvait lui demander de se synchroniser. La
  racine est désormais passée en paramètre à `draw` et à `pump`, et la pile appartient à
  l'appelant. C'est exactement l'argument que le module invoquait déjà pour ne pas prendre
  la boucle — appliqué une fois de plus.
- **`AirView` hérite de `Any` (ADR-013).** `find` et `find_mut` rendent une vue avec son
  type, par identifiant. Les deux alternatives coûtent plus cher : un paramètre de type
  sur les conteneurs remonte jusqu'à l'application et interdit de mêler deux types de
  fenêtres dans un bureau ; une méthode par type de vue ferait du cœur le catalogue de ses
  widgets.
- **`AirScreen` existe pour rendre une erreur impossible.** Au jalon M5b, la barre de menu
  devait être insérée en dernier, et la contrainte ne tenait que par la documentation.
  L'écran range ses quatre pièces lui-même — bureau, statut, menu, modal —, dans le seul
  ordre qui marche, quel que soit l'ordre des appels. La **modalité** est son quatrième
  emplacement : un clic hors du modal est consommé plutôt que laissé à ce qui est derrière.
- **Le contrôle de conformité Unicode a payé au premier lancement.** `cargo xtask
  check-segmentation` confronte les deux segmentations aux fichiers d'essais du Consortium.
  La segmentation en **mots**, écrite pour ce jalon, passait ses 1944 cas sans écart. Celle
  en **grappes**, livrée au jalon M3 et annoncée comme faite, en échouait **dix** : la
  révision 49 de l'annexe 29, livrée avec Unicode 18.0.0, a réécrit `GB9c` en
  `\p{InCB=Linker} \p{InCB=Extend}* × \p{InCB=Consonant}` et retiré l'exigence d'une
  consonne **avant** le lieur. Les tables du dépôt venaient déjà de 18.0.0, les règles non
  — elles n'étaient pas de la même édition. Corrigé, et les deux cas d'essai
  correspondants sont dans la suite de tests.

  La leçon est la même qu'au jalon M5b avec le pseudo-terminal : une règle écrite d'après
  une spécification, sans jeu d'essais officiel pour la contredire, est une règle dont
  personne ne sait si elle est juste.
- **Deux de mes attentes étaient fausses, pas le code.** J'avais écrit que `l'enfant` se
  découpait en trois segments et qu'une marque combinante après un saut de ligne se
  rattachait à la lettre suivante. L'annexe dit le contraire dans les deux cas, et le code
  l'appliquait correctement. J'ai corrigé les tests **et** la documentation du module, qui
  portait le mauvais exemple.
- **Un essai en pseudo-terminal m'a donné un faux négatif**, et il faut le dire : j'avais
  envoyé `1b5b31337e` en croyant envoyer `F2`. C'est `F3`, qui ouvre un dialogue modal, et
  le modal a consommé le `F10` suivant — verdict `timeout`, alors que tout fonctionnait.
  Le comportement était correct ; l'essai était faux. La liste de contrôles de `CLAUDE.md`
  porte désormais les trois codes et cet avertissement.
- **Vérifié dans un pseudo-terminal, essais de contrôle compris** : `redit fichier` dessine
  le menu, le cadre à case `[■]`, le titre `essai.txt  1:1`, le texte, les deux ascenseurs
  et la barre de statut avec `F2` **en gris** — le grisage de l'ADR-007 à l'œil nu, sur un
  document intact. Taper `ABC`, `F2`, `F10` : le fichier sur le disque contient
  `ABCorigine`. `F3`, `Entrée` sur un nom vide : « aucun nom de fichier », le dialogue se
  referme. Sans `F10`, le programme tient.

## 2026-10-10 — Le défaut qui rendait air-tui inutilisable sur macOS

- **`poll` sur `/dev/tty` rend `POLLNVAL` sur macOS.** `PosixTerminal::open` ouvrait
  `/dev/tty` — un choix qui paraissait plus propre, puisqu'il fait survivre l'interface à
  une redirection des flux. Conséquence réelle : `wait_for_input` échouait au **premier**
  appel, et toute application sortait aussitôt. Le tableau mesuré hors d'air-tui :

  | | macOS | Linux |
  |---|---|---|
  | `poll` sur `/dev/tty` | **`POLLNVAL`** | normal |
  | `poll` sur le descripteur 0 du même terminal | normal | normal |
  | `select` sur `/dev/tty` | normal | normal |

  `/dev/tty` n'est pas un périphérique ordinaire : c'est un alias du terminal de contrôle
  résolu à l'ouverture, et son pilote macOS ne prend pas en charge le mécanisme que `poll`
  utilise.
- **Le correctif choisit chaque bout séparément** : le descripteur standard quand c'en est
  un terminal, `/dev/tty` sinon. C'est ce que font ncurses et Turbo Vision, et cela traite
  du même coup le cas mixte — `programme > fichier` garde son entrée sur le descripteur 0,
  donc une attente qui fonctionne, et ne passe par `/dev/tty` que pour écrire. Le seul cas
  qui reste cassé sur macOS est assumé et documenté : `programme < fichier`, où lire les
  frappes du terminal après avoir délibérément redirigé l'entrée est une demande
  contradictoire.
- **Ce qui a laissé passer le défaut, et ce qui l'a trouvé.** Compiler ne prouve rien pour
  une application qui a besoin d'un terminal. `script(1)` ne suffit pas non plus : les
  versions util-linux et BSD ne transmettent pas un stdin redirigé de la même façon, et le
  même essai ne prouve donc pas la même chose sur les deux plateformes — sur macOS il
  donnait « sortie 0 », que j'ai d'abord pris pour « la touche a fonctionné ». C'est
  `tools/pty-drive.py`, et surtout l'**essai de contrôle**, qui ont tranché : sans la
  touche de sortie, le programme doit rester vivant. Sans ce second essai, un programme
  qui sort pour une tout autre raison donne le même verdict que celui qui obéit.
- Le harnais est versionné et les deux essais sont inscrits dans la liste de contrôles de
  `CLAUDE.md`. Il est en Python parce qu'il faut `forkpty`, que la bibliothèque standard de
  Rust n'offre pas : le porter dans `xtask` avec sa propre frontière `unsafe` reste
  possible et n'est pas fait.
- Un test de régression sur l'invariante « un bout sur `/dev/tty` non ouvert est une erreur
  et non une supposition » : l'état ne se produit pas par `open`, mais une invariante tenue
  par construction reste une invariante, et un jour la construction change.
- `air-tui-platform-posix` 0.1.1 → **0.2.0** : l'API ne change pas, mais la sortie va
  désormais sur la sortie standard quand c'en est un terminal, là où elle allait toujours
  sur `/dev/tty`. C'est un changement de comportement observable, donc un mineur et non un
  correctif.

## 2026-10-10 — M5b, barre de menu, barre de statut, champ de saisie, curseur

- **La surimpression annoncée au jalon M5 n'était pas nécessaire.** C'est la découverte du
  jalon, et elle vient d'avoir lu le code avant d'écrire le mécanisme : `draw_view`
  parcourt les enfants de l'arrière vers l'avant, `dispatch_positional` de l'avant vers
  l'arrière, et un test de M4 — `un_clic_va_a_la_vue_du_premier_plan` — le vérifiait déjà.
  Une sœur au premier plan recouvre donc, au dessin comme au routage. `AirMenuBar`
  **grandit** quand elle s'ouvre, son menu reste dans ses propres limites, et le découpage
  par vue garde zéro exception. La contrainte qui reste est réelle et documentée dans
  l'en-tête du module : la barre doit être insérée **en dernier**.
- **Un défaut de fond trouvé chez moi, et corrigé à la cause.** J'avais écrit dans
  `AirMenuBar` une fonction `is_command_enabled()` qui rendait `true` sans condition, avec
  un commentaire la présentant comme une couture — alors que `ARCHITECTURE-fr.md`
  affirmait depuis M5 que « chaque vue interroge le jeu de commandes au moment où elle se
  dessine ». Le chemin de dessin ne recevait pas ce jeu. C'est exactement le NO GO n° 2.
  La correction n'est pas le commentaire : c'est `AirDrawContext`, qui porte le thème
  **et** le jeu de commandes jusqu'à `draw_content`. Le grisage et le refus d'activer
  lisent désormais la même source et ne peuvent plus diverger. Changement de rupture,
  donc `air-tui` passe en 0.6.0.
- **Le curseur matériel manquait**, et sans lui un champ de saisie n'est pas utilisable.
  La règle est portée par la traversée, pas par la vue : `AirView::cursor` répond en
  coordonnées de contenu, `draw_view` ne retient la réponse que d'une vue **focalisée**,
  garde celle du descendant le plus profond, et jette un curseur qui ne passe pas le
  découpage — il irait clignoter sur la vue qui recouvre. Le rendu l'émet **après** les
  cellules, parce que l'écriture de la dernière cellule a laissé le curseur matériel là où
  elle s'est terminée.
- **`AirInputLine` édite par grappes de graphème**, pas par caractère. `é` décomposé est
  deux caractères et une touche effacée ; une famille d'emoji est sept caractères et une
  touche effacée. Se déplacer « d'un caractère » couperait entre une lettre et son accent.
  Les tables de M3 servent enfin à ce pour quoi elles ont été écrites. Le curseur est un
  décalage en octets sur une frontière de grappe : stable à l'insertion comme à la
  suppression, là où un indice de grappe demanderait de recompter tout le texte.
- Un détail trouvé en écrivant le défilement : il doit avancer **par grappes**, pas par
  colonnes. Avec un idéogramme à deux colonnes, avancer d'une colonne couperait le glyphe
  en deux. Et la dernière colonne du champ est réservée au curseur, sans quoi taper en
  bout de champ le placerait hors de la vue.
- **Les deux barres sont dans des phases opposées, et pour des raisons opposées.**
  `AirMenuBar` est en pré-traitement : aucun champ de saisie n'a d'usage pour `Alt`+lettre.
  `AirStatusLine` est en post-traitement : elle publie des touches de fonction, et les
  prendre d'abord reviendrait à les voler à la vue focalisée. Deux conséquences testées :
  fermée, la barre de menu ne consomme **rien** ; et la touche d'un item de barre de statut
  interdit **ne consomme pas** — une frappe qui disparaît sans effet est pire qu'une
  frappe ignorée.
- **L'exemple `frame` est devenu une vraie boucle d'événements** et a été exécuté dans un
  pty, pas seulement compilé : `Alt`+`F` ouvre bien le menu — le cadre du menu déroulant
  apparaît dans le flux de sortie — et le curseur est posé à chaque image. Sur Linux,
  `F10` quitte avec le code 0, et le contrôle inverse le confirme : **sans** `F10`,
  l'exemple reste vivant jusqu'au délai du harnais. C'est ce contraste qui a isolé le
  défaut macOS ci-dessous.
- **Un défaut bloquant sur macOS, trouvé par ce même essai, et pas corrigé ici.** La
  première lecture des traces m'a fait écrire que `F10` quittait l'exemple avec le code 0.
  C'était faux : le code 0 venait d'une autre branche de sortie. En instrumentant les
  quatre sorties de la boucle, le coupable est apparu — `wait_for_input` rend
  `InvalidDevice`, c'est-à-dire que `poll` a rendu `POLLNVAL`. Mesuré hors d'air-tui, le
  comportement est net : sur macOS, `poll` sur un descripteur ouvert sur **`/dev/tty`**
  rend `POLLNVAL`, alors que `poll` sur le descripteur **0** du même terminal fonctionne,
  et que `select` sur `/dev/tty` fonctionne aussi. Sur Linux, les trois fonctionnent. Or
  `PosixTerminal::open` ouvre `/dev/tty` : **toute application air-tui sort immédiatement
  sur macOS**, cible tier-1. Le correctif — employer les descripteurs standards quand ce
  sont des terminaux, comme le font ncurses et Turbo Vision — fait l'objet de la PR
  suivante, pour ne pas mêler un correctif de couche 0 à des widgets.
- Des documents nettoyés de ce qui était devenu faux : la section « Pas fait — et à ne pas
  annoncer comme fait » de `etat-avancement.md` listait encore les tables Unicode, l'arbre
  de vues, les widgets et le découpage par vue, tous faits depuis. Une section qui porte ce
  titre et mentionne des choses faites est le défaut même que les NO GO visent. Et
  `widgets/mod.rs` annonçait toujours « deux widgets seulement ».

## 2026-10-10 — M5, moteur de thèmes et premiers widgets

- **La palette est transcrite, pas approchée.** Plutôt que d'approcher le look Borland de
  mémoire, j'ai lu `cpAppColor` et `cpGrayDialog` dans le source C++ de `magiblot/tvision`,
  qui est sur le disque, et suivi les chaînes de `cpBackground`, `cpMenuView`,
  `cpStatusLine`, `cpFrame` et `cpButton`. Chaque entrée porte l'octet d'origine et sa
  chaîne en commentaire : la transcription se vérifie ligne à ligne.
- **Deux corrections de fidélité que la lecture a révélées.** Les indices VGA ne sont pas
  les indices ANSI : le bleu et le rouge y sont échangés, et transcrire sans conversion
  donnait un bureau rouge et des lettres d'accès rapide bleues. Et `cpStatusLine` partage
  sa chaîne avec `cpMenuView` : la barre de statut est noir sur gris clair, pas sur cyan
  comme les valeurs de M0 le disaient — celles-là venaient d'un souvenir, pas d'une lecture.
- Un désaccord tranché par recoupement : ma lecture de la chaîne donnait le bureau en bleu
  sur gris clair, le port vérifié par instantanés le rend en gris clair sur bleu. Les
  captures d'époque tranchent pour le second, et c'est ce qui est retenu.
- **Quantificateur de couleurs**, requis par ADR-008 : trois profondeurs, déduites de
  `COLORTERM` et `TERM`, forçables par l'application, appliquées au dernier moment dans le
  rendu pour que l'encodeur SGR reste pur. Idempotent, et testé comme tel — sans quoi un
  rendu partiel et un rendu complet donneraient deux images différentes.
- **`AirWindow`** avec sa case de fermeture `[■]` cliquable : l'ordre de dessin la fait
  passer après le titre, donc un titre trop long ne peut pas la recouvrir. La précédence
  est voulue et testée — la case est fonctionnelle, le titre informatif.
- **`AirButton`** avec la notation `~X~` de Turbo Vision. Trois chemins d'activation — clic,
  `Entrée`/`Espace` au focus, `Alt`+lettre de loin — et ils mènent tous à la même commande :
  un raccourci qui ferait autre chose que le clic serait un piège. Une commande grisée
  **consomme** le geste sans rien émettre, pour que ce qui se trouve dessous ne réagisse
  pas à la place.
- Le crochet `draw_frame_decorations` est la seule exception assumée à « une vue n'écrit
  pas sur son cadre » : une case de fermeture est sur le cadre par définition.
- **Périmètre recalé une fois de plus, et dit** : la barre de menu, la barre de statut et
  le champ de saisie passent à un jalon M5b. La première demande un mécanisme de
  surimpression — un menu ouvert sort des limites de son propriétaire — qui mérite d'être
  explicite plutôt que glissé en exception.
- 491 tests. `air-tui` à 98,9 % de régions, `air-tui` passe en 0.5.0 (rupture : des
  variantes de `AirRole` disparaissent).

## 2026-10-10 — M4, l'arbre de vues

- `air-tui/src/view/` : trait `AirView` et composition `AirViewState`, `AirGroup`
  composite, routage en trois phases, focus exclusif entre sœurs, ordre de profondeur,
  bordure par défaut d'ADR-010.
- **`AirPainter` est la pièce qui fait tout tenir.** Une vue dessine à partir de son propre
  `(0, 0)` et ne sait pas où elle se trouve ; le peintre traduit et **découpe**. Le défaut
  du jalon M0 — un contenu trop long qui écrase le cadre qui l'entoure — est devenu
  impossible, et l'exemple `frame` en fait la démonstration en insérant volontairement une
  étiquette plus large que sa fenêtre. Vérifié au-delà du test : sur le rendu réel, les
  bordures tombent aux colonnes 14 et 67 sur **toutes** les lignes, texte accentué et texte
  débordant compris.
- Les coordonnées de souris sont traduites à la descente de l'arbre et restaurées à la
  remontée : une vue reçoit toujours des coordonnées qui sont les siennes, sans savoir où
  elle est. Déplacer plutôt que cloner évite une allocation par vue traversée.
- `AirCommandSet` : le mécanisme 3 de l'ADR-007, en *pull*. Personne n'est notifié, chaque
  vue interroge au dessin. Huit kilooctets de bits couvrent les 65 536 commandes, et un
  test vérifie que désactiver la commande 5 ne désactive pas la 69.
- **Une duplication supprimée en cours de route** : le dessin de texte par grappes existait
  en deux endroits, sur la surface et dans le peintre. `AirSurface::draw_text` délègue
  désormais au peintre. Deux implémentations de la même règle garantissaient qu'elles
  divergent.
- **Périmètre recalé, et dit** : M4 annonçait aussi les widgets de base. Ils passent à M5
  avec le moteur de thèmes, parce qu'ils se tiennent — un bouton a besoin de rôles de
  couleur que la palette actuelle n'a pas. Deux widgets seulement sont livrés ici, pour
  prouver l'arbre : `AirPanel` et `AirStaticText`.
- 409 tests. `air-tui` passe en 0.4.1 (ajout d'API pure).

## 2026-10-10 — M3, les tables Unicode

- `xtask/` : l'outillage du dépôt, en Rust et sans dépendance, avec `download-ucd` et
  `generate-unicode-tables` (plus un mode `--check` qui refuse un fichier généré périmé).
  Le téléchargement passe par `curl` comme processus plutôt que par une pile TLS en
  dépendance, et l'URL porte la **version** et non `latest` : une génération doit pouvoir
  être rejouée des années plus tard (ADR-025).
- `air-tui/src/unicode/` : largeur d'affichage et segmentation en graphèmes, tables
  générées en **Unicode 18.0.0** — sept tables, 2546 plages, 113 Ko de source committée.
  Les règles `GB3` à `GB13` de l'annexe 29 sont implémentées, **`GB9c` comprise** : la
  coupure conjointe indienne, sans laquelle le curseur coupe au milieu d'un `क्ष`.
- **Le parseur a trouvé la règle que j'aurais manquée.** Il a refusé une ligne à trois
  champs de `DerivedCoreProperties.txt` — `094D ; InCB ; Linker` — parce qu'il était
  strict. C'est ainsi que `GB9c` est entrée dans le périmètre : un parseur tolérant aurait
  ignoré ces lignes en silence et la segmentation des écritures indiennes aurait été
  fausse sans que rien ne le signale.
- **Les tables servent à quelque chose** : `AirSurface::draw_text` parcourt les grappes et
  pose une cellule de continuation après une grappe large, que le rendu saute. Un texte
  idéographique n'écrase plus la bordure de sa fenêtre, et ce défaut a son test.
- Choix consigné : la largeur d'une grappe est le **maximum** des largeurs de ses
  caractères, pas leur somme — une famille d'emoji occupe deux colonnes, pas huit.
- Un fichier généré passe les mêmes contrôles que le code écrit à la main : les
  doc-comments émis manquaient d'accents graves autour des noms de règles et faisaient
  échouer `clippy -D warnings`. Corrigé dans le **générateur**, pas dans le produit.
- `air-tui` passe en **0.4.0** : `AirCell` gagne `is_continuation`, donc rupture pour
  toute construction littérale (ADR-009). `xtask` naît en 0.1.0, non publiée.
- `NOTICE` ajouté : les données du référentiel Unicode sont sous `Unicode-3.0`, et c'est la
  seule donnée tierce du dépôt.
- 322 tests. Sept modules à 100 % de couverture : les six de codec et d'entrée, plus
  `unicode/mod.rs`.
- **Génération reproductible, et vérifiée comme telle** : macOS `x86_64`, hydrogen
  (Linux `x86_64`) et neon (Linux `aarch64`) ont chacune retéléchargé le référentiel et
  régénéré les tables. Le fichier committé correspond à l'octet dans les trois cas. C'est
  ce qu'ADR-025 promet, constaté plutôt qu'affirmé.

## 2026-10-10 — M2, le protocole clavier Kitty

- `air-tui/src/input/kitty.rs` et l'extension du décodeur : frappes `CSI … u` avec
  modificateurs complets, types d'événement dont le relâchement, et touche de la
  disposition de base — `AirKeyEvent::base_layout` plus `shortcut_key()`, de quoi faire
  fonctionner `Alt+F` sur un clavier cyrillique. Repli `modifyOtherKeys` niveau 2.
- Négociation par interrogation `CSI ? u` **suivie** d'un jalon d'attributs de
  périphérique : voir le jalon sans réponse Kitty avant lui est une conclusion négative
  certaine, pas une expiration de délai. Le délai ne sert qu'au terminal muet. Ce qui est
  tapé pendant l'interrogation est transmis, pas jeté. Le protocole est défait au `Drop`.
- **Deux corrections de fond trouvées en écrivant les tests** : les paramètres de séquence
  étaient en `u16`, ce qui faisait refuser `CSI 128512 u` — un emoji en numéro de touche —
  comme malformé ; et les numéros hors table tombaient dans la zone à usage privé
  d'Unicode, donc devenaient des caractères imprimables fantômes qu'une application aurait
  insérés dans un champ de saisie. Les deux sont couvertes par un test nommé.
- `air-tui` passe en **0.3.0** : `base_layout` est un champ ajouté à une structure
  publique, donc une rupture pour toute construction littérale. En `0.y.z`, la rupture
  prend la case du milieu (ADR-009).
- 212 tests, 100 % de couverture sur les six modules de codec et d'entrée, 371 416
  exécutions de fuzzing sans plantage sur le décodeur étendu.
- **Vérifié sur les trois plateformes**, et le contrôle `aarch64` était le plus
  important : sur neon (Raspberry Pi, Ubuntu 26.04, noyau 7.0.0-1020-raspi), 249 tests au
  vert et l'exemple `frame` rendant une grille identique. La déclaration **variadique**
  d'`ioctl` est donc juste — sur cette ABI les arguments variadiques passent par la pile,
  et une déclaration non-variadique aurait fonctionné sur `x86_64` en cassant ici. C'était
  le défaut latent le plus probable du projet.
- **Affirmations périmées corrigées** : `CLAUDE.md`, `AGENTS.md`, `README.md` et
  `CONVENTIONS-fr.md` nommaient encore `air-tui-sys`, crate renommée par ADR-011. Les ADR
  antérieurs et le journal ne sont pas réécrits — ce sont des archives — et une note de
  lecture dans `CONVENTIONS-fr.md` fait le pont.

## 2026-10-10 — vérification sur Linux, dette soldée

- Branche M1 vérifiée sur **hydrogen** (Ubuntu 26.04.1 LTS, noyau 7.0.0-38, `x86_64`) :
  `fmt`, `clippy -D warnings`, 188 tests, `build --locked`, `deny check` propres ;
  couverture globale 95,0 % de régions, codecs et modules d'entrée à 100 % ; exemple
  `frame` rendant une grille identique à celle de macOS ; 371 229 exécutions de fuzzing
  supplémentaires sans plantage.
- Les trois valeurs propres à Linux de la couche POSIX sont confirmées justes :
  `TIOCGWINSZ = 0x5413`, `nfds_t` sur 64 bits, et `poll` rendant `POLLIN` sur `/dev/null`
  — la divergence avec macOS, consignée au jalon M0, est exacte dans les deux sens.
- **Un défaut trouvé, de la classe exacte qu'une seule plateforme de développement laisse
  passer** : un import ne servant qu'à un test gardé par `cfg(target_os = "macos")` faisait
  échouer `clippy -D warnings` sur Linux, contrôle obligatoire du projet. Corrigé dans la
  même branche ; `air-tui-platform-posix` passe en 0.1.1.
- Reste hors vérification : `aarch64` — jamais construit, et c'est là que le piège
  variadique d'`ioctl` sur l'ABI d'Apple se manifesterait — et la console Linux native,
  inaccessible par SSH.

## 2026-10-10 — M1, le décodeur ANSI d'entrée

- `air-tui/src/input/` : modèle d'événements conforme à l'ADR-003 et décodeur de séquences
  ANSI. Caractères UTF-8, commandes C0, touche alternative, CSI et SS3 avec modificateurs,
  souris SGR 1006, collages encadrés, focus. `air-tui` passe en 0.2.1 (ajout d'API pure).
- **100 % de couverture** sur les trois modules d'entrée, comme l'ADR-012 l'exige d'un
  codec. La contrainte a de nouveau dicté la forme du code : deux branches « entrée vide »
  ont été supprimées en passant à l'appelé l'octet de tête que l'appelant connaissait déjà,
  et les trois fermetures `|_| {}` des tests d'erreur ont été fondues en une seule, qu'un
  nouveau test exerce.
- Cible `cargo-fuzz` `ansi_decoder`, dans un espace de travail séparé puisque le fuzzing
  exige nightly et que le dépôt est pinné sur stable. **494 565 exécutions, aucun
  plantage.** Au-delà de l'absence de panique, l'invariant qui compte est vérifié :
  découper l'entrée en deux lectures produit la même suite d'événements qu'un morceau
  unique.
- Limite consignée plutôt que découverte plus tard : sur un pty sans protocole Kitty,
  `Ctrl+Shift+S` reste indiscernable de `Ctrl+S`. C'est le codage historique qui ne sait
  pas l'exprimer, et c'est l'objet du jalon M2.

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
