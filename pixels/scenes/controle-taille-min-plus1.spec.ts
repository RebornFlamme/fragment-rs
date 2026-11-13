// Contrôle du mode invisible (fr-278) : fenêtre 1 px au-dessus de la taille minimale de Fragment (601 × 401).
// Preuve sans référence : rien n'est écrit dans pixels/reference (voir pixels/invisible/controle-taille.ts).
import { controleTaille } from '../invisible/controle-taille';

controleTaille('controle-taille-min-plus1', 601, 401);
