// Port de app/src/ui/core/editor/rangeHighlights.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  Les SURLIGNAGES de passages — un seul chemin pour toute surface de texte
//  (note, PDF, toute `TextSurface` tierce), posé AU-DESSUS du contrat : aucune
//  implémentation n'a à le coder.
//
//    highlights.set("doc-widget:42", vec![HighlightRange::with_cls(from, to, "is-hover")])
//
//  ★ UNE table (et UN calque) par surface, long-vivante : sur une note c'est un
//    vrai `layer()` de CM6, dessiné SOUS le texte comme la sélection native, sans
//    toucher au DOM des lignes. Sur CM, ajouter / retirer un calque reconfigure
//    tous les autres : on ne crée donc JAMAIS de calque par survol — on change
//    la table et on « poke » (`request_update`).
//  ★ La forme est celle de `coords_for_range` : sur une note, exactement la forme
//    d'une sélection (1re ligne partielle, lignes pleines, dernière partielle) ;
//    sur un PDF, les spans de la couche de texte.
//  ★ Les plages SUIVENT le texte : `from` colle au caractère qui suit, `to` à
//    celui qui précède ; une plage vidée par une suppression disparaît.
//
//  Ce fichier porte la TABLE et sa géométrie (les boîtes), pas le calque : en TS
//  la table s'attache à la surface par une WeakMap et s'abonne elle-même
//  (`addLayer`, `onChange`, microtâche) ; en Rust la SURFACE possède sa table,
//  lui transmet ses changements (`remap`) et vide le poke (`take_redraw`) quand
//  elle peut redessiner — jamais pendant sa propre mise à jour.
// ═══════════════════════════════════════════════════════════════════════════

use crate::editor::{Assoc, EditorChange, MappedPos, TextSurface, map_range};
use crate::visibility::{PosVisibility, pos_visibility};

/// Une plage surlignée.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct HighlightRange {
    pub from: usize,
    pub to: usize,
    /// Classe(s) en plus de `range-highlight` (état, style d'un plugin).
    pub cls: Option<String>,
}

impl HighlightRange {
    pub fn new(from: usize, to: usize) -> Self {
        Self {
            from,
            to,
            cls: None,
        }
    }

    pub fn with_cls(from: usize, to: usize, cls: &str) -> Self {
        Self {
            from,
            to,
            cls: Some(cls.to_owned()),
        }
    }
}

/// Une boîte à dessiner (TS : `HighlightMarker`, dont `eq` comparait classe et
/// géométrie — ici `PartialEq`). `cls` : `range-highlight`, suivi de la classe de la plage.
#[derive(Clone, Debug, PartialEq)]
pub struct HighlightBox {
    pub cls: String,
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

/// Le retrait d'une pose (TS : la fonction rendue par `set`) : sans effet si la
/// clef a été reposée depuis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HighlightHandle {
    key: String,
    generation: u64,
}

/// Une clef et ses plages (TS : une entrée de la `Map`, dans l'ordre de pose).
#[derive(Debug)]
struct Entree {
    key: String,
    generation: u64,
    ranges: Vec<HighlightRange>,
}

/// La table des surlignages d'UNE surface.
#[derive(Debug, Default)]
pub struct RangeHighlights {
    /// Dans l'ordre de PREMIÈRE pose (une clef reposée garde sa place, comme une `Map`) :
    /// c'est l'ordre de dessin.
    plages: Vec<Entree>,
    generation: u64,
    /// TS : `arme` — le calque est posé au premier surlignage, puis gardé.
    arme: bool,
    /// TS : `poke` — un redessin est demandé et pas encore livré.
    poke: bool,
}

impl RangeHighlights {
    pub fn new() -> Self {
        Self::default()
    }

    /// Le calque des surlignages est dessiné SOUS le texte, comme la sélection
    /// native (TS : `addLayer({ above: false })`) — le `above` de son calque.
    pub fn layer_above(&self) -> bool {
        false
    }

    /// Pose (ou remplace) les plages d'une clef. Rend le retrait.
    pub fn set(&mut self, key: &str, ranges: Vec<HighlightRange>) -> HighlightHandle {
        let valides: Vec<HighlightRange> = ranges.into_iter().filter(|r| r.to > r.from).collect();
        if valides.is_empty() {
            self.delete(key);
            return HighlightHandle {
                key: key.to_owned(),
                generation: 0,
            };
        }
        self.arme = true;
        self.generation += 1;
        let generation = self.generation;
        match self.plages.iter_mut().find(|e| e.key == key) {
            Some(e) => {
                e.generation = generation;
                e.ranges = valides;
            }
            None => self.plages.push(Entree {
                key: key.to_owned(),
                generation,
                ranges: valides,
            }),
        }
        self.redessiner();
        HighlightHandle {
            key: key.to_owned(),
            generation,
        }
    }

    /// Retire une pose — seulement si c'est encore ELLE qui est en place.
    pub fn retire(&mut self, handle: &HighlightHandle) {
        if self
            .plages
            .iter()
            .any(|e| e.key == handle.key && e.generation == handle.generation)
        {
            self.delete(&handle.key);
        }
    }

    pub fn delete(&mut self, key: &str) {
        let avant = self.plages.len();
        self.plages.retain(|e| e.key != key);
        if self.plages.len() != avant {
            self.redessiner();
        }
    }

    /// Les plages COURANTES d'une clef (remappées par les éditions).
    pub fn get(&self, key: &str) -> &[HighlightRange] {
        self.plages
            .iter()
            .find(|e| e.key == key)
            .map_or(&[], |e| e.ranges.as_slice())
    }

    /// Le calque a-t-il été demandé (premier surlignage posé) ?
    pub fn is_armed(&self) -> bool {
        self.arme
    }

    /// Un redessin est-il en attente ? Le lire le CONSOMME : la surface appelle
    /// alors `request_update` — un seul poke pour toutes les poses regroupées
    /// (TS : la microtâche).
    pub fn take_redraw(&mut self) -> bool {
        std::mem::replace(&mut self.poke, false)
    }

    /// Pas pendant une mise à jour de l'éditeur (un dispatch y est interdit) : différé.
    fn redessiner(&mut self) {
        self.poke = true;
    }

    /// Un changement de la surface : les plages suivent le texte.
    pub fn remap(&mut self, change: &EditorChange) {
        if change.doc_changed && !self.plages.is_empty() {
            self.remapper(&*change.map_pos);
        }
    }

    /// ★ Puis un poke : le calque se recalcule bien sur `doc_changed`, mais sa mesure
    ///   a pu être FORCÉE avant nous dans la même mise à jour (un autre abonné qui
    ///   lit `coords_at_pos` — WidgetLayer) — avec les plages d'avant. Mesuré.
    pub fn remapper(&mut self, map_pos: &dyn Fn(usize, Assoc) -> MappedPos) {
        for e in &mut self.plages {
            e.ranges = e
                .ranges
                .iter()
                .map(|r| {
                    let m = map_range(map_pos, r.from, r.to);
                    HighlightRange {
                        from: m.from,
                        to: m.to,
                        cls: r.cls.clone(),
                    }
                })
                .filter(|r| r.to > r.from)
                .collect();
        }
        self.plages.retain(|e| !e.ranges.is_empty());
        self.redessiner();
    }

    /// Les boîtes à dessiner (TS : `markers`), dans le repère document de la surface.
    pub fn boxes(&self, s: &dyn TextSurface) -> Vec<HighlightBox> {
        let mut out = Vec::new();
        for r in self.plages.iter().flat_map(|e| e.ranges.iter()) {
            // Ni au début ni à la fin dans le rendu (replié, page libérée) : rien.
            if pos_visibility(s, r.from) != PosVisibility::Rendered
                && pos_visibility(s, r.to) != PosVisibility::Rendered
            {
                continue;
            }
            let cls = match &r.cls {
                Some(c) if !c.is_empty() => format!("range-highlight {c}"),
                _ => "range-highlight".to_owned(),
            };
            for q in s.coords_for_range(r.from, r.to) {
                if q.right <= q.left || q.bottom <= q.top {
                    continue;
                }
                out.push(HighlightBox {
                    cls: cls.clone(),
                    left: q.left,
                    top: q.top,
                    width: q.right - q.left,
                    height: q.bottom - q.top,
                });
            }
        }
        out
    }
}
