// ═══════════════════════════════════════════════════════════════════════════
//  Décor des scènes de surfaces flottantes de T1 (notice, menu, infobulle, barre d'outils).
//
//  Ces surfaces ne dépendent pas de la fenêtre qui les porte : on cache donc l'interface de
//  Fragment (la racine React) et on peint le fond de scène en `--workspace-bg`. Le candidat Rust
//  fait pareil (un fond de thème, rien d'autre) : la comparaison ne mesure que la surface flottante.
//  Les onglets viennent avec `fr-t1-onglets`, qui a ses propres scènes.
// ═══════════════════════════════════════════════════════════════════════════
import { sceneT1, type ContexteT1 } from './_t1';

/** Le fond de scène : racine masquée, corps en `--workspace-bg`. */
async function viderLeDecor(c: ContexteT1): Promise<void> {
    await c.p.evaluate(() => {
        let s = document.getElementById('pixels-decor-vide') as HTMLStyleElement | null;
        if (!s) {
            s = document.createElement('style');
            s.id = 'pixels-decor-vide';
            document.head.appendChild(s);
        }
        s.textContent = '#root{visibility:hidden!important}body{background:var(--workspace-bg)!important}';
    });
}

/** Retire ce qu'une scène a posé sur le body (le thème suivant repart d'un body propre). */
async function nettoyer(c: ContexteT1): Promise<void> {
    await c.p.evaluate(() => {
        document.querySelectorAll('.notice-container, .menu, .tooltip, .toolbar, .pixels-cible').forEach((n) => n.remove());
    });
}

/** Une scène flottante : décor vidé, `corps` joue et capture, puis nettoyage. */
export function sceneFlottante(nom: string, corps: (c: ContexteT1) => Promise<void>): void {
    sceneT1(nom, async (c) => {
        await viderLeDecor(c);
        await c.act.hover(2, 2);
        try {
            await corps(c);
        } finally {
            await nettoyer(c);
        }
    });
}
