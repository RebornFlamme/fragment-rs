// Scène T2 « t2-gestionnaire-premier » : le gestionnaire de coffres au PREMIER lancement (aucun coffre
// connu) : fenêtre à part de 840 × 560, liste vide, deux actions (Créer, Ouvrir). Le JSON de cette scène
// est écrit par fr-t2-gestionnaire.
import { sceneT2 } from './_t2';

sceneT2('t2-gestionnaire-premier', async ({ act, capturer, sonde, p }) => {
    await act.hover(2, 2);
    await act.waitStable();
    for (const s of ['.vault-manager-empty', '.vault-manager-title', '.vault-manager-button', '.vault-manager-close']) await sonde(s, p, s);
    await capturer();
}, { fenetre: 'gestion', config: {} });
