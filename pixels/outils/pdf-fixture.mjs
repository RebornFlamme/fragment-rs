// ═══════════════════════════════════════════════════════════════════════════
//  Générateur des PDF de fixture des scènes pixels de T4 (fr-t4-scenes).
//
//  Écrit par NOUS, textes à nous : aucun PDF ni aucun texte de Fragment (ADR-007). Seul le
//  principe (un fichier PDF minimal fabriqué à la volée) est le même. Aucune dépendance :
//  Node seul, exécuté avec le Node de Fragment, sans rien écrire dans son arbre.
//
//      node pixels/outils/pdf-fixture.mjs        → écrit pixels/vault-t4/cours.pdf et memo.pdf
//
//  ★ Déterministe : ni date, ni identifiant aléatoire, ni /Info daté ; deux exécutions rendent
//    les MÊMES octets (la référence et le candidat ouvrent le même fichier commité).
//  ★ La table xref se CALCULE (offsets d'octets) : un caractère de plus décale tout ce qui suit.
//  ★ Police Helvetica standard (non incorporée) en WinAnsi : les accents français passent en
//    latin1. Le rendu de l'intérieur des pages relève d'une catégorie propre (raster, S7) ;
//    le chrome de la vue reste strict.
// ═══════════════════════════════════════════════════════════════════════════
import { mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ICI = path.dirname(fileURLToPath(import.meta.url));
const SORTIE = path.resolve(ICI, '..', 'vault-t4');

const echapper = (t) => t.replace(/([\\()])/g, '\\$1');

/** Coupe un paragraphe en lignes d'au plus `max` caractères (largeur moyenne approchée). */
function plier(texte, max) {
    const lignes = [];
    let cour = '';
    for (const mot of texte.split(' ')) {
        if (cour && (cour + ' ' + mot).length > max) { lignes.push(cour); cour = mot; }
        else cour = cour ? cour + ' ' + mot : mot;
    }
    if (cour) lignes.push(cour);
    return lignes;
}

const MARGE = 40;
const lignesDe = (p) => {
    const max = Math.max(12, Math.floor((p.largeur - 2 * MARGE) / 6.2));
    return p.paragraphes.flatMap((par, k) => [...plier(par, max), ...(k < p.paragraphes.length - 1 ? [''] : [])]);
};

/**
 * Fabrique un PDF.
 * @param {object} o
 * @param {{titre:string, paragraphes:string[], largeur:number, hauteur:number}[]} o.pages
 * @param {{titre:string, page:number, enfants?:any[]}[]} [o.outline]  pages 1-based
 * @param {{page:number, versPage?:number, uri?:string, ligne:number}[]} [o.liens]  `ligne` = n° (0-based) de ligne de corps
 * @returns {Buffer}
 */
export function fabriquerPdf({ pages, outline = [], liens = [] }) {
    // Numérotation : 1 catalogue, 2 arbre des pages, 3-4 polices, puis 2 objets par page.
    const idPage = (i) => 5 + i * 2;
    const idContenu = (i) => 6 + i * 2;
    const idOutlines = 5 + pages.length * 2;
    const total = (l) => l.reduce((n, e) => n + 1 + total(e.enfants ?? []), 0);
    const nOutline = outline.length ? 1 + total(outline) : 0;
    const idLien = (k) => idOutlines + nOutline + k;
    const idInfo = idOutlines + nOutline + liens.length;

    const objets = [];
    objets.push(outline.length
        ? `<< /Type /Catalog /Pages 2 0 R /Outlines ${idOutlines} 0 R >>`
        : '<< /Type /Catalog /Pages 2 0 R >>');
    objets.push(`<< /Type /Pages /Kids [ ${pages.map((_, i) => `${idPage(i)} 0 R`).join(' ')} ] /Count ${pages.length} >>`);
    objets.push('<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>');
    objets.push('<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>');

    pages.forEach((p, i) => {
        const ids = liens.flatMap((l, k) => (l.page === i + 1 ? [`${idLien(k)} 0 R`] : []));
        const annots = ids.length ? ` /Annots [ ${ids.join(' ')} ]` : '';
        objets.push(`<< /Type /Page /Parent 2 0 R /MediaBox [ 0 0 ${p.largeur} ${p.hauteur} ] `
            + `/Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents ${idContenu(i)} 0 R${annots} >>`);
        const flux = [
            `BT /F2 15 Tf ${MARGE} ${p.hauteur - 70} Td (${echapper(p.titre)}) Tj ET`,
            `0.6 w ${MARGE} ${p.hauteur - 82} m ${p.largeur - MARGE} ${p.hauteur - 82} l S`,
            `BT /F1 12 Tf 16 TL ${MARGE} ${p.hauteur - 112} Td`,
            ...lignesDe(p).map((l, k) => `${k === 0 ? '' : 'T* '}(${echapper(l)}) Tj`),
            'ET',
            `BT /F1 9 Tf ${p.largeur / 2 - 6} 24 Td (${i + 1}) Tj ET`,
        ].join('\n');
        objets.push(`<< /Length ${Buffer.byteLength(flux, 'latin1')} >>\nstream\n${flux}\nendstream`);
    });

    if (outline.length) {
        const entrees = [];
        let prochain = idOutlines + 1;
        const ecrire = (liste, parent) => {
            const ids = liste.map(() => prochain++);
            const enfantsIds = liste.map((e, i) => ecrire(e.enfants ?? [], ids[i]));
            liste.forEach((e, i) => {
                const liensE = [
                    `/Parent ${parent} 0 R`,
                    i > 0 ? `/Prev ${ids[i - 1]} 0 R` : '',
                    i < liste.length - 1 ? `/Next ${ids[i + 1]} 0 R` : '',
                    enfantsIds[i].length ? `/First ${enfantsIds[i][0]} 0 R /Last ${enfantsIds[i].at(-1)} 0 R /Count -${enfantsIds[i].length}` : '',
                ].filter(Boolean).join(' ');
                entrees[ids[i] - idOutlines - 1] =
                    `<< /Title (${echapper(e.titre)}) ${liensE} /Dest [ ${idPage(e.page - 1)} 0 R /XYZ 0 ${pages[e.page - 1].hauteur} null ] >>`;
            });
            return ids;
        };
        const racine = ecrire(outline, idOutlines);
        objets.push(`<< /Type /Outlines /First ${racine[0]} 0 R /Last ${racine.at(-1)} 0 R /Count ${total(outline)} >>`);
        objets.push(...entrees);
    }

    for (const l of liens) {
        const p = pages[l.page - 1];
        const basse = p.hauteur - 112 - l.ligne * 16 - 3;
        const cible = l.uri
            ? `/A << /S /URI /URI (${echapper(l.uri)}) >>`
            : `/Dest [ ${idPage(l.versPage - 1)} 0 R /XYZ 0 ${pages[l.versPage - 1].hauteur} null ]`;
        objets.push(`<< /Type /Annot /Subtype /Link /Rect [ ${MARGE} ${basse} ${p.largeur - MARGE} ${basse + 16} ] /Border [ 0 0 0 ] ${cible} >>`);
    }

    objets.push(`<< /Title (${echapper(pages[0].titre)}) /Producer (fragment-rs pixels) >>`);

    let corps = '%PDF-1.4\n';
    const offsets = [];
    objets.forEach((o, i) => {
        offsets.push(Buffer.byteLength(corps, 'latin1'));
        corps += `${i + 1} 0 obj\n${o}\nendobj\n`;
    });
    const debut = Buffer.byteLength(corps, 'latin1');
    let xref = `xref\n0 ${objets.length + 1}\n0000000000 65535 f \n`;
    for (const o of offsets) xref += `${String(o).padStart(10, '0')} 00000 n \n`;
    const fin = `trailer\n<< /Size ${objets.length + 1} /Root 1 0 R /Info ${idInfo} 0 R >>\nstartxref\n${debut}\n%%EOF\n`;
    return Buffer.from(corps + xref + fin, 'latin1');
}

// ── Le contenu : notes d'un cours imaginaire sur un phare ───────────────────────────────────────

const PORTRAIT = { largeur: 300, hauteur: 420 };
/** Tailles inégales : la 2e plus large, la 4e et la 11e en paysage, la 8e en grand format. */
const TAILLES = { 2: { largeur: 340, hauteur: 440 }, 4: { largeur: 420, hauteur: 300 }, 8: { largeur: 360, hauteur: 520 }, 11: { largeur: 420, hauteur: 300 } };

const TITRES = [
    'Le phare de la Dent-Creuse', 'Un peu d\x27histoire', 'La lumière et ses secrets', 'Plan des lieux',
    'La tour', 'Les gardiens', 'Le matériel', 'Carte des écueils', 'Les saisons', 'Les tempêtes',
    'Tableau des feux', 'Visites guidées', 'Le sentier côtier', 'Faune et flore', 'Les épaves',
    'Photographier le phare', 'Légendes locales', 'Entretien de la peinture', 'Questions fréquentes', 'Pour aller plus loin',
];
const POOL = [
    'La tour se dresse sur un rocher que la mer recouvre deux fois par jour.',
    'Les gardiens montaient chaque soir quatre-vingt-dix marches pour allumer la lampe.',
    'Un registre relié de toile consignait la météo, l\x27heure et la visibilité.',
    'Le brouillard cache parfois la lanterne pendant plusieurs jours de suite.',
    'On y accède à pied par la digue, seulement à marée basse.',
    'La peinture blanche est refaite tous les cinq ans, avec une bande rouge au sommet.',
    'Les cormorans nichent sur la corniche nord et reviennent chaque printemps.',
    'Le vent d\x27ouest dépasse souvent cent kilomètres par heure en hiver.',
    'Une cloche de brume sonnait toutes les trente secondes quand l\x27air devenait épais.',
    'Le mécanisme d\x27horlogerie tournait grâce à un poids descendant le long de la tour.',
    'La visite dure environ quarante minutes et se termine par la lanterne.',
    'Un petit musée présente des cartes anciennes et des outils de forgeron.',
];
/** Le mot « balise » (scènes de recherche) : 2 fois page 1, puis pages 2, 7 et 15 — 5 occurrences. */
const SPECIAL = {
    1: ['Ces notes présentent le phare, son histoire et ses habitants.', 'La balise du large clignote toutes les douze secondes.', 'Chaque balise porte un numéro gravé sur sa base.'],
    2: ['On entretient une balise par temps calme, jamais pendant la tempête.'],
    7: ['Une balise de secours reste sur la jetée, au cas où la lampe principale tomberait en panne.'],
    15: ['La dernière balise a été remplacée l\x27an passé.'],
};

function pagesCours() {
    return TITRES.map((titre, i) => {
        const n = i + 1;
        const paragraphes = [...(SPECIAL[n] ?? [])];
        for (let k = 0; paragraphes.length < 3; k++) paragraphes.push(POOL[(n * 5 + k * 3) % POOL.length]);
        return { titre, paragraphes, ...(TAILLES[n] ?? PORTRAIT) };
    });
}

const OUTLINE = [
    { titre: 'Introduction', page: 1 },
    { titre: 'Chapitre 1 : le site', page: 3, enfants: [{ titre: '1.1 La tour', page: 5 }, { titre: '1.2 Les gardiens', page: 6 }, { titre: '1.3 Le matériel', page: 7 }] },
    { titre: 'Chapitre 2 : la mer', page: 9, enfants: [{ titre: '2.1 Les saisons', page: 9 }, { titre: '2.2 Les tempêtes', page: 10 }] },
    { titre: 'Chapitre 3 : visiter', page: 12 },
    { titre: 'Conclusion', page: 20 },
];

export function ecrireFixtures(dossier = SORTIE) {
    mkdirSync(dossier, { recursive: true });
    const cours = pagesCours();
    const nbBalise = cours.flatMap((p) => p.paragraphes).join(' ').match(/balise/gi)?.length ?? 0;
    if (nbBalise !== 5) throw new Error(`« balise » doit apparaître 5 fois, ${nbBalise} trouvées`);
    // Deux liens : un interne (page 1, vers la page 9) et un externe (page 2). `ligne` = ligne de corps.
    writeFileSync(path.join(dossier, 'cours.pdf'), fabriquerPdf({
        pages: cours,
        outline: OUTLINE,
        liens: [{ page: 1, versPage: 9, ligne: 4 }, { page: 2, uri: 'https://example.org/phare', ligne: 2 }],
    }));
    // Le second PDF : trois pages, SANS table des matières.
    writeFileSync(path.join(dossier, 'memo.pdf'), fabriquerPdf({
        pages: [
            { titre: 'Mémo de passation', paragraphes: ['Les clés sont dans le tiroir du bureau, sous le registre.', 'Penser à relever le baromètre avant la relève.'], ...PORTRAIT },
            { titre: 'Contacts', paragraphes: ['Le capitaine du port répond de sept heures à dix-neuf heures.'], ...PORTRAIT },
            { titre: 'Liste de contrôle', paragraphes: ['Lampe, générateur, cloche de brume, jumelles.'], ...PORTRAIT },
        ],
    }));
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) ecrireFixtures();
