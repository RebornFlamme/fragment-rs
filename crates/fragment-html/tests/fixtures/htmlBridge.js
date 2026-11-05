(() => {
    if (window.parent === window) return;
    const parent = window.parent;
    const envoyer = parent.postMessage.bind(parent);
    const lireSelection = document.getSelection.bind(document);
    const nouvelleRange = document.createRange.bind(document);
    const plusTard = window.setTimeout.bind(window);
    const image = window.requestAnimationFrame.bind(window);
    const post = (m) => envoyer(Object.assign({ fragmentHtml: 1 }, m), '*');

    // ── le thème de l'app : la page se fond dans Fragment ────────────────────
    // Le thème arrive AVEC la page (paramètre « fragment-theme » de son URL, posé par
    // la vue) : appliqué ici, avant la première image — aucun flash, rien de masqué.
    // Ensuite, le message « theme » le suit en direct (et apporte les polices). Les
    // défauts (:where : priorité nulle) habillent une page sans CSS — le sien gagne.
    const racine = document.documentElement;
    const policesPosees = new Set();
    const appliquerTheme = (m) => {
        const tete = document.head || racine;
        if (!tete) return;
        let feuille = document.getElementById('fragment-theme');
        if (!feuille) { feuille = document.createElement('style'); feuille.id = 'fragment-theme'; }
        if (tete.firstChild !== feuille) tete.insertBefore(feuille, tete.firstChild);
        const vars = m.vars && typeof m.vars === 'object' ? m.vars : {};
        const decl = Object.keys(vars).filter((k) => /^--[\w-]+$/.test(k)).map((k) => k + ':' + String(vars[k]).replace(/[;{}]/g, '') + ';').join('');
        const schema = m.scheme === 'light' ? 'light' : 'dark';
        feuille.textContent = ':root{' + decl + 'color-scheme:' + schema + ';}'
            + ':where(html){background:var(--background-primary);color:var(--text-normal);font-family:var(--font-text);font-size:var(--font-text-size);}';
        if (Array.isArray(m.fonts) && typeof FontFace !== 'undefined') {
            for (const f of m.fonts) {
                const cle = f.family + ':' + f.weight + ':' + f.style;
                if (policesPosees.has(cle)) continue;
                policesPosees.add(cle);
                try { const ff = new FontFace(f.family, f.data, { weight: String(f.weight), style: f.style || 'normal' }); document.fonts.add(ff); ff.load(); } catch (e) { /* une police illisible : la suivante */ }
            }
        }
    };
    try {
        const brut = new URLSearchParams(location.search).get('fragment-theme');
        if (brut) appliquerTheme(JSON.parse(brut));
    } catch (e) { /* pas de thème lisible : le message suivra */ }
    post({ type: 'hello' });

    // ── hauteur et mise en page ──────────────────────────────────────────────
    let derniere = -1;
    const hauteur = () => {
        const d = document.documentElement;
        const b = document.body;
        const h = Math.ceil(Math.max(d ? d.scrollHeight : 0, b ? b.scrollHeight : 0));
        if (h !== derniere) { derniere = h; post({ type: 'height', height: h }); }
    };
    let lv = 0;
    let layoutPrevu = false;
    const layout = () => {
        if (layoutPrevu) return;
        layoutPrevu = true;
        image(() => { layoutPrevu = false; lv++; post({ type: 'layout', lv }); });
    };

    // ── le texte aplati ──────────────────────────────────────────────────────
    const SAUTE = /^(SCRIPT|STYLE|NOSCRIPT|TEMPLATE|TITLE|TEXTAREA|SELECT|CANVAS|VIDEO|AUDIO|OBJECT|IFRAME)$/;
    const BLOC = /^(ADDRESS|ARTICLE|ASIDE|BLOCKQUOTE|BR|CAPTION|DD|DETAILS|DIALOG|DIV|DL|DT|FIELDSET|FIGCAPTION|FIGURE|FOOTER|FORM|H[1-6]|HEADER|HR|LI|MAIN|NAV|OL|P|PRE|SECTION|SUMMARY|TABLE|TBODY|TD|TFOOT|TH|THEAD|TR|UL)$/;
    let texte = null;
    let segs = [];
    let parNoeud = new Map();
    let tv = 0;
    let sale = true;
    const aplatir = () => {
        const morceaux = [];
        const s = [];
        let n = 0;
        let dernier = '\n';
        let virtuel = false;
        const coupe = () => { if (dernier !== '\n') { morceaux.push('\n'); n++; dernier = '\n'; virtuel = true; } };
        const visite = (el, pre) => {
            for (let c = el.firstChild; c; c = c.nextSibling) {
                if (c.nodeType === 3) {
                    const d = c.data;
                    // Les blancs de mise en forme de la source (retour + indentation) ne sont pas du texte.
                    if (!d || (!pre && !/\S/.test(d) && d.indexOf('\n') >= 0)) continue;
                    s.push({ node: c, start: n, len: d.length });
                    morceaux.push(d);
                    n += d.length;
                    dernier = d[d.length - 1];
                    virtuel = false;
                } else if (c.nodeType === 1) {
                    const tag = String(c.tagName).toUpperCase();
                    if (SAUTE.test(tag)) continue;
                    const bloc = BLOC.test(tag);
                    if (bloc) coupe();
                    visite(c, pre || tag === 'PRE');
                    if (bloc) coupe();
                }
            }
        };
        if (document.body) visite(document.body, false);
        // Le saut virtuel qui fermait le dernier bloc n'est pas du texte.
        if (virtuel) morceaux.pop();
        return { text: morceaux.join(''), segs: s };
    };
    const publierTexte = () => {
        sale = false;
        const a = aplatir();
        segs = a.segs;
        parNoeud = new Map();
        for (const sg of segs) parNoeud.set(sg.node, sg.start);
        if (a.text !== texte) { texte = a.text; tv++; post({ type: 'text', text: texte, tv }); }
    };
    let textePrevu = 0;
    const texteBientot = () => {
        sale = true;
        if (!textePrevu) textePrevu = plusTard(() => { textePrevu = 0; if (sale) publierTexte(); }, 250);
    };
    const aJour = () => { if (sale) publierTexte(); };

    // ── positions ↔ DOM ──────────────────────────────────────────────────────
    /** Le premier segment qui finit à l'index i ou après. */
    const premierSeg = (i) => {
        let lo = 0;
        let hi = segs.length;
        while (lo < hi) { const mid = (lo + hi) >> 1; if (segs[mid].start + segs[mid].len < i) lo = mid + 1; else hi = mid; }
        return lo;
    };
    /** Les rects d'une plage, TEXTE SEUL (jamais la boîte d'un élément) ; plage vide : le curseur. */
    const rectsDe = (from, to) => {
        const out = [];
        if (!(from >= 1) || !(to >= from)) return out;
        const r = nouvelleRange();
        const i0 = from - 1;
        const i1 = to - 1;
        const dx = window.scrollX;
        const dy = window.scrollY;
        const vide = from === to;
        for (let k = premierSeg(i0); k < segs.length; k++) {
            const sg = segs[k];
            if (sg.start > i1) break;
            if (!sg.node.isConnected) continue;
            const max = sg.node.data.length;
            const a = Math.min(max, Math.max(0, i0 - sg.start));
            const b = Math.min(max, Math.max(0, i1 - sg.start));
            if (!vide && b <= a) continue;
            try { r.setStart(sg.node, a); r.setEnd(sg.node, b); } catch (e) { continue; }
            let liste = r.getClientRects();
            if (vide && !liste.length && b < max) { r.setEnd(sg.node, b + 1); liste = r.getClientRects(); }
            for (const x of liste) {
                if (!vide && !x.width) continue;
                out.push({ left: x.left + dx, top: x.top + dy, right: (vide ? x.left : x.right) + dx, bottom: x.bottom + dy });
                if (vide) break;
            }
            if (vide) break;
        }
        return out;
    };
    const avant = (noeud, r) => { try { return r.comparePoint(noeud, 0) < 0; } catch (e) { return true; } };
    /** Un point du DOM (celui d'une sélection) → une position. */
    const posDe = (noeud, offset) => {
        const debut = parNoeud.get(noeud);
        if (debut !== undefined) return debut + Math.min(offset, noeud.data.length) + 1;
        const r = nouvelleRange();
        try { r.setStart(noeud, offset); } catch (e) { return 0; }
        let lo = 0;
        let hi = segs.length;
        while (lo < hi) { const mid = (lo + hi) >> 1; if (avant(segs[mid].node, r)) lo = mid + 1; else hi = mid; }
        return lo < segs.length ? segs[lo].start + 1 : (texte ? texte.length : 0) + 1;
    };

    // ── la sélection, quand elle se pose ─────────────────────────────────────
    let bas = false;
    let derniereSel = '';
    let selPrevue = 0;
    const envoyerSelection = () => {
        selPrevue = 0;
        const s = lireSelection();
        let from = 0;
        let to = 0;
        let rects = [];
        if (s && s.rangeCount && !s.isCollapsed) {
            aJour();
            const r0 = s.getRangeAt(0);
            from = posDe(r0.startContainer, r0.startOffset);
            to = posDe(r0.endContainer, r0.endOffset);
            if (to > from) rects = rectsDe(from, to);
            else { from = 0; to = 0; }
        }
        const k = from + ':' + to + ':' + tv + ':' + lv;
        if (k === derniereSel) return;
        derniereSel = k;
        post({ type: 'selection', from, to, tv, lv, rects });
    };
    const selectionBientot = () => { if (!selPrevue) selPrevue = plusTard(envoyerSelection, 0); };
    window.addEventListener('pointerdown', () => { bas = true; }, true);
    window.addEventListener('pointerup', () => { bas = false; selectionBientot(); }, true);
    window.addEventListener('blur', () => { bas = false; });
    document.addEventListener('selectionchange', () => { if (!bas) selectionBientot(); });

    // ── liens ────────────────────────────────────────────────────────────────
    document.addEventListener('click', (e) => {
        const a = e.target && e.target.closest ? e.target.closest('a[href]') : null;
        if (!a || e.defaultPrevented) return;
        const brut = a.getAttribute('href') || '';
        e.preventDefault();
        if (brut.startsWith('#')) {
            const id = decodeURIComponent(brut.slice(1));
            const cible = document.getElementById(id) || document.getElementsByName(id)[0];
            if (cible) post({ type: 'anchor', top: cible.getBoundingClientRect().top + window.scrollY });
            return;
        }
        post({ type: 'link', href: a.href, mod: e.ctrlKey || e.metaKey });
    }, true);

    // ── clavier ──────────────────────────────────────────────────────────────
    const champ = (el) => !!el && (el.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(String(el.tagName).toUpperCase()));
    const touche = (e) => post({ type: 'key', key: e.key, code: e.code, ctrlKey: e.ctrlKey, metaKey: e.metaKey, shiftKey: e.shiftKey, altKey: e.altKey });
    window.addEventListener('keydown', (e) => {
        if (e.ctrlKey || e.metaKey || e.key === 'Escape') { touche(e); return; }
        if (e.altKey || e.isComposing) return;
        if (!(e.key.length === 1 || e.key === 'Backspace' || e.key === 'Enter')) return;
        const s = lireSelection();
        if (!s || s.isCollapsed || champ(document.activeElement)) return;
        e.preventDefault();
        e.stopImmediatePropagation();
        touche(e);
    }, true);

    // ── le geste de la marge : un balayage horizontal que la page n'absorbe pas ──
    const absorbe = (cible, dx) => {
        for (let el = cible instanceof Element ? cible : null; el; el = el.parentElement) {
            if (el.scrollWidth <= el.clientWidth + 1) continue;
            const ox = getComputedStyle(el).overflowX;
            if (ox !== 'auto' && ox !== 'scroll' && el !== document.scrollingElement) continue;
            if (dx > 0 && el.scrollLeft + el.clientWidth < el.scrollWidth - 1) return true;
            if (dx < 0 && el.scrollLeft > 0) return true;
        }
        return false;
    };
    window.addEventListener('wheel', (e) => {
        if (e.ctrlKey || Math.abs(e.deltaX) <= Math.abs(e.deltaY) || absorbe(e.target, e.deltaX)) return;
        post({ type: 'wheel', deltaX: e.deltaX, deltaY: e.deltaY });
    }, { passive: true, capture: true });

    // ── les demandes de l'app ────────────────────────────────────────────────
    window.addEventListener('message', (e) => {
        if (e.source !== parent) return;
        const m = e.data;
        if (!m || m.fragmentHtml !== 1) return;
        if (m.type === 'theme') { appliquerTheme(m); return; }
        if (m.type !== 'rects' || m.tv !== tv || !Array.isArray(m.ranges)) return;
        const results = [];
        for (const g of m.ranges) if (Array.isArray(g)) results.push({ from: g[0], to: g[1], rects: rectsDe(+g[0], +g[1]) });
        post({ type: 'rects', tv, lv, results });
    });

    const demarrer = () => {
        hauteur();
        publierTexte();
        layout();
        if (typeof ResizeObserver !== 'undefined') {
            const ro = new ResizeObserver(() => { hauteur(); layout(); });
            ro.observe(document.documentElement);
            if (document.body) ro.observe(document.body);
        }
        new MutationObserver(() => { texteBientot(); layout(); })
            .observe(document.documentElement, { subtree: true, childList: true, characterData: true });
        if (document.fonts) document.fonts.addEventListener('loadingdone', layout);
    };
    if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', demarrer);
    else demarrer();
    window.addEventListener('load', () => { hauteur(); aJour(); layout(); });
    window.addEventListener('resize', layout);
})();