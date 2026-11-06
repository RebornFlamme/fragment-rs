// Config Vite « enveloppe » pour les références pixels.
//
// But : servir le renderer de Fragment sans rien écrire dans son arbre. On réutilise sa config
// telle quelle (plugins, define, alias) et on ne surcharge que deux choses :
//   - root     : Fragment/app (index.html, src/ et .env y sont lus, jamais écrits) ;
//   - cacheDir : un dossier hors de Fragment pour le pré-bundle des dépendances.
// ★ Le port 5123 est inchangé : le harnais e2e et dist-electron/main.js le codent en dur, il ne se
// surcharge pas. Le script de lancement décide s'il faut démarrer ce serveur ou réutiliser celui de
// l'utilisateur.
// ★ À lancer avec `--configLoader runner` : le chargeur par défaut compile cette config dans un
// fichier `.vite-temp` posé à côté du paquet de la config, ce qui écrirait dans Fragment. Le mode
// « runner » évalue la config en mémoire.
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import fragmentConfig from '../../Fragment/app/vite.config.ts';

const ici = path.dirname(fileURLToPath(import.meta.url));
const racineFragment = path.resolve(ici, '../../Fragment/app');
const cache = process.env.FRAGMENT_RS_VITE_CACHE ?? 'C:/dev/fragment-rs-vite-cache';

export default async (env: unknown) => {
    const base = typeof fragmentConfig === 'function' ? await (fragmentConfig as (e: unknown) => unknown)(env) : fragmentConfig;
    const b = base as Record<string, unknown> & { server?: Record<string, unknown> };
    return {
        ...b,
        root: racineFragment,
        cacheDir: cache,
        server: { ...b.server, port: 5123, strictPort: true },
    };
};
