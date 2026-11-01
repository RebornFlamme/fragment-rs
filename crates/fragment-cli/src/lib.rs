//! CLI : protocole, index d'instances, shim, registre des coffres, client `fragment`.
//!
//! Modèle PUR uniquement (program.md §5, phase 1) : le transport réel (tuyau nommé,
//! sockets) viendra avec `fragment-app`. Ici : le contrat du fil, l'index des
//! instances (disque seulement, la sonde du tuyau est injectée), le shim, le registre
//! des coffres, les décisions du client et le traitement d'une requête côté serveur.

pub mod protocol;
