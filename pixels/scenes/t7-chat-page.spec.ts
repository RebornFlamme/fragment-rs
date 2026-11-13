// Scène T7 « t7-chat-page » : fenêtre étroite (700 px) : la colonne de la note, l'auteur au-dessus de la réponse, pas de barre.
import { sceneT7a, MAINTENANT, UNE_FEUILLE } from './_t7a';

sceneT7a('t7-chat-page', async ({ act, capturer, p }) => {
    await act.horloge(MAINTENANT);
    await act.layout(UNE_FEUILLE);
    await act.chat('page', 'c-page');
    await act.hover(2, 2);
    await act.waitStable();
    await capturer();
}, { fenetre: { largeur: 700, hauteur: 800 } });
