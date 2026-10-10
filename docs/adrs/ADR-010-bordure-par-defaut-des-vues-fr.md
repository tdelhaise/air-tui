# ADR-010 — Bordure par défaut sur les vues

**Statut :** Accepté — 2026-10-10

## Contexte

Turbo Vision ne met pas de cadre sur une vue. Son `TView` n'en a aucun ; c'est `TWindow`
qui insère un `TFrame` comme sous-vue, et une vue ordinaire se dessine bord à bord. Le
cadre est donc, chez lui, un objet qu'on ajoute.

L'auteur retient l'inverse pour air-tui.

## Décision

**Toute `AirView` a une bordure par défaut.** Le développeur la retire explicitement en
posant `border = false` sur la vue, et rien n'est alors dessiné sur son pourtour.

```rust
pub struct AirViewState {
    bounds: AirRectangle,
    /// Vrai par défaut. À `false`, aucun pourtour n'est dessiné et la zone de contenu
    /// occupe la totalité des limites de la vue.
    border: bool,
    // …
}
```

Deux conséquences qui doivent être traitées dès l'écriture de l'arbre de vues (jalon M4),
pas découvertes ensuite :

**La zone de contenu n'est pas les limites de la vue.** Une vue bordée perd une cellule de
chaque côté. L'API doit donc exposer les deux, et le dessin d'un widget doit viser la
seconde :

```rust
impl AirViewState {
    /// Les limites extérieures de la vue, bordure comprise.
    pub fn bounds(&self) -> AirRectangle;
    /// La zone disponible pour le contenu : les limites, moins une cellule de chaque côté
    /// quand `border` est vrai.
    pub fn content_bounds(&self) -> AirRectangle;
}
```

**Les petits contrôles posent `border = false` dans leur constructeur.** Un `AirButton`,
un `AirLabel`, un `AirStaticText` ou un `AirCheckBoxes` encadrés ne ressemblent à rien, et
un bouton bordé sur une ligne de trois cellules de haut n'a plus de place pour son texte.
Le défaut profite aux conteneurs — `AirWindow`, `AirDialog`, `AirPanel`, `AirListBox` — et
les contrôles s'en dispensent explicitement. C'est à vérifier à la revue : un widget qui
laisse le défaut doit avoir une raison.

La bordure est **dessinée par le thème**, pas par la vue : les glyphes viennent de
`AirGlyphSet` et les couleurs d'un rôle de `AirPalette` (ADR-008). Une vue ne choisit pas
ses caractères de cadre, elle choisit d'en avoir un.

## Conséquences

- Un découpage par vue devient indispensable, et pas seulement souhaitable : sans lui, un
  contenu trop long écrase le bord droit de son propre cadre. Le défaut a été constaté sur
  l'exemple du jalon M0, où `draw_text` ne découpe que sur la surface.
- L'écart avec Turbo Vision est à documenter dans le guide de portage : un développeur qui
  vient de `TView` ne s'attend pas à une bordure.

## Alternatives écartées

**Suivre Turbo Vision — pas de bordure, cadre ajouté par le conteneur.** Fidèle, mais
verbeux au quotidien : la vue bordée est le cas courant dans une interface à fenêtres.

**Une bordure portée par un type `AirBorderedView` enveloppant.** Double la hiérarchie de
types pour un booléen, et complique chaque accès à l'enfant.
