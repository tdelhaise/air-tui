# Journal de bord d'air-tui

Archive chronologique **append-only**, en ordre **anté-chronologique** : la plus récente en
tête. On ajoute, on ne réécrit jamais une entrée passée.

Une entrée par campagne, pas par commit. Répartition des rôles : `etat-avancement.md` est
l'état vivant, ce journal est la trace figée, `INDEX.md` est la carte structurelle.

## 2026-10-12 — Jalon M7l : la recherche, une question au modèle et une politique au contrôleur

- **La décision du jalon est que chercher n'est pas une action.** L'ADR-007 fait passer
  toute mutation et tout déplacement par une `AirTextAction` ; chercher ne change rien,
  donc `find_from` est une méthode de **lecture**. L'énumération des actions — la table qui
  dit tout ce qui peut arriver au texte — n'a pas bougé d'une ligne, et je n'ai écrit aucune
  action nouvelle.
- **Et le gain n'est pas théorique.** Une action `Find` aurait obligé le tampon à répondre
  « je n'ai rien trouvé » par le canal des changements structurels, c'est-à-dire à inventer
  un `AirTextChange` pour un fait qui n'en est pas un : **rien n'a changé**. L'échec d'une
  recherche est une valeur de retour, et c'est le seul endroit où il ne demande rien à
  personne. Le même raisonnement sépare ensuite ce qui reste au contrôleur : le bouclage est
  une politique qui se voit à l'écran, et un motif n'est pas une propriété du document.
- **Ce que le contrôleur fait de la réponse tient en deux actions existantes** : un
  déplacement qui efface la sélection vers le début de l'occurrence, un déplacement qui
  l'étend vers sa fin. Appliquées au même tour, donc la vue ne voit que l'état final.
- **L'unité de comparaison est la grappe de graphème, et ce n'est pas un raffinement.** Un
  curseur d'air-tui se déplace par grappes depuis M3 : une occurrence qui commencerait au
  milieu d'une grappe donnerait une sélection que le curseur ne sait pas décrire. Chercher
  `e` dans un `é` décomposé ne trouve donc rien. Le détail agréable est que la comparaison
  grappe à grappe **est** la vérification des frontières — il n'y a pas de passe de
  validation à oublier, donc rien à oublier.
- **Deux attentes d'essai fausses, et le code avait raison les deux fois.** « ÉTÉ » fait
  cinq octets et non six — `É` en fait deux, `T` un seul. Et surtout : mon essai des
  frontières de grappe cherchait `e` dans « étendu », qui porte un **second** `e` ; la
  recherche le trouvait à bon droit, et l'essai accusait la bonne règle d'un défaut qu'elle
  n'avait pas. Il porte maintenant sur « état », et la raison du choix de mot est écrite à
  côté.
- **Un motif n'est pas coupé de ses espaces, là où un nom de fichier l'est.** Chercher
  « le » entre deux espaces est une demande légitime, et la couper en ferait une autre. Les
  deux questions passent par le même dialogue, donc la distinction est écrite dans la
  **réponse** — c'est l'endroit où elle se décide, pas le dialogue.
- **Le motif est retenu avant d'être cherché**, même quand il ne donne rien : corriger une
  faute de frappe et continuer est le geste suivant le plus probable. Un essai le fixe, pour
  qu'une « simplification » ne le défasse pas.
- **Trois gestes, et chacun a sa source.** `Ctrl`+`L` est le geste de Borland, qui répète la
  recherche dans son IDE ; `F3`, que le même IDE emploie, est pris par « Ouvrir » dans
  `redit` ; `F7` est celui de l'éditeur de `mc`, et il est dans la barre de statut parce
  qu'une fonction qui ne s'y montre pas est introuvable sans lire la documentation.
- **Ce qui n'est pas fait est écrit comme tel** : pas de recherche en arrière — `redit` n'a
  pas de geste pour la demander, et un réglage sans lecteur est ce que ce dépôt refuse
  d'écrire —, pas de remplacement, pas d'expression régulière, et pas de case « respecter la
  casse » : le modèle sait les deux, `redit` ne sait poser que l'insensible, parce que
  `AirPromptDialog` tient un champ et aucun parcours de focus interne.
- **Vérification** : 1339 essais, et trois essais en pseudo-terminal qui vont **par paire**
  avec leur contrôle. `F7`, « belle », `Entrée`, `Ctrl`+`X`, `F2` laisse « une  journee »
  dans le fichier : la coupe prouve la sélection, ce qu'un examen de la sortie n'aurait pas
  fait. Un motif absent laisse le fichier intact — sans ce contrôle, un `Ctrl`+`X` qui
  couperait n'importe quoi donnerait le même verdict. Et `Ctrl`+`L` coupe l'occurrence de la
  **seconde** ligne, ce qui distingue « continuer » de « chercher encore une fois ».

## 2026-10-12 — Jalon M7k : l'ombre portée des boutons, et le champ qui a attendu son second lecteur

- **Le meilleur résultat du jalon est ce qui n'a pas été écrit.** L'ADR-015 affirmait que
  son mécanisme suffirait aux boutons ; la vérification est que `draw_view_shadow` n'a gagné
  **aucun** cas particulier pour eux. Un bouton est l'enfant d'une fenêtre ou d'un dialogue,
  qui possède donc la zone où son ombre tombe, et le découpage du peintre n'a pas bougé.
- **Ce qui diffère est l'apparence, et elle diffère par trois choses à la fois** : la forme
  — un trait qui **épouse** la vue, là où celle d'une fenêtre est le rectangle **décalé**
  moins la vue —, le remplissage — des demi-blocs, là où l'autre recolore en gardant le
  caractère — et le rôle de couleur. Trois différences simultanées, c'est la raison d'une
  **énumération** à deux variantes plutôt que de trois champs de réglage sur `AirShadow` :
  une ombre paramétrée par trois boutons décrirait des combinaisons qu'aucun thème ne
  remplit, et un réglage sans lecteur est exactement ce que ce dépôt refuse d'écrire.
- **Les demi-blocs sont une transcription, et non une idée de rendu.** Turbo Vision écrit
  `0xDC` et `0xDF` de la page de code 437 — `▄` et `▀` — pour qu'une ombre de bouton
  paraisse **deux fois plus fine** que celle d'une fenêtre : la cellule à droite du bouton
  n'est dans l'ombre que par sa moitié basse, celle du dessous par sa moitié haute. La ligne
  du dessous est décalée d'une colonne, parce que l'ombre est portée en bas **et** à droite.
- **Une seconde convergence mesurée, et celle-là ne se devinait pas.** Un demi-bloc ne
  couvre que la moitié de sa cellule, donc j'ai gardé le **fond** de ce qui est dessous et
  n'ai changé que l'avant-plan — sans quoi l'autre moitié ne montrerait rien. Turbo Vision,
  lui, écrit les deux couleurs de l'attribut `0x70`. Les deux règles donnent le **même
  octet** dans un dialogue, parce que son fond *est* ce gris : l'ombre des boutons de `redit`
  sort en `38;5;0;48;5;7`, soit exactement `0x70`. Elles ne diffèrent que là où ce dépôt
  pose des boutons et où Borland n'en posait pas — la fenêtre **bleue** de l'exemple `frame`,
  qui donne `38;5;0;48;5;4` au lieu d'un rectangle gris collé sur du bleu.
- **Le premier champ que les deux thèmes du dépôt remplissent différemment.** `mc` met
  `None` à l'ombre de bouton : ses boutons sont `[ OK ]`, plats, et son format de skin ne
  décrit rien de tel. La règle écrite au jalon M7j — « une assiette ne gagne un champ que le
  jour où deux thèmes le remplissent autrement » — est donc respectée plutôt que contournée,
  et le **contrôle** de l'essai est ce thème-là, pas un thème fabriqué pour l'occasion.
- **`AirRole::ButtonShadow` a enfin un lecteur**, cinq jalons après sa transcription. Aucun
  rôle n'a été ajouté : la palette en compte toujours trente-neuf, et c'est l'un d'eux qui a
  été **lu**.
- **Un second propriétaire de surface, et une fonction rendue publique pour lui.**
  `AirPromptDialog` compose ses trois contrôles lui-même — ils ne paraissent pas dans ses
  enfants —, donc la traversée de l'arbre ne voit pas leurs ombres. Il appelle la **même**
  fonction que la traversée plutôt que de refaire le calcul : une règle écrite deux fois
  finit par ne plus l'être qu'une. C'est aussi ce qui fait que le champ de saisie n'en
  projette pas sans qu'une exception soit écrite nulle part — c'est son état qui le dit.
- **Une ligne de plus dans ce dialogue, et ce n'est pas un espacement.** Sa zone de contenu
  en a cinq au lieu de quatre, la cinquième étant celle où l'ombre des boutons tombe. Sans
  elle, le découpage l'écarterait : juste, et invisible. Deux essais qui comptaient six
  lignes ont été corrigés — et c'est la bonne direction de correction, puisque la dimension
  demandée a réellement changé.
- **Les trois essais nouveaux ont été vus échouer** avant d'être crus : en posant
  `button_shadow: None` dans les décorations de Turbo Vision, la géométrie, le fond conservé
  et la table des décorations tombent ensemble, chacun en nommant ce qu'il attendait.
- **Vérification** : 1313 essais, `fmt`, `clippy -D warnings`, `deny check`, les cinq
  contrôles d'`xtask` et les trois `import-mc-skin --check`. En pseudo-terminal, l'exemple
  `frame` émet deux `▄` et vingt-deux `▀` — dix et douze colonnes de bouton —, le même
  compte dans le dialogue de `redit`, et **zéro** sous le thème de `mc`, dont les douze
  `38;5;8;48;5;0` d'ombre de fenêtre sont inchangés.

## 2026-10-10 — Jalon M7j : la peinture hors des limites, et l'invariant qui n'a pas bougé

- **La question posée était « une vue peut-elle peindre hors de ses limites ? ». La réponse
  est non, et c'est le meilleur résultat du jalon.** Ce qui dépasse d'une vue est peint par
  le **propriétaire de la surface où cela dépasse** : l'ombre appartient au bureau, pas à la
  fenêtre, et c'est donc le bureau qui l'écrit — dans ses propres limites, juste avant de
  dessiner la fenêtre. Le découpage du peintre reste sans exception, donc rien de ce que M4
  et M5b garantissent n'est à revérifier.
- **Le dépôt employait déjà une solution de cette famille sans l'avoir nommée**, et c'est
  ce qui m'a convaincu que la troisième stratégie n'était pas nécessaire : la barre de menu
  **agrandit ses propres limites** à l'ouverture d'un menu, depuis le jalon M5b. Le
  découpage est respecté parce qu'il n'y a plus rien dehors.
- **Agrandir ne convenait pas à l'ombre, et la raison est concrète** : la zone d'ombre
  entrerait dans les limites de la fenêtre, donc dans sa **détection de clic** et dans le
  calcul de son intérieur. Cliquer sur l'ombre activerait la fenêtre, et son contenu serait
  disposé dans un rectangle plus grand que ce que l'on voit. L'ombre n'appartient pas à la
  fenêtre : elle appartient à ce qui est derrière.
- **La troisième stratégie — une marge de débordement déclarée par la vue — est la plus
  courte à écrire et la plus chère.** Elle rend l'invariant **conditionnel** : « cette vue
  a-t-elle une marge ? » devient une question qu'on se pose partout. Elle ouvre aussi la
  porte à une vue qui peindrait sur ses sœurs sans que l'ordre de dessin le dise, ce qui est
  précisément ce que le découpage empêchait.
- **`AirTheme` est enfin le triplet que l'ADR-008 décrit depuis le jalon M5.** L'écart avait
  été assumé pendant quatre jalons, et pour une raison qui n'était pas de la paresse : une
  assiette qui aurait décrit une ombre que rien ne dessine aurait annoncé une capacité
  absente. Elle en décrit maintenant une que l'on dessine.
- **Deux champs ne sont pas dans l'assiette, et c'est délibéré.** Le style de trait vit
  déjà dans le jeu de glyphes — c'est par là que les dialogues en trait simple de `mc` se
  distinguent des `╔═╗` de Borland —, et le marqueur `~X~` des touches rapides est le même
  dans les deux thèmes. Une assiette ne gagne un champ que le jour où deux thèmes le
  remplissent différemment.
- **Une convergence que je n'attendais pas, et qui s'est mesurée toute seule.** La couleur
  de l'ombre de Turbo Vision n'est **pas** transcriptible d'une chaîne de palette : l'ombre
  d'une fenêtre n'a pas d'entrée dans `cpAppColor`, seule celle d'un bouton en a une
  (`cpButton[8]`, déjà transcrite et toujours lue par personne). J'ai donc retenu l'attribut
  VGA `0x08` en écrivant que ce n'était pas une transcription. Puis l'importeur a compilé
  `[core] shadow` du skin par défaut de `mc` : `gray;black`, c'est-à-dire `pair(8, 0)` —
  exactement la même valeur. Deux apparences qui concordent ne font pas une source, mais
  elles font un repère, et le pseudo-terminal l'a confirmé : les deux thèmes émettent la
  même séquence `38;5;8;48;5;0`, douze fois chacun.
- **L'ombre recolore ce qui est dessous en gardant le caractère**, et c'est ce qui en fait
  une ombre plutôt qu'un rectangle collé à la fenêtre. Vérifié sur les octets émis de la
  façon la plus directe : le **seul** caractère que l'exemple `frame` peint à la couleur
  d'ombre est `░`, le motif du bureau.
- **Un essai que j'ai dû corriger parce que sa prémisse était fausse.** J'avais écrit
  « l'ombre garde le caractère » en remplissant la surface d'un motif avant de dessiner — et
  le panneau conteneur repeint son fond d'espaces par-dessus. Le seul conteneur qui montre
  l'effet est le **vrai bureau**, qui remplit du `░` de Turbo Vision. L'essai le dit
  maintenant, et il est meilleur : il éprouve le cas réel au lieu d'un montage.
- **Trois rôles de l'assiette, et la palette passe à trente-neuf.** `AirViewState::casts_shadow`
  dit quelle vue en projette une — faux par défaut, parce qu'une ombre sur chaque vue
  couvrirait l'écran de gris —, `AirDecorations` dit sa taille, la palette dit sa couleur.
  Un thème sans ombre les éteint toutes d'un seul endroit.
- **1307 essais**, dont sept pour l'ombre : les deux bandes, le non-recouvrement de la
  fenêtre, le caractère conservé, le thème sans ombre, la vue sans ombre, le découpage au
  bord du bureau, et l'ordre à deux temps entre deux fenêtres superposées.
- Il reste que les **boutons** ne projettent pas d'ombre, et que `AirRole::ButtonShadow` est
  toujours lu par personne. Le mécanisme y suffirait — un bouton est l'enfant d'une fenêtre,
  qui possède donc la zone où son ombre tomberait — et c'est écrit comme pas fait.

## 2026-10-10 — Jalon M7i : plusieurs skins compilés, et le choix par un nom

- **La limite que M7g avait laissée.** Les trente-neuf skins livrés avec `mc` étaient
  importables, mais pas **ensemble** : le dépôt ne portait qu'un artefact, et importer le
  second effaçait le premier. Une capacité qu'on ne peut employer qu'une fois à la fois
  n'est pas tout à fait une capacité.
- **La décision qui ne touche pas à l'ADR-008, et c'est le point.** `theme/skins/mod.rs`
  est devenu un **registre généré** qui déclare les modules et les nomme, et
  `AirTheme::from_command_line_name` résout trois formes : `tv`, `mc`, et `mc:<nom>`. Rien
  n'est lu à l'exécution — le choix est une clef dans une table construite à la
  compilation, pas un fichier ouvert au démarrage. L'espace de noms `mc:` dit d'où vient
  un thème, ce qu'un nom nu ne dirait pas.
- **Le registre est généré parce que deux listes disent la même chose.** La liste des
  modules et la table des noms divergeraient si on les tenait à la main : un artefact
  ajouté sans son entrée serait du code mort que `clippy` signalerait, une entrée sans son
  artefact ne compilerait pas. Les deux sortent du même balayage du répertoire, à chaque
  import.
- **J'ai mesuré avant de décider combien de skins reprendre.** Les trente-neuf coûtent
  395 Kio de source générée et **deux dixièmes de seconde** de compilation — 1,03 s contre
  1,26 s sur cette machine. C'est peu, et ce n'est donc pas le coût technique qui tranche :
  ce qui tranche est que reprendre trente-neuf skins d'un projet sous GPLv3 dans un dépôt
  sous MPL-2.0 ressemble à vendorer son répertoire de skins, et que l'argument du `NOTICE`
  — « ce qui est repris est la donnée de configuration, pas du code » — est une décision
  de l'auteur du dépôt, pas la mienne. J'en ai donc repris **trois**, chacun pour ce qu'il
  démontre, et écrit le chiffre à côté de la décision pour qu'elle reste ouverte.
- **Une régression que j'ai introduite et rattrapée en relisant.** En remplaçant le
  `ThemeChoice` des deux applications par un nom, j'ai donné `tv` pour défaut aux deux —
  alors que `rmc` s'ouvre aux couleurs de `mc`, ce que son propre doc-comment disait :
  « une coquille qui s'appelle `rmc` s'ouvre aux couleurs de `mc` ». Trouvée en relisant le
  mode d'emploi que je venais de modifier, pas par un essai : aucun n'affirmait le défaut
  de `rmc` autrement que par une comparaison à `ThemeChoice::default()`, qui disparaissait
  avec le type. L'essai qui le dit maintenant compare à `DEFAULT_THEME`.
- **La résolution d'un nom est dans `air-tui`, et pas dans les deux applications.** Elles
  la demandaient toutes les deux, et `ThemeChoice` était déjà écrit deux fois — une fois
  dans chaque `main.rs`, avec des défauts différents. Écrite deux fois, une règle finit par
  ne plus l'être qu'une.
- **Une arête du dépôt, payée deux fois en écrivant ce jalon.** `xtask` dépend d'`air-tui`,
  donc un registre désaccordé de ses fichiers **empêche l'outil de se bâtir** — et c'est
  l'outil qui écrit le registre. Retirer un artefact demande donc de corriger `mod.rs` à la
  main une fois avant de relancer un import. C'est le prix d'un espace de travail unique
  (ADR-005), et il est écrit dans `CLAUDE.md` plutôt que redécouvert.
- **Le nom d'un thème est le nom du skin transposé en identifiant Rust**, minuscules
  comprises : `seasons-autumn16M.ini` donne `mc:seasons_autumn16m`. Un module Rust n'admet
  ni tiret ni majuscule, et `clippy -D warnings` refuse `non_snake_case` — ce que j'ai
  appris en voyant la compilation échouer sur `seasons_autumn16M`. Plutôt que de faire
  porter au registre deux noms à tenir d'accord, `--list-themes` donne les noms exacts et
  fait foi.
- **Vérifié sur les octets émis, les quatre thèmes dans le même binaire** : `tv` donne
  `38;5;0;48;5;7`, `mc:default` donne `38;5;0;48;5;6`, `mc:modarcon16_defbg` donne
  `38;5;15;48;5;2` avec seize `;49m` — le fond du terminal —, et `mc:seasons_autumn16m`
  donne `38;2;0;0;0;48;2;255;201;12` avec trois soulignés. Un seul exécutable, quatre
  apparences, rien de lu au démarrage.
- **1296 essais**, 7 codecs toujours à 100 %.

## 2026-10-10 — Les deux derniers gates de conventions, et la frontière que personne ne tenait

- **Les six contrôles de `docs/CONVENTIONS-fr.md` sont écrits.** La section « À outiller »
  du document a disparu, et avec elle la dernière règle que ce dépôt tenait à la main en le
  sachant.
- **Je les ai chiffrés avant de les écrire, et le chiffre était zéro.** Soixante-dix-neuf
  fichiers portent tous l'avis MPL ; onze blocs `unsafe` portent tous leur `// SAFETY:`,
  la concordance tenant fichier par fichier. Ce sont donc des gates de **prévention**, pas
  des réparations — et le dire est la moitié de leur valeur. `check-dco` avait trouvé un
  trou réel dès son premier passage ; ces deux-là ne trouvent rien, et annoncer le contraire
  serait exactement le défaut que le NO GO n° 2 nomme.
- **Leur intérêt est ailleurs, et il est précis** : un fichier **nouveau** est le moment où
  l'une de ces deux règles s'oublie, et c'est aussi le moment où personne ne relit les trois
  premières lignes des soixante-dix-huit autres.
- **Le seul apport réel est une règle que personne ne m'avait demandé d'outiller.**
  `CLAUDE.md` écrit que « `unsafe` et les appels C vivent dans `air-tui-platform-posix`, et
  nulle part ailleurs », et c'est l'ADR-011. Rien ne le vérifiait. Les deux crates du cœur
  le tiennent par leur `#![forbid(unsafe_code)]` — mais `redit`, `rmc`, `xtask` et les
  cibles de fuzzing ne le portent pas, donc un `unsafe` y aurait compilé sans un mot.
  `check-safety-comments` porte désormais la frontière, et c'est la seule des deux règles de
  ce jalon qui garde quelque chose qui n'était pas gardé.
- **« Un bloc `unsafe` » n'a pas une seule réponse syntaxique, et c'était la vraie question
  de conception** — celle que j'avais annoncée en proposant le jalon. Cinq formes portent le
  mot-clé. Seule `unsafe { }` exige une justification, parce que seule elle **affirme** au
  compilateur quelque chose qu'il ne vérifie pas. `unsafe extern` est une déclaration de
  symboles que l'édition 2024 oblige à marquer : elle n'affirme rien sur un appel, et
  l'engagement se prend à l'appel. `unsafe fn`, `unsafe impl` et `unsafe trait` reportent
  l'engagement sur l'appelant, qui le documente par une section rustdoc `# Safety` — ce que
  `clippy::missing_safety_doc` exige déjà. Les quatre sont **comptées et annoncées** à
  chaque exécution plutôt qu'exemptées en silence, comme les pardons de `check-dco`.
- **Et j'ai pu montrer pourquoi un `grep` ne suffisait pas, au lieu de l'affirmer.**
  `xtask/src/naming.rs` porte deux fois le mot `unsafe` dans des **chaînes de
  caractères** : sa liste de mots-clés, et un source d'essai. Un contrôle de frontière
  fondé sur `grep` déclarerait donc `xtask` en infraction dès le premier passage — et on
  apprendrait à l'ignorer, ce qui est la façon dont un gate meurt. Le mot-clé est cherché
  par le balayage lexical de `check-naming`, **partagé** plutôt que réécrit : deux scanners
  donneraient deux comportements dont un seul serait éprouvé, et celui-là l'est sur près de
  dix mille noms.
- **Trois contrôles négatifs avant d'y croire** : un avis retiré d'un fichier, un
  `// SAFETY:` retiré d'un bloc, un `unsafe { }` ajouté dans `redit`. Les trois sont
  attrapés et nommés avec leur fichier et leur ligne. Un essai confronte aussi `licence.rs`
  à lui-même — le contrôle porte l'avis qu'il exige.
- Le marqueur est cherché sur **trois** lignes au-dessus du bloc, et non une : une
  justification tient rarement sur une ligne, et les blocs du dépôt le montrent. La borne
  existe pour qu'un marqueur d'un autre bloc ne couvre pas celui-ci, et les deux bords
  portent leur essai.

## 2026-10-10 — Jalon M7h : l'annulation, et la mutation unique qu'elle a imposée

- **La décision à trancher était entre deux choses qui portent le même nom.** Un
  **instantané** du tampon par pas d'annulation est bête et incapable de se tromper, au prix
  de la taille du document à chaque frappe — un fichier d'un mégaoctet coûterait un
  mégaoctet par caractère tapé. Un **journal de remplacements** coûte la longueur du texte
  qui change, donc un caractère pour une frappe. Le rapport entre les deux n'est pas un
  facteur constant : il est la taille du document. L'ADR-014 choisit le journal, et nomme ce
  qu'on paie en le choisissant — il n'est juste que si chaque mutation produit son inverse
  exact.
- **Et c'est ce prix qui a payé, mieux que la fonctionnalité elle-même.** Le tampon portait
  **sept** chirurgies de lignes écrites à la main, chacune découpant et joignant à sa façon.
  Sept chemins de mutation, c'est sept inverses à tenir d'accord. Il n'y en a plus qu'un :
  `splice(début, fin, texte)`, auquel les sept se ramènent par un triplet. L'annulation
  n'est donc pas une couche posée à côté du tampon — c'est une propriété de son unique point
  de mutation, et un huitième chemin d'édition qui la contournerait ne compilerait pas.
- **Deux défauts trouvés, et aucun des deux n'était dans ce que j'écrivais.** Le premier
  par un essai **existant** : la plage d'une dernière ligne supprimée doit partir de la fin
  de la **précédente**, parce que le saut de ligne à emporter est devant elle et non
  derrière. L'écriture d'origine retirait la ligne du vecteur, ce qui masquait entièrement la
  question ; la ramener à un remplacement l'a posée, et le filet de plus de cent essais du
  tampon a attrapé l'écart au premier passage. C'est exactement ce qu'on attend d'une
  refonte : qu'elle fasse apparaître les questions que l'ancienne forme ne posait pas.
- **Le second par les essais de l'annulation elle-même, et il était de conception.** Taper
  sur une sélection faisait **deux** pas — une suppression, puis une insertion —, donc
  l'utilisateur aurait dû annuler deux fois un geste qu'il n'avait fait qu'une. J'avais écrit
  l'essai pour vérifier que les deux revenaient « dans le bon ordre », et c'est en le lisant
  que le défaut m'est apparu : l'ordre était juste, le nombre de pas était faux. Les trois
  insertions remplacent désormais la sélection dans le **même** remplacement — ce qui a
  aussi retiré un appel sur trois chemins, et fait qu'annuler rend la surbrillance.
- **Le geste est l'unité, et j'ai écrit ses bords parce qu'ils sont arbitraires.** Midnight
  Commander annule caractère par caractère, Turbo Vision ne garde qu'un seul pas : ni l'un ni
  l'autre n'est un argument. Ce qui en est un, c'est qu'un collage de dix lignes annulé en
  dix fois serait un défaut, et qu'une ligne tapée annulée d'un coup en serait un autre. La
  règle de fusion tient en deux cas — une insertion dans une insertion contiguë, un retour
  arrière dans un retour arrière contigu — et tout le reste coupe.
- **Le drapeau de modification est devenu dérivé, et c'était nécessaire.** Un drapeau que
  seule une édition allume reste allumé sur un texte revenu à son état enregistré : annuler
  tout aurait laissé l'étoile dans le titre de `redit`. Il se lit maintenant de
  l'historique, par comparaison avec le point d'enregistrement — qui suit la borne du
  journal et devient **injoignable** si l'entrée qui y menait est oubliée. C'est le seul
  endroit où la borne de deux cent cinquante-six pas se voit, et il porte son essai.
- **Le choix de la seconde touche s'est fait après vérification, et mon premier texte était
  faux.** L'ADR, dans sa première écriture, affirmait que « `redit` n'a jamais lié
  `Ctrl`+`Y` ». C'est faux : la vue d'édition la lie à la suppression de ligne depuis M6,
  avec un commentaire qui cite Turbo Vision. J'ai relu le code avant de publier, corrigé la
  phrase, et choisi `Ctrl`+`R`. `Ctrl`+`Maj`+`Z` a été écarté pour une raison mesurable et
  non par goût : un terminal qui ne négocie pas le protocole Kitty ne le distingue pas de
  `Ctrl`+`Z`, donc le rétablissement marcherait sur certains terminaux et pas sur d'autres.
- **`Ctrl`+`Z` ne suspend pas le processus, et je l'ai vérifié au lieu de l'espérer.**
  `cfmakeraw` efface `ISIG`, ce que le commentaire de `ffi.rs` nomme déjà. Sans cela, la
  touche la plus évidente de ce jalon aurait mis l'éditeur en arrière-plan.
- **Aucun mécanisme de plus dans l'ADR-007, et c'est un résultat.** Le grisage des deux
  commandes est en *pull*, lu au dessin comme celui de `COPY` depuis M7d ; aucun changement
  structurel n'a été ajouté. Les deux touches sortent par le canal des **intentions** et non
  des commandes, parce que l'historique vit dans le modèle que la vue adresse déjà — l'exact
  inverse du presse-papiers, qui est au contrôleur parce qu'il est partagé. Les deux
  décisions suivent la même règle : une chose appartient à qui la connaît.
- **Vérifié en pseudo-terminal par paires**, et la paire compte ici plus qu'ailleurs : le
  contrôle du rétablissement est l'essai d'**annulation seule**, pas un essai sans touche.
  Sans `Ctrl`+`Z`, le fichier porte déjà « ABCorigine » — un rétablissement muet donnerait
  donc le même fichier qu'un rétablissement qui marche. Un troisième essai montre la coupure
  du geste : « ab », `Début`, « z », `Ctrl`+`Z` rend « aborigine » et non « origine ».
- **1263 essais**, dont soixante nouveaux, 7 codecs toujours à 100 %.
- Trois notes périmées corrigées dans `statuts.toml` : l'ADR-008 y annonçait « trente-deux
  rôles » et les deux limites de thème comme restant à lever, l'ADR-012 « six codecs » là où
  il y en a sept. Le fichier d'avancement des ADR dérive comme n'importe quel décompte en
  prose, et c'est le troisième jalon de suite qui en trouve.

## 2026-10-10 — Le gate `check-dco`, écrit par le défaut qu'il ne pouvait pas empêcher

- **Le troisième des trois contrôles « à outiller » est écrit**, et c'est le commit de
  fusion du jalon M7g qui l'a demandé : `806f7ae` est le seul commit de `main` sans
  `Signed-off-by`, parce qu'un corps de fusion écrit à la main a remplacé celui que GitHub
  aurait construit depuis les commits de la branche, trailers compris.
- **Mesurer avant d'écrire a changé la conception, deux fois.** Vingt-trois commits sur
  `main`, un seul sans signature, zéro `Co-Authored-By` : une portée « tout l'historique »
  est donc tenable, à un pardon près. Et dix-neuf de ces vingt-trois sont des fusions squash
  que GitHub signe de `…@users.noreply.github.com` alors que la signature porte l'adresse de
  l'auteur — donc **ne pas** confronter les deux adresses. Je l'aurais fait d'instinct, et
  presque tout l'historique aurait échoué pour une raison qui n'est pas celle du DCO.
- **Un pardon nommé plutôt qu'une portée restreinte en douce.** La tentation était de ne
  garder que la plage de la branche : le contrôle serait passé au vert sans mentir, et plus
  personne n'aurait su que `main` porte un commit sans DCO. Le pardon est donc une constante
  avec l'empreinte complète, la raison écrite à côté, et un compte rendu qui l'annonce à
  **chaque** exécution, succès compris. Un contrôle qui taît ce qu'il ne regarde pas donne
  la même assurance qu'un contrôle absent.
- **Ce contrôle ne peut pas empêcher le défaut qui l'a motivé, et c'est écrit partout où il
  est décrit.** Une fusion squash est fabriquée **après** le dernier passage de la CI sur la
  branche : aucune vérification de branche ne peut la voir. D'où deux portées — la plage sur
  une pull request, l'historique entier sur une poussée vers `main` — et une phrase qu'il
  faut assumer : sur `main`, le contrôle **constate** au lieu de prévenir. C'est tout ce
  qu'un dépôt qui interdit le force-push peut obtenir, et c'est déjà le passage du silence
  au rouge.
- **Un contrôle plus large que sa règle écrite est le défaut symétrique de celui qu'il
  répare.** `CLAUDE.md` nommait `Co-Authored-By: Claude …` et rien d'autre ; le client
  propose aussi un trailer `Claude-Session:`. J'ai refusé les deux — et **étendu la règle
  écrite** dans `docs/CONVENTIONS-fr.md` au lieu de laisser le code dépasser le document en
  silence. De même, un co-auteur **humain** reste légitime : aucune règle du dépôt ne
  l'interdit, et la liste des marques d'agent est courte et explicite plutôt que large et
  devinée.
- **La lecture d'un seul appel à `git log`, avec l'ambiguïté détectée au lieu d'être
  supposée absente.** Les séparateurs sont U+001E et U+001F ; un message qui en porterait un
  ferait rendre trois champs à un enregistrement, et le découpage **refuse** en nommant le
  caractère. Mal lire un message de commit pour conclure qu'il est signé serait pire que ne
  pas le lire.
- **Son premier passage en CI a trouvé un fait d'environnement que je n'avais pas prévu.**
  Sur un événement `pull_request`, `actions/checkout` ne place pas la tête de la branche
  mais un commit de fusion que **GitHub fabrique** pour éprouver le résultat — « Merge
  <sha> into <base> ». Personne ne l'a écrit, personne ne peut le signer, et il était dans
  la plage : le gate a donc échoué sur son propre passage, pour une raison juste. Les
  commits à plus d'un parent sont désormais comptés et ignorés — ils n'apportent aucun
  contenu à certifier, ce qui est aussi pourquoi `git rebase --signoff` ne les signe pas.
  Une fusion **squash** n'en est pas une, un seul parent : les dix-neuf de `main` restent
  examinées. Un gate dont le premier échec est un vrai renseignement vaut mieux qu'un gate
  vert du premier coup.
- **Vérifié par ses contrôles négatifs avant d'y croire.** Sur une branche jetable : un
  commit sans signature, un `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>` et un
  `Claude-Session:` sont chacun attrapés et nommés, la commande rend le code 1, une vraie
  fusion `--no-ff` est bien comptée comme ignorée, et l'historique propre repasse au vert.
  Un gate qu'on n'a pas vu échouer ne garde rien — c'est la même discipline que le test de
  garde des attributs au jalon M7g.
- **Dix-neuf essais, tous sur la partie pure** : un commit y est une empreinte, un nombre
  de parents et un message,
  donc les règles se vérifient sans dépôt, sans branche jetable et sans `git`.
- Il reste **deux** contrôles à outiller : `check-licence-headers` et
  `check-safety-comments`. Le décompte est corrigé partout où il figurait — `CLAUDE.md`,
  `docs/CONVENTIONS-fr.md`, `docs/etat-avancement.md` —, parce qu'un décompte de dettes est
  exactement le genre de nombre qui survit à sa dette.

## 2026-10-10 — Jalon M7g : les deux limites du thème, et une troisième qui n'en était pas une

- **Le résultat tient en deux nombres : neuf skins importables sur trente-neuf avant, les
  trente-neuf après.** Mais le chemin entre les deux est l'intérêt du jalon, parce qu'il a
  démenti la prévision chiffrée sur laquelle il était fondé.
- **Un attribut par rôle.** `AirColorPair` porte `attributes` en plus de ses deux couleurs :
  un rôle décrit une présentation, pas seulement un couple. Le thème Turbo Vision n'en pose
  aucun — les seize couleurs du VGA portent déjà la distinction vive/sombre dans l'index
  lui-même —, et c'est précisément ce qui rend le défaut de ce jalon invisible.
- **Le défaut que ce jalon rend possible est silencieux, donc il lui fallait un test de
  garde.** Un widget qui construit sa cellule à partir des deux champs de couleur du couple
  laisse l'attribut au bord du chemin, et aucun des deux thèmes livrés n'en pose : rien ne
  le montrerait pendant le développement, et seuls les skins de `mc` le feraient voir — en
  production. D'où une palette d'essai dont les trente-huit rôles sont gras, et un test qui
  peint les douze widgets et refuse la moindre cellule qui ne l'est pas. Je l'ai vérifié par
  son contrôle avant de m'y fier : rétabli sur un seul site, il échoue en nommant le widget,
  la cellule et sa position. Un test de garde qu'on n'a pas vu échouer ne garde rien.
- **`AirColor::TerminalDefault` n'est pas une couleur de plus, c'est l'absence de choix.**
  Elle s'émet par les paramètres SGR `39` et `49`, qui annulent une couleur au lieu d'en
  poser une, et elle traverse les trois profondeurs du quantificateur inchangée. C'est la
  seule dans ce cas, et la raison n'est pas une commodité : la quantification cherche la
  plus proche voisine d'une couleur connue, et celle-ci est celle que l'utilisateur a réglée
  dans son terminal. La remplacer par un index effacerait exactement ce qu'elle demande.
- **La prévision chiffrée de l'ADR-008 était fausse, et la mesure l'a dit tout de suite.**
  L'ADR annonçait que lever ces deux limites rendrait « vingt-neuf des trente refusés »
  importables. Les deux levées, le compte est passé de neuf à **seize**. Le défaut du
  raisonnement ne se voit qu'à la mesure : l'importeur s'arrête à la **première** cause de
  refus qu'il rencontre, donc le décompte comptait une cause par skin alors que la plupart
  en avaient deux. Une mesure qui compte des causes exclusives là où elles se cumulent ne se
  corrige pas en raffinant le décompte — elle se corrige en levant une limite et en
  recomptant.
- **Et la troisième cause n'était pas une limite du modèle. C'était un défaut de
  l'analyseur.** Vingt-trois skins restaient refusés pour une clef absente : dix-huit
  n'écrivent pas `[editor] editframeactive`, quatre n'ont pas de `[editor] _default_`,
  `gotar.ini` n'a pas de `[core] inputmark`. L'ADR rangeait ce cas parmi les limites du
  modèle d'air-tui et écrivait que « `mc` en applique un [défaut] ; air-tui préférerait
  qu'on décide lequel ». Il n'y avait rien à décider : le manuel de `mc` décrit la cascade
  en deux endroits — `[core] _default_` est « used in all other sections if they not contain
  color definitions », et le `_default_` de chaque section est « used [core]._default_ if
  not specified ». Une clef absente hérite de sa section, puis de `[core]`. Aucun de ces
  vingt-trois skins n'est mal formé, et `mc` les affiche tous.
- **La leçon que je retiens du jalon : une limite attribuée au modèle est une hypothèse
  tant qu'elle n'a pas été levée.** Deux l'étaient vraiment. La troisième était une lacune
  de trois lignes rangée du mauvais côté pendant six jalons, et il a suffi de lire le manuel
  — pas le code de `mc`, son manuel — pour s'en apercevoir. Ce qui l'a cachée est le confort
  d'un refus : un refus se lit comme une décision, alors que celui-là n'en était pas une.
- **`AirAttributes` gagne `blink`, et pour une raison qui n'est pas l'exhaustivité.** Aucun
  des trente-neuf skins ne l'emploie — mesuré avant de l'écrire. Il est là pour que les cinq
  attributs que `mc` sait écrire aient tous une expression, donc pour qu'il n'y ait pas de
  branche de refus. Un test confronte les cinq noms de l'analyseur aux cinq champs de la
  structure : le refus « cet attribut n'a pas d'expression » devient démontrablement
  inatteignable, au lieu d'être supposé tel.
- **Un rôle composé retient les attributs de la clef qui donne l'avant-plan**, et la règle
  est écrite à côté de lui. Les trois attributs que les skins emploient portent sur le dessin
  du caractère, et c'est l'avant-plan qui le décrit ; prendre ceux du fond mettrait en gras
  un texte dont le skin ne dit rien.
- **L'artefact committé n'a pas bougé d'un octet, empreinte comprise.** Le skin par défaut
  ne pose aucun attribut et n'emploie pas la couleur du terminal, donc la forme courte
  `pair(f, b)` le décrit toujours en entier — ce qui est la meilleure preuve que ce jalon
  n'a rien changé à ce qui marchait. Les quatre fonctions d'aide se décident en revanche
  séparément maintenant : un attribut suffit à envoyer un couple d'index sur la forme
  longue, qui emploie `index` sans qu'aucune vraie couleur paraisse, et les émettre ensemble
  laisserait `rgb` en code mort que `clippy -D warnings` refuse à juste titre.
- **Vérifié de bout en bout sur les octets émis, et sur des skins jusqu'ici refusés.**
  `modarcon16-defbg` fait apparaître `;49m` vingt fois dans la sortie de l'exemple `frame` —
  le fond du terminal, qui traverse la palette, le quantificateur et l'encodeur — et `[0;1;`
  quatre fois. `seasons-autumn16M` fait apparaître `[0;4;38;2;0;0;0;48;2;255;201;12m` : un
  souligné sur une vraie couleur, pour la lettre d'accès rapide d'un menu. Compiler ne
  prouvait rien ici ; ce qui prouve, c'est l'octet `4` dans la séquence.
- **Cinq décomptes faux corrigés, tous hérités de M7d et M7f.** « trente-deux rôles » dans
  trois doc-comments, « huit assiettes » là où il y en a onze, et « quinze correspondances
  annotées » dans l'ADR-008 là où l'outil en compte seize depuis deux jalons. C'est le
  troisième jalon de suite qui en trouve, et le motif est toujours le même : un décompte
  écrit en prose à côté d'une table qui grandit.
- **Deux affirmations fausses corrigées dans `cell.rs`**, du genre inverse et aussi
  trompeur : le doc-comment d'`AirColor` annonçait le quantificateur « prévu au jalon M5 et
  **n'est pas écrit** », celui d'`AirCell` les tables UCD de même. Les deux existent depuis
  M5 et M3. Un commentaire qui sous-estime ce qui existe envoie chercher ailleurs ce qui est
  déjà là.
- **1203 essais**, 7 codecs toujours à 100 %, trente-neuf skins sur trente-neuf.

## 2026-10-10 — Jalon M7f : les cases à cocher et les boutons radio

- **Un seul type pour les deux, et c'est la structure de Borland sans son héritage.**
  `TCheckBoxes` et `TRadioButtons` dérivent tous deux de `TCluster` et ne diffèrent que par
  trois choses : la marque dessinée, ce qu'un appui fait, et si un déplacement du curseur
  emporte la sélection. Tout le reste — la disposition en lignes, la navigation aux flèches,
  les lettres d'accès rapide, le clic, le dessin — est commun. Deux types Rust séparés
  auraient dupliqué ce reste, et une règle écrite deux fois finit par ne plus l'être qu'une :
  c'est la leçon de M7c et de M7d, appliquée une troisième fois. D'où un `AirCluster`, un
  `AirClusterKind` qui porte la différence, et deux constructeurs nommés pour que le site
  d'appel dise lequel il construit.
- **Le déplacement qui choisit vient de Borland, pas d'une préférence.**
  `TRadioButtons::movedTo` affecte `value` : dans un groupe à choix unique, la flèche
  sélectionne. Un curseur qui s'y promènerait sans rien choisir obligerait à appuyer après
  chaque flèche, pour un geste qui n'a qu'une issue possible. Dans un groupe de cases, la
  même flèche ne fait que déplacer. Les deux comportements portent chacun leur essai, parce
  que c'est exactement le genre d'asymétrie qu'une refonte aplatit sans le voir.
- **La lecture de `cpAppColor` s'est recoupée toute seule.** `cpCluster "\x10\x11\x12…"`
  donne `cpGrayDialog[16..18]`, soit `cpAppColor[47..49]`, que `tvdemo2.cpp` nomme
  « Cluster normal », « Cluster selected » et « Cluster shortcut ». En relisant la table
  pour ces trois indices, j'ai vérifié au passage que les valeurs des listes transcrites à
  M7b — 57, 58, 59 — concordent à l'octet avec la même lecture. Trois jalons, trois
  extractions indépendantes, aucune divergence : c'est ce qui donne confiance dans la
  méthode plutôt que dans le résultat.
- **Six glyphes qui ne viennent pas du skin, et le dire était la seule option honnête.**
  `[`, `X`, `]`, `(`, `•`, `)` : un balayage des trente-neuf skins de `mc` 4.8.33 montre
  qu'**aucun** ne porte de clef pour eux. Ils restent donc ceux du thème par défaut, et ce
  n'est pas une transcription. La tentation était d'écrire ce que `mc` affiche de mémoire ;
  je n'ai pas de source pour l'affirmer, donc l'artefact dit « aucun skin ne les décrit »
  plutôt que d'inventer une provenance.
- **Un décompte faux trouvé en y touchant, hérité de M7b.** Le commentaire de l'artefact
  généré et la doc de `AirGlyphSet` annonçaient « cinq glyphes » hors skin et en énuméraient
  **six** : `scroll_thumb` avait été ajouté à M7b sans que la phrase suive. Il est à douze
  maintenant, et juste aux trois endroits qui le portent. Un décompte dans un document
  généré est exactement ce qui dérive sans qu'on le voie, puisque personne ne le recompte.
- **Une généralisation de l'ADR-008 démentie par la mesure, et c'est le résultat le plus
  intéressant du jalon.** L'ADR affirmait qu'« un rôle de plus est une clef de plus à savoir
  exprimer » — formulation écrite à M7b, quand deux skins avaient été perdus. Ce jalon
  ajoute trois rôles et le compte **ne bouge pas** : toujours neuf sur trente-neuf, parce
  que les trois se branchent sur `[dialog] _default_`, `dfocus` et `dhotnormal`, que la
  table lisait déjà. Ce qui fait baisser le compte est une **clef** nouvelle, pas un rôle
  nouveau. La phrase est corrigée, et le contre-exemple est écrit à côté.
- **L'invariant du choix unique vaut aussi à la construction.** Un item coché ajouté à un
  groupe radio décoche les autres. Sans cela, un groupe mal monté afficherait deux points et
  le premier appui en effacerait un sans qu'on sache lequel — un défaut qui ne se voit qu'à
  l'écran, jamais à la relecture du code qui monte le dialogue.
- **Deux états légitimes, écrits plutôt que corrigés.** Un groupe radio neuf n'a **aucun**
  choix : « rien n'est encore décidé » est un état réel, et en cocher un d'office déciderait
  à la place de l'utilisateur. Et décocher le choix courant est accepté, parce que le
  refuser obligerait l'appelant à reconstruire le groupe pour revenir à cet état.
- **Le curseur ne se montre que dans un groupe qui a le focus**, et l'exemple `frame` met
  les deux groupes **côte à côte** — la seule disposition qui le vérifie à l'œil. Le champ
  de saisie y perd son focus au profit des cases : deux vues focalisées se disputeraient le
  curseur du terminal, et la traversée n'en retiendrait qu'un.
- **Vérifié sur les octets émis, dans les deux thèmes** : `[X]`, `[ ]`, `(•)` et `( )`
  apparaissent dans la sortie de `frame` comme dans celle de `frame mc`, et la couleur du
  curseur de groupe du skin de `mc` y est présente. Un essai refuse par ailleurs que les
  glyphes des deux thèmes divergent, puisque rien ne les distingue.
- **Une extraction imposée par clippy, et le découpage s'est révélé juste** : `emit`
  dépassait cent lignes, et c'est l'émission du jeu de glyphes qui en est sortie. Les deux
  sorties ne partagent que le tampon qu'elles remplissent, donc la coupure était là où la
  fonction avait déjà deux sujets.
- **Vérifications** : `fmt`, `clippy -D warnings`, **1191 essais** (trente de plus),
  `deny check`, `check-naming`, `check-versions`, `check-codec-coverage` — 7 codecs à
  100 % —, `import-mc-skin --check`, et les essais en pseudo-terminal avec leurs contrôles.

## 2026-10-10 — Jalon M7e : le double et le triple clic

- **La décision qui restait à prendre était de placement, et trois endroits possibles
  s'excluaient pour trois raisons différentes.** C'est le résultat de cette campagne : la
  question « où vit l'horloge » avait une réponse unique dès qu'on écrivait les contraintes.
  Le **décodeur** la refuse parce qu'il est pur, et cette pureté n'est pas décorative : elle
  est ce qui permet au fuzzing de vérifier qu'un découpage arbitraire de l'entrée produit la
  même suite d'événements. Une horloge dedans, et le même flux rejoué donnerait deux
  résultats. Les **widgets** la refusent parce que ce serait la même règle écrite douze fois
  — la leçon de M7c, appliquée cette fois *avant* de la repayer. Et `dispatch_event` la
  refuse parce que c'est une fonction pure sur l'arbre, testable sans horloge, et qu'elle
  doit le rester. Reste la **boucle d'application**, seul endroit qui voit tous les
  événements avant l'arbre et qui connaît déjà le temps.
- **Le seuil et ses conditions sont transcrits, pas inventés.**
  `TEventQueue::doubleDelay = 8` ticks de l'horloge BIOS à 18,2 Hz, soit 440 ms, et les
  trois conditions de `tevent.cpp` reprises telles quelles : le même bouton, **exactement
  la même cellule**, le délai. La condition de position est stricte chez Borland — aucune
  tolérance d'une cellule —, et elle le reste : une cellule de terminal est assez grande
  pour qu'un double clic y tienne. La comparaison est `<=`, et un essai le dit en cliquant
  pile au seuil.
- **Le triple clic venait avec**, parce que c'est le même compteur. Borland le gère
  (`meTripleClick`) et plafonne au triple : un quatrième clic rapide reste un triple chez
  lui comme ici. L'ignorer aurait fait du troisième clic un second double, ce qui est
  précisément le défaut qu'on remarque sans savoir le nommer.
- **Le suiveur ne tient pas d'horloge, et c'est ce qui le rend vérifiable.** L'instant lui
  est donné, et la mesure d'écart aussi, par une fermeture. Les essais emploient un simple
  compteur de millisecondes, la boucle un vrai `Instant` : **treize essais déterministes**
  là où un `Instant::now()` à l'intérieur aurait demandé de faire dormir un test, donc un
  test lent et instable que personne n'aurait osé resserrer.
- **L'événement s'ajoute au lieu de remplacer, et la rétrocompatibilité en découle.** Un
  double clic produit `Pressed` **puis** `DoubleClicked`. L'ordre compte — l'enfoncement
  place le curseur, le clic multiple étend ensuite —, et un widget qui ignore le geste garde
  le comportement du simple clic au lieu de ne plus rien recevoir. C'est obtenu par
  construction, pas par précaution : il n'y a aucun cas à traiter pour que ce soit vrai.
- **Un redimensionnement coupe l'enchaînement**, parce que la cellule d'avant ne désigne
  plus la même chose. Sans cela, agrandir le terminal entre deux clics ferait un double clic
  sur un autre widget que celui visé.
- **Trois usages, et le troisième n'est pas une sélection.** Dans la vue d'édition, le
  double clic prend le segment et le triple la ligne. Dans un champ d'une ligne, le triple
  vaut « tout sélectionner », parce qu'il n'y a qu'une ligne à prendre. Dans une **liste**,
  le double clic **ouvre** l'item — le geste de Midnight Commander. Le même événement, trois
  sens selon le widget : c'est exactement pourquoi la boucle le synthétise sans décider quoi
  en faire, et pourquoi il ne pouvait pas être « la sélection d'un mot » dans le framework.
- **Le segment, et non « le mot ».** `unicode::segment_at` rend les bornes du segment au
  sens de l'UAX #29 : un mot, mais aussi une suite d'espaces ou un signe de ponctuation. Un
  double clic sur un espace sélectionne les espaces, parce que rendre « rien » laisserait le
  geste sans effet visible. Le nom de l'action le dit — `SelectSegment` et non `SelectWord`,
  qui aurait menti dès le premier espace.
- **Vérifié en pseudo-terminal, avec son contrôle**, et c'est ce qui prouve la chaîne
  entière : séquence SGR, décodeur, synthèse dans la boucle, distribution, intention,
  modèle, enregistrement. Deux clics sur « belle » puis `Ctrl`+`X` donnent « une  journee » ;
  un **seul** clic laisse le texte intact. Sans ce second essai, le premier ne prouverait
  pas que c'est le *double* clic qui agit.
- **Un piège de coordonnées, consigné dans `CLAUDE.md`.** Le format SGR compte depuis un, et
  la vue d'édition de `redit` commence à la colonne 1, ligne 2 de l'écran — sous la barre de
  menu et dans le cadre de sa fenêtre. Se tromper d'une cellule fait cliquer dans le cadre,
  et le geste n'a plus d'effet : un faux négatif qui ressemble à un défaut.
- **Un point de catégorie, écrit pour qu'on n'y voie pas un oubli** : `input/clicks.rs`
  n'est pas soumis à la règle des 100 % de l'ADR-012. Ce qui fait un codec est de convertir
  un flux, pas de vivre sous `input/`, et ce fichier agrège des événements avec une horloge
  sans en convertir aucun. La liste de `xtask` est nominative, et porte désormais la raison.
- **Vérifications** : `fmt`, `clippy -D warnings`, **1161 essais** (trente-sept de plus),
  `deny check`, `check-naming`, `check-versions`, `check-codec-coverage` — 7 codecs à
  100 % —, `import-mc-skin --check`, et les essais en pseudo-terminal avec leurs contrôles.

## 2026-10-10 — Jalon M7d : la sélection de texte, et le presse-papiers

- **Le plus gros chantier de la liste, et c'est l'ADR-007 qui en sort renforcé.** La
  sélection posait trois questions de placement, et le découpage MVC les a tranchées sans
  qu'il faille l'amender. Où vit la sélection ? Dans le modèle, parce que le texte y est.
  Mais ce que la vue dessine est un intervalle de **colonnes**, quand les bornes sont des
  décalages en octets : la conversion demande le début de la ligne, que le reflet ne garde
  pas. Elle se fait donc à l'étape 3, au moment où le contrôleur pousse le reflet — seul
  instant où la vue voit le modèle. **La contrainte a désigné la place ; il n'a pas fallu
  la choisir.**
- **Une ancre, et non deux bornes ordonnées.** C'est la décision qui porte tout le reste.
  La sélection est l'intervalle entre l'ancre et le curseur, *dans l'ordre où ils se
  trouvent*. Deux bornes déjà triées feraient perdre le sens du geste : sélectionner trois
  caractères vers la droite puis remonter vers la gauche doit **réduire** la sélection, pas
  en commencer une autre. Un essai le dit dans chacun des deux widgets.
- **Les quatorze `Move*` sont devenues une seule variante**, `Move { motion, extend }`, avec
  `AirTextMotion` pour le déplacement et deux constructeurs qui nomment les deux valeurs du
  drapeau : `move_by` efface la sélection, `select_by` l'étend. Le raisonnement vient
  directement de M7c : décliner `MoveLeft` et `SelectLeft` aurait obligé à ajouter chaque
  déplacement futur à deux endroits, et une règle écrite deux fois finit par ne plus l'être
  qu'une. C'est une rupture d'API, et elle est assumée en `y`.
- **La transcription des couleurs a validé l'existant, ce qui ne s'était jamais produit.**
  `cpEditor "\x06\x07"` donne `cpBlueWindow[6]` et `[7]`, soit `cpAppColor[13]` et `[14]` ;
  `tvdemo2.cpp` nomme 13 « Normal text » et 14 « Highlighted text ». Or `cpAppColor[13]` est
  déjà `AirRole::WindowText`, transcrit au jalon M5 : **le texte non sélectionné de
  l'éditeur n'a pas besoin d'un rôle propre, et la vue peignait juste depuis le début** —
  sans qu'on sache pourquoi. Deux rôles attendus, un seul nécessaire.
- **Le presse-papiers n'est ni au modèle ni à la vue.** Il est partagé entre les deux
  widgets éditables, donc il appartient au contrôleur. Les vues le **demandent** par trois
  commandes — `COPY`, `CUT`, `PASTE` — sans jamais le toucher, exactement comme un
  ascenseur demande une position sans rien défiler. C'est la deuxième fois que ce motif
  s'impose de lui-même, et il n'a pas été cherché. `AirClipboard` refuse un texte vide :
  couper une sélection vide ne doit pas détruire une copie que l'utilisateur croyait sûre.
- **Le collage encadré était décodé depuis M1 et jeté depuis M1.** `AirInputEvent::Paste`
  existait, aucun widget ne le lisait : la séquence était reconnue, puis perdue. C'est le
  même genre de trou que M7c venait de trouver dans le type d'événement — une capacité
  décodée dont personne n'avait écrit le consommateur, et que rien ne signalait. Les deux
  widgets éditables le traitent désormais par la même action `InsertText` que le
  presse-papiers interne, parce qu'un collage n'est pas une suite de frappes : le traiter
  comme telle ferait avancer la génération une fois par caractère.
- **Un troisième compteur de génération, et la raison vaut d'être retenue.** Abandonner une
  sélection ne déplace pas le curseur, et la vue doit pourtant repeindre. Réutiliser la
  génération du curseur aurait laissé une **surbrillance orpheline** à l'écran. Un compteur
  par chose qui peut changer seule — la règle se vérifie en essayant de la violer, et
  l'essai qui l'éprouve passe par l'abandon explicite de la sélection.
- **Le piège de la vérification, repris de M7b et payant.** Le surlignage se vérifie sur les
  **couleurs** de la surface rendue, jamais sur les caractères : un essai qui ne regarderait
  que le texte passerait alors que rien n'est surligné. Les deux widgets ont donc une aide
  d'essai qui rend les fonds de chaque colonne.
- **Quatre attendus faux, le code juste à chaque fois.** Le surlignage après défilement
  horizontal — la fenêtre montre les colonnes 5 à 8, donc le `gh` sélectionné tombe en
  positions 1 et 2, pas 2 et 3. Coller juste après avoir copié : la sélection est encore là,
  donc le collage la **remplace** au lieu de la dupliquer — comportement correct, et l'essai
  a été scindé en deux, « coller sur une sélection la remplace » et « copier puis coller
  hors sélection duplique », chacun disant ce qu'il vérifie. Et le premier essai en
  pseudo-terminal rendait `aabbZ` là où j'attendais `ababZ` : après deux `Maj`+Gauche le
  curseur est **à gauche** de la sélection, donc le collage tombe au milieu. Le scénario
  était mal pensé ; refait avec `Fin`, il rend `abZab`. Un attendu écrit de mémoire se
  trompe là où le code raisonne.
- **Le champ de saisie fait exception, et c'est écrit comme tel.** Il tient sa sélection
  lui-même, parce qu'il tient déjà son texte : lui donner un modèle pour une ligne serait
  payer l'ADR-007 sans rien en retirer. La frontière n'est pas « tout widget a un modèle »,
  elle est « ce qui est partagé ou persistant a un modèle ».
- **Mesuré, pas supposé** : neuf skins importables sur les trente-neuf livrés avec `mc`, un
  de moins qu'à M7b. Le perdu est `gotar.ini`, et pour une raison **nouvelle au décompte** —
  non pas un attribut que la palette ne porte pas, mais une clef purement **absente**,
  `[core] inputmark`. Les refus se répartissent désormais en vingt attributs, neuf couleurs
  « default » et une clef manquante. L'amorçage de l'artefact à la main et sa régénération
  par l'outil ont donné les mêmes valeurs, ce qui vérifie la table autant que l'artefact.
- **Ce qui reste, et qui est maintenant le dernier geste manquant** : le double clic. Il
  sélectionnerait un mot, `unicode` sait découper les mots depuis M6, et tout est là **sauf
  l'horloge**. Le décodeur la refuse parce qu'il est pur ; la boucle d'application en a une.
  C'est une décision de placement, pas un développement, et elle n'est pas prise.
- **Vérifications** : `fmt`, `clippy -D warnings`, **1124 essais** (soixante de plus),
  `deny check`, `check-naming`, `check-versions`, `check-codec-coverage` — 7 codecs à
  100 % —, `import-mc-skin --check`. Essais en pseudo-terminal avec leur contrôle, dont un
  nouveau qui sélectionne par `Maj`+Gauche, copie par `Ctrl`+`C` et colle par `Ctrl`+`V` :
  le fichier enregistré porte `abZab`. Il vérifie au passage que **`Ctrl`+`C` n'interrompt
  pas** l'application — le mode brut désactive `ISIG`, et rien ne l'avait éprouvé.
- **Versions** : `air-tui` 0.9.0 (rupture d'API), `redit` 0.2.0, `rmc` 0.1.2, `xtask`
  0.3.2.

## 2026-10-10 — Jalon M7c : le type d'événement de touche, décodé puis écarté

- **Le défaut que M7b avait consigné était plus large que sa description, et deux
  affirmations de M7b étaient fausses.** C'est le résultat principal de cette campagne, et
  il tient à l'ordre dans lequel les choses ont été faites : mesurer avant d'écrire. Le
  jalon précédent avait noté « le filtrage des relâchements n'est fait que par
  `AirListBox` » et rangé cela en défaut ouvert. En l'instruisant, le filtre s'est révélé
  presque inutile et la cause bien plus en amont.
- **Le décodeur ignorait le type d'événement partout où il compte.** Le protocole Kitty
  place le type en sous-paramètre du champ des modificateurs, et cette règle vaut pour
  *toutes* les formes. `air-tui` ne la lisait que sur la forme `CSI … u` — celle des
  caractères — et pas sur l'encodage historique des touches spéciales : flèches, `Début`,
  `Fin`, `Inser`, `Suppr`, `F1` à `F20`. Autrement dit le type était décodé là où il ne
  sert à presque rien, et perdu là où les widgets vivent. Mesuré avant de corriger :
  `\x1b[;1:3B` ressortait en **enfoncement** de flèche bas.
- **Donc le commentaire que j'avais écrit dans `AirListBox` à M7b décrivait un effet que
  son filtre ne prévenait pas** — « les traiter ferait descendre le curseur de deux items
  par appui sur la flèche ». La flèche relâchée descendait bien le curseur, et le filtre
  n'y pouvait rien : l'événement n'arrivait jamais marqué comme un relâchement. Un filtre
  qui rassure sans protéger est pire qu'un filtre absent, pour la même raison que le NO GO
  n° 1 : il donne une assurance que personne ne revérifie.
- **La forme réellement émise omet le numéro de touche**, et la page du protocole ne le
  montre pas. Elle donne la règle générale — le type est un sous-champ des modificateurs —
  et aucun exemple de flèche. S'arrêter là aurait laissé une inférence au cœur d'un codec.
  La vérification est venue de `kitty/key_encoding.py`, qui n'écrit le numéro de touche que
  s'il diffère de un : la séquence est `CSI ; 1 : 3 B`, **champ vide**. Les deux formes sont
  décodées et éprouvées, celle avec le numéro comme celle sans.
- **Le filtre est à la distribution, en un seul endroit.** `view::dispatch_event` écarte un
  relâchement avant la descente dans l'arbre. Douze widgets qui répètent une règle sont
  douze endroits où elle peut manquer — c'est précisément ce que M7b avait livré, onze
  widgets exposés et un protégé pour rien. L'information n'est pas détruite pour autant :
  une boucle d'application tient l'événement d'entrée avant de le distribuer, donc qui a
  besoin des relâchements les a encore. C'est l'argument qui a fait préférer cet endroit à
  la boucle elle-même, que `rmc` et `redit` écrivent chacun de leur côté.
- **Une répétition passe, et son essai compte autant que l'autre.** Maintenir une flèche
  doit répéter le déplacement : écarter « tout ce qui n'est pas un enfoncement » aurait été
  le défaut symétrique, moins visible et plus pénible à l'usage. Les deux moitiés de la
  règle portent chacune leur essai, dans le décodeur et dans la distribution, parce qu'un
  filtre trop large passerait l'un et casserait l'autre.
- **L'essai en pseudo-terminal était possible, contre ce que M7b avait écrit** (« aucun
  essai ne le voit, il faut un terminal Kitty réel »). Un pty nu ne négocie pas le
  protocole, mais il n'a pas à le faire : **il suffit d'émettre les octets**, car le
  décodeur décode ce qu'il reçoit sans égard pour ce qui a été négocié. Les deux essais
  qu'exige `CLAUDE.md` ont été faits sur un binaire d'avant et un binaire d'après, et ils
  sont décisifs. Avant : quatre repeints, la marque apparue au troisième et **disparue au
  quatrième**, le titre portant `4 o` — la flèche relâchée avait descendu le curseur, et le
  relâchement d'`Inser` avait démarqué. Après : trois repeints, la marque dans le dernier,
  `2 o`. Le nombre de repeints le dit à lui seul.
- **Deux pièges de lecture de cette sortie, consignés dans `CLAUDE.md`.** Le titre d'un
  panneau est tronqué à la moitié de la largeur : monté sous un chemin de bloc-notes
  temporaire, il n'émet jamais `[1 marqué — 2 o]`, et son absence ressemble à un défaut
  alors que c'est un titre coupé — première mesure perdue à cela. Et un motif trouvé dans
  la sortie n'est pas l'état final : chaque frappe produit un repeint, la sortie les porte
  tous, et `1 marqué` au troisième repeint sur quatre dit que la marque a existé puis
  disparu. Ma première lecture concluait l'inverse du bon verdict.
- **L'exigence de couverture a corrigé un essai, et non l'inverse.** L'aide d'essai écrite
  d'abord extrayait l'événement d'un `match` dont le bras d'échec portait un `panic!` —
  jamais pris, donc deux régions non couvertes, donc un codec en défaut d'ADR-012 pour du
  code de test. Réécrite en aide **constructrice**, qui fabrique l'événement attendu au
  lieu d'inspecter celui qu'on a reçu, elle n'a plus de branche morte et les essais sont
  revenus au style du fichier. Que le code d'essai compte dans la mesure est voulu, et ce
  n'est pas la première fois que cela oriente une écriture.
- **Vérifications** : `fmt`, `clippy -D warnings`, **1064 essais** (dix de plus), `deny
  check`, `check-naming` (8489 noms, 72 fichiers), `check-versions`,
  `check-codec-coverage` — **7 codecs à 100 %** de régions, de fonctions et de lignes —, et
  `import-mc-skin --check` (empreinte `0xefc735864405630c`, inchangée). Fuzzing du décodeur
  touché : **282 946 exécutions en 61 secondes, aucun plantage**. Essais en pseudo-terminal
  avec leur contrôle : `frame`, `frame mc`, `redit` (« ABCorigine » écrit) et `rmc` sortent
  sur leur touche et tiennent sans elle.
- **Versions** : `air-tui` 0.8.1 — une correction de défaut, aucune API nouvelle, donc `z`
  et non `y` ; `redit` 0.1.3, `rmc` 0.1.1 et `xtask` 0.3.1, touchées par l'alignement de la
  dépendance que l'ADR-009 exige d'incrémenter.

## 2026-10-10 — Jalon M7b : les listes, et la coquille `rmc` à deux panneaux

- **Les deux moitiés que l'ADR-008 sépare existent maintenant séparément.** M7a avait le
  thème de `mc` sans sa coquille ; M7b a la coquille, et elle porte le thème de Turbo
  Vision aussi bien que celui de `mc`. L'avertissement de l'ADR — « ce qui fait reconnaître
  Midnight Commander est sa coquille, pas sa palette » — n'est plus une affirmation à
  croire : `redit --theme mc` donne un éditeur aux couleurs de `mc`, `rmc --theme tv` donne
  la coquille de `mc` aux couleurs de Borland, et ni l'un ni l'autre n'est `mc`.
- **Les quatre rôles de liste ont éprouvé le moteur de thèmes au lieu de le décrire.**
  C'est la première extension de la palette depuis qu'un second thème existe, et c'est là
  que l'on voit si le moteur en est un : ajouter un rôle a demandé une valeur dans chacun
  des deux thèmes, et celle du thème compilé est venue d'une clef du skin que la table
  d'`xtask` désigne — pas d'une valeur écrite à la main dans l'artefact, que rien n'aurait
  rattrapée. `mc` nomme les quatre ; Turbo Vision n'en nomme que trois, et le quatrième est
  composé.
- **Un piège de vocabulaire, qui aurait inversé deux couleurs en silence.** Borland appelle
  *Focused* l'item sous le curseur et *Selected* un item marqué. air-tui emploie `Selected`
  pour ce qui est sous le curseur, depuis `MenuSelected` au jalon M5. La transcription
  croise donc les deux noms, et c'est écrit dans l'en-tête de la palette : `ListSelected`
  vient de `cpListViewer[3]`, `ListMarked` de `cpListViewer[4]`.
- **Le décompte de `cpAppColor` a été validé par une source indépendante du calcul.**
  Suivre `cpListViewer` → `cpGrayDialog` → `cpAppColor` donne les indices 57 à 60 ; or
  `examples/tvdemo/tvdemo2.cpp` nomme lui-même ses entrées, et il donne « List normal » à
  57, « List focused » à 58, « List selected » à 59. Les six entrées des boutons,
  transcrites à M5, portent les mêmes numéros que ce fichier leur attribue. Deux chemins
  qui tombent sur le même résultat valent mieux qu'un décompte soigneux.
- **Trois défauts trouvés par les essais, aucun par la relecture.**
  - Le marquage ne faisait pas avancer la génération du modèle : la marque était dans le
    modèle, le titre du panneau la comptait, et la liste ne la montrait pas. Le contrôleur
    ne repoussait pas un reflet qu'il croyait à jour.
  - Poser le curseur sur le répertoire d'où l'on remonte ne peut pas se faire au moment de
    la commande : à cet instant la liste reflète encore le répertoire précédent, et le nom
    cherché n'y est pas. C'est devenu une **intention**, que l'étape 3 applique après avoir
    poussé le reflet — exactement la forme que l'ADR-007 prescrit.
  - Le curseur d'ascenseur empruntait `close_box_mark`. Les deux glyphes coïncident dans
    Turbo Vision — c'est le même `■` —, et le skin de `mc` ferme ses fenêtres par un `X` :
    la glissière portait donc un `X` en plein milieu. **Un thème unique ne permettait pas de
    le voir.** `AirGlyphSet` a gagné `scroll_thumb`, et un essai refuse désormais que les
    deux glyphes soient égaux dans le thème de `mc`.
- **Un défaut découvert et laissé ouvert, écrit comme tel.** Le protocole clavier Kitty
  rapporte les relâchements de touche — `\x1b[>23u` demande le type d'événement —, et
  **aucun widget du dépôt ne les écartait**. `AirListBox` est le premier à le faire, parce
  que sans ce filtre une flèche descendrait de deux items. Les autres widgets restent
  exposés, et aucun essai ne peut le montrer : un pty nu ne négocie pas le protocole, donc
  il faut un terminal Kitty réel. Le bon correctif est probablement central — dans la
  distribution plutôt que dans chaque widget —, ce qui en fait une décision et non une
  retouche.
- **Le compte des skins importables a baissé, et c'est le prix de l'exigence.** Dix sur
  trente-neuf, contre douze à M7a. Les deux perdus posent `bold` sur `[core] marked`, une
  clef que la table n'employait pas avant les rôles de liste : le modèle s'est enrichi, donc
  il a une clef de plus à savoir exprimer. Les deux limites restent les mêmes — pas
  d'attribut par rôle, pas de couleur « défaut du terminal » — et elles sont chiffrées, donc
  décidables.
- **Ce que `rmc` ne fait pas est dans son mode d'emploi, et un essai le vérifie.** Il
  n'exécute aucun programme et ne modifie aucun fichier. Sa barre de touches ne publie que
  `Tab`, `F5` et `F10` : `mc` en affiche dix, et en annoncer dix dont sept ne feraient rien
  serait exactement ce que le NO GO n° 2 interdit.
- **Vérifié sur les octets.** `rmc` sort sur `F10` (`exit:0`) et tient sans elle
  (`timeout`) ; `Tab`, flèche bas puis `Inser` donnent `38;5;11;48;5;6`, jaune sur cyan,
  l'item marqué sous le curseur du skin de `mc`, et le titre affiche « 1 marqué ». `redit`
  et l'exemple `frame` passent les mêmes essais qu'avant le changement de glyphe. 1054
  essais, sept codecs toujours à 100 %, 8433 noms inspectés dans 72 fichiers sans
  abréviation.

## 2026-10-10 — Jalon M7a : le thème Midnight Commander, compilé et non recopié

- **Le moteur de thèmes a cessé d'être un moteur sur parole.** Il avait un thème depuis
  M5, ce qui ne prouvait rien : une palette unique peut se cacher n'importe où. Il en a
  deux, et le second est **compilé depuis une source extérieure** par
  `cargo xtask import-mc-skin`. Les deux lancements de l'exemple dessinent exactement les
  mêmes vues ; seules la palette et les glyphes changent.
- **L'ADR-008 a tranché une question que j'avais mal posée.** J'avais annoncé l'importeur
  comme un codec à l'exécution. L'ADR dit l'inverse, noir sur blanc : « les thèmes sont
  compilés, pas analysés au runtime », et un skin mal formé doit être un refus à la
  compilation. L'importeur est donc un `xtask`, et `air-tui` n'embarque aucun analyseur de
  `.ini` — ce qui est vérifiable, pas promis.
- **La licence a été tranchée par l'auteur du projet, pas par moi.** Les valeurs viennent
  du skin par défaut de `mc`, sous GPLv3, dans un dépôt MPL-2.0 : ce n'est pas le même
  geste que la transcription de Borland, dont Turbo Vision avait été versée au domaine
  public. Décision retenue : artefact généré committé, et une entrée dans `NOTICE` qui
  nomme le projet, sa licence et ce qui en est repris — vingt-huit couples de couleurs et
  douze caractères de cadre, c'est-à-dire de la donnée de configuration, aucune ligne de
  code. `NOTICE` n'affirme plus qu'Unicode est la seule donnée tierce du dépôt.
- **Le manuel de `mc` se trompe sur deux points, et les fichiers font foi.** Il nomme les
  jonctions `centertop`, `centerbottom` et `centermiddle` ; les trente-neuf skins livrés
  écrivent `topmiddle`, `bottommiddle` et `cross`, et **aucun** n'emploie les noms du
  manuel. Il annonce `color16` à `color255` ; les skins emploient aussi `color0` à
  `color15`, plus de deux mille fois. Écrire l'analyseur d'après le manuel aurait donné un
  outil conforme à la documentation et faux sur les données.
- **Un défaut de mon analyseur trouvé par la mesure, et invisible à la relecture.** Je
  découpais les valeurs sur `#` pour retirer les commentaires de fin de ligne. Or les
  trente-neuf skins portent cent quatre-vingt-douze valeurs contenant un `#`, et les cent
  quatre-vingt-douze sont des **vraies couleurs** : aucun skin n'emploie de commentaire en
  fin de ligne. Quatre skins se vidaient donc de leurs couleurs, en annonçant « «  » n'est
  pas une couleur connue de mc ». Ce n'est pas la relecture du code qui l'a vu, c'est de
  l'avoir lancé sur les trente-neuf fichiers.
- **Douze skins sur trente-neuf sont importables, et les vingt-sept refus sont chiffrés.**
  Dix-huit posent un attribut — `bold`, `underline` — sur un rôle employé, et
  `AirColorPair` ne porte que deux couleurs ; neuf emploient `default`, le fond transparent
  de `mc`, qu'`AirColor` ne sait pas exprimer. Les deux sont des limites du modèle
  d'air-tui, pas des défauts de l'analyseur, et les lever rendrait les vingt-sept
  importables. Un refus chiffré est une décision possible ; un refus silencieux est une
  surprise.
- **Le décodeur de skins est à 100 % des trois mesures, et la règle a fait son travail.**
  ADR-012 promet qu'exiger 100 % change l'écriture plutôt que d'allonger les essais : c'est
  exactement ce qui s'est produit. Les branches « ce chiffre manque » et « index hors
  palette » ont été **supprimées** — motifs de tranche pour les trois et six chiffres d'une
  couleur, table hexadécimale au lieu d'un `to_digit` suivi d'une conversion infaillible.
  Le fichier est plus simple qu'avant la règle, pas plus long. Septième codec au gate.
- **Vingt-cinq rôles transcrits, trois composés, quatorze annotés.** `mc` nomme ses
  couleurs par élément d'interface, air-tui par rôle sémantique, et les deux ensembles ne
  se recouvrent pas : `mc` ne connaît ni bouton par défaut, ni ascenseur, ni item de menu à
  la fois sélectionné et interdit. Chaque choix est écrit dans l'artefact, à côté de la
  valeur. Une dérivation nommée se discute ; une dérivation silencieuse se découvre des
  mois plus tard devant un écran qui ne ressemble pas au skin.
- **Le thème se vérifie sur les octets, pas à l'œil.** Dans un pseudo-terminal, la barre de
  menu passe de `38;5;0;48;5;7` à `38;5;15;48;5;6`, la barre de statut de `48;5;7` à
  `48;5;6`, et le bureau du `░` à l'espace — ce que `default.ini` dit mot pour mot. Les
  quatre essais passent, essais de contrôle compris.
- **Deux faits que les deux thèmes partagent, et qui ne sont pas des défauts** : le fond du
  bureau est gris clair sur bleu dans les deux — `mc` a repris le fond de Norton Commander,
  que Turbo Vision employait déjà —, et le skin par défaut de `mc` dessine ses dialogues en
  trait **simple**, là où Turbo Vision met son `╔═╗`. Les deux ont leur essai.
- **Le gate des versions a attrapé sa deuxième vraie infraction**, dans la même séance où
  il a attrapé la première : `redit` déclarait `air-tui = "0.7.0"` alors que la crate
  passait en 0.7.1. Cargo l'accepte sans un mot, `^0.7.0` étant satisfait ; c'est la
  publication qui aurait livré une borne fausse.
- **Ce que ce jalon ne fait pas, et qui est écrit plutôt que sous-entendu** : `AirDecorations`
  n'existe pas, donc `AirTheme` est un **couple** là où l'ADR-008 décrit un triplet. Le
  style de trait vit déjà dans les glyphes, mais l'ombre portée ne peut pas être dessinée —
  elle tombe hors des limites de la vue qui la projette, et le découpage du peintre ne
  souffre aucune exception. `AirRole::ButtonShadow` est dans la palette depuis M5, transcrit
  de Borland comme de `mc`, et aucun widget ne le lit. Une assiette qui décrirait une ombre
  que rien ne dessine annoncerait une capacité absente : la trancher demande de décider si
  une vue peut peindre hors de ses limites, ce qui est une décision d'architecture.
- Et `rmc` n'existe pas. L'ADR-008 prévient que « ce qui fait reconnaître Midnight
  Commander est sa coquille, pas sa palette » : `redit --theme mc` donne un éditeur aux
  couleurs de `mc`, pas `mc`. Le thème seul ne contredit pas cet avertissement, il le
  confirme.
- Versions : `air-tui` 0.7.0 → **0.7.1**, `redit` 0.1.0 → **0.1.1**, `xtask` 0.2.1 →
  **0.2.2** — trois ajouts d'API, donc `z` (en `0.y.z`, seule une rupture avance `y`).
  973 essais au vert, dont 79 nouveaux.

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
- **`main` exige désormais les trois contrôles**, `enforce_admins` compris : la règle vaut
  pour l'auteur du dépôt comme pour un contributeur. Une CI qui informe sans rien empêcher
  n'aurait fait que déplacer le problème. `strict` reste à `false` : une branche n'a pas à
  être rebasée sur `main` pour être fusionnée, seulement à voir ses contrôles passer — et
  l'exiger rendrait une chaîne de PR empilées impraticable, ce qu'elle a déjà été au jalon
  M6.
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
