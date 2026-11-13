# Intégration macOS — Ajout rapide de tâches via Spotlight (HUD natif)

## Objectif

Depuis **Spotlight** (`⌘Espace` → taper « krello » ou « ajouter » → `Entrée`), ouvrir instantanément un **HUD flottant compact** par-dessus l'application active pour saisir et enregistrer une tâche dans Krello **sans jamais changer d'espace de travail ni quitter l'application en cours**.

## Architecture & Fonctionnement

Spotlight ne permettant pas de passer des arguments en ligne de commande, la solution moderne et ergonomique repose sur un utilitaire léger nativement indexé dans `~/Applications/` :

```
⌘Espace (Spotlight)
      │
      ▼ (Tapez « krello » puis Entrée)
Ajouter à Krello.app (~/Applications/Ajouter à Krello.app)
      │  (LSUIElement = true : aucun icône Dock, reste au-dessus de l'app active)
      ▼
Fenêtre HUD flottante (AppKit NSPanel vibrant)
      │  • Lecture SQLite ultra-rapide des listes actives (~1ms)
      │  • Saisie texte avec focus automatique
      │  • Détection dynamique de @NomListe
      │  • Échap pour fermer, Entrée pour valider
      ▼
Validation (Entrée)
      ├──> HTTP POST http://127.0.0.1:8890/api/v1/deep-link (mise à jour WS temps réel si Krello ouvert)
      └──> Repli SQLite direct dans krello.db (si Krello est fermé)
      │
      ▼
Notification discrète macOS (« Tâche ajoutée dans Inbox ») & fermeture instantanée
```

## Composants techniques

1. **Applet Swift native (`crates/krello-server/swift/quick_add.swift`)** :
   - Panneau `NSPanel` avec `canBecomeKey = true`, style `.hudWindow` translucide vibrant.
   - Saisie rapide, menu déroulant des listes actives, détection de syntaxe `@Liste`.
   - Fermeture automatique au clic en dehors (`NSEvent.addGlobalMonitorForEvents`).
   - Double stratégie d'écriture : appel API local HTTP (ports 8890 / 8787) pour diffusion WebSocket immédiate, et repli SQLite direct si Krello est fermé.

2. **Module d'installation backend (`crates/krello-server/src/quick_add.rs`)** :
   - `install()` : compile `quick_add.swift` via `swiftc -O`, configure le bundle `Ajouter à Krello.app` avec `Info.plist` (`LSUIElement = true`) et l'icône de l'app, applique une signature ad-hoc avec `codesign` et enregistre le bundle auprès de LaunchServices (`lsregister`) pour indexation Spotlight immédiate.
   - `status()` : vérifie la présence du bundle dans `~/Applications`.
   - `uninstall()` : supprime le bundle et nettoie LaunchServices.
   - `test_launch()` : ouvre l'utilitaire via `open` pour tester.

3. **Routes API REST (`crates/krello-server/src/routes.rs`)** :
   - `GET /api/v1/macos/quick-add/status`
   - `POST /api/v1/macos/quick-add/install`
   - `POST /api/v1/macos/quick-add/uninstall`
   - `POST /api/v1/macos/quick-add/test`

4. **Interface Paramètres (`frontend/src/views/settings.js`)** :
   - Onglet **macOS** entièrement repensé avec statut d'installation en direct.
   - Bouton d'installation en 1 clic (« Installer dans Spotlight »).
   - Boutons « Tester l'ouverture », « Réinstaller » et « Désinstaller ».
   - Guide visuel étape par étape expliquant l'utilisation avec Spotlight et la syntaxe `@NomListe`.
