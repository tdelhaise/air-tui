# ADR-013 — Accès typé dans l'arbre de vues : `AirView` hérite de `Any`

**Statut :** Accepté — 2026-10-10

## Contexte

L'arbre de vues tient des `Box<dyn AirView>` (ADR-011). C'est ce qui lui permet de ne
porter aucun paramètre de type et d'accueillir les vues d'une application à côté de celles
du framework.

Le jalon M6 a montré le prix exact de cet effacement. Le contrôleur MVC de l'ADR-007 doit,
à chaque tour de boucle :

- appeler `AirEditorView::synchronize(&buffer)` sur **sa** vue d'édition ;
- lui prendre ses intentions par `drain_actions` ;
- donner leur plage aux deux `AirScrollBar`, et relever leurs demandes.

Aucune de ces méthodes n'est dans `AirView`, et elles n'y ont rien à faire : ce sont les
méthodes de trois widgets précis. Une fois la vue posée dans l'arbre, plus personne ne
pouvait les appeler. Le contrôleur était aveugle à ses propres vues.

## Décision

**`AirView` hérite de `Any`**, et deux fonctions libres rendent une vue avec son type
d'origine :

```rust
pub trait AirView: Any { … }

pub fn find<ViewType: AirView>(view: &dyn AirView, identifier: AirViewId)
    -> Option<&ViewType>;
pub fn find_mut<ViewType: AirView>(view: &mut dyn AirView, identifier: AirViewId)
    -> Option<&mut ViewType>;
```

La recherche est en profondeur, par `AirViewId` — c'est le mécanisme 4 de l'ADR-007,
« abonnement par identifiant » : le contrôleur garde des identifiants, jamais de pointeur
arrière.

Un identifiant absent **ou** un type qui ne correspond pas rendent `None`. Se tromper de
type est une erreur de programme qui mérite une absence, pas une panique : un contrôleur
qui cherche son éditeur et trouve un bouton doit pouvoir le constater.

Le lien d'héritage impose `'static` aux implémentations. Elles le sont toutes : une vue ne
contient pas d'emprunt, et le peintre qu'elle reçoit lui est passé en paramètre.

## Conséquences

- Aucune ligne de plus dans les vues : `Any` est implémenté automatiquement pour tout
  `T: 'static`, et la conversion `dyn AirView` → `dyn Any` est un transtypage ascendant de
  trait, stable depuis Rust 1.86.
- À l'exécution, une comparaison d'identifiant de type. Le parcours de l'arbre, lui, est en
  O(nombre de nœuds) — une poignée de dizaines dans une application, sans commune mesure
  avec le rendu.
- `find` et `find_mut` ne remplacent pas le routage : un événement continue de descendre
  par les trois phases, et aucune vue ne connaît une autre vue. Seul le **contrôleur** s'en
  sert, et seulement sur des vues qu'il a lui-même posées.

## Alternatives écartées

**Un paramètre de type sur les conteneurs** — `AirScreen<Desktop>`, `AirDesktop<Window>`.
Typé sans transtypage, et c'est sa seule qualité : le paramètre remonte jusqu'à
`AirApplication`, interdit de mêler deux types de fenêtres dans un même bureau, et rend
impossible une fenêtre choisie à l'exécution. Le patron Composite perd ce qui le rend
utile.

**Une méthode par type de vue dans le trait** — `fn as_editor_mut(&mut self) -> Option<&mut AirEditorView>`.
Ferait du cœur le catalogue de ses propres widgets, et une application ne pourrait pas
retrouver **ses** vues sans modifier le framework.

**Un registre d'identifiants chez le contrôleur, tenant les vues hors de l'arbre.** Le
contrôleur garderait les vues typées et l'arbre des coquilles qui les dessinent. Il faudrait
alors deux sources de vérité pour les limites, le focus et la visibilité, et le découpage
réel du peintre ne s'appliquerait plus — c'est le défaut du jalon M0, réintroduit.

**Des canaux `mpsc` entre le contrôleur et ses vues.** Déjà écarté par l'ADR-007 pour les
événements d'interface, et pour la même raison : une file à drainer au bon moment, et
l'ordre relatif au rendu perdu.
