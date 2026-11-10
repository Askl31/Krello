# Apple Intelligence locale — pertinence & faisabilité (macOS)

## Verdict

**Pertinente et faisable** — l'intégration est faite et **fonctionne** (testée sur
cette machine : `/api/v1/ai/status` → `{"available": true, "backend": "apple_intelligence"}`,
et une génération réelle a abouti). Elle colle parfaitement au cahier des charges
d'une application **locale** : le modèle (~3 Md de paramètres) tourne **on-device**
sur le Neural Engine, **gratuitement**, **sans clé API**, et **aucune donnée ne
quitte la machine** — exactement l'esprit « version locale » du projet.

## Cadre technique

| Élément | Détail |
|---|---|
| Framework | **FoundationModels** (`SystemLanguageModel`, `LanguageModelSession`, `GenerationOptions`) |
| Disponibilité | macOS **15.1+** (framework), machines **Apple Silicon**, Apple Intelligence **activée** (Réglages › Apple Intelligence & Siri) |
| Vérification runtime | `SystemLanguageModel.default.availability` → `.available` / `.unavailable(.deviceNotEligible | .appleIntelligenceNotEnabled | .modelNotReady)` |
| Coût | aucun (inclus dans l'OS) ; quotas Apple en cas d'usage intensif, jamais atteints en usage personnel |
| Confidentialité | 100 % local. *Private Cloud Compute* existe pour déborder vers le cloud, **non utilisé** (contre l'esprit local du projet) |

## Architecture d'intégration choisie

Le backend est Rust ; FoundationModels n'est appelable qu'en Swift/Obj-C.
Trois options existaient :

1. ~~FFI Rust → lib Swift statique~~ — link Swift/C fragile, runtime Swift à
   embarquer, problème pour un seul appel.
2. ~~Plugin Tauri natif~~ — lie l'IA à Tauri uniquement ; le binaire CLI en serait privé.
3. ✅ **Helper process Swift embarqué** — `crates/krello-ai-helper/swift/main.swift`
   compilé **au build** par `xcrun swiftc` (build.rs), binaire **embarqué** dans la
   lib Rust (`include_bytes!`), écrit dans un répertoire temporaire au lancement et
   **spawné** par `krello-core`. Protocole : une requête JSON par ligne stdin, une
   réponse JSON par ligne stdout ; ligne de statut au démarrage.

Avantages : zéro link Swift côté Rust, fonctionne identiquement dans le binaire
CLI **et** l'app Tauri, testable, dégradation propre (stub vide hors macOS ou
sans Xcode → `available: false` et UI masquée). Limites : ~120 ms de démarrage
du process par requête (négligeable face à l'inférence), une requête à la fois
(par design).

Dans le SDK récent (macOS 26/27), l'API est **async** : `session.respond(to:options:)`
et `GenerationOptions.maximumResponseTokens` (l'ancien `maximumTokens` n'existe
plus) — le helper est écrit pour ce SDK et dégrade sous 15.1 via `#available`.

## Cas d'usage implémentés (JSON validé côté Rust, avec retry correctif)

| Endpoint | Usage |
|---|---|
| `POST /api/v1/ai/parse-task` | « préparer la démo vendredi, priorité haute » → titre, description, **dates résolues**, priorité, labels, checklist ; création optionnelle `create: true` |
| `POST /ai/plan` | objectif → 3–12 tâches (durées, jalons, `depends_on`) → création cartes + dépendances **FS** + ordonnancement CPM immédiat |
| `POST /ai/summarize` | résumé d'avancement/risques/échéances du board (panneau Dashboard) |
| `POST /ai/priorities` | suggestions de priorité **justifiées** par carte, application optionnelle |
| `POST /ai/rewrite` | reformulation de description (modale carte) |
| `POST /ai/checklist` | génération d'étapes actionnables |

Tous les prompts exigent un **JSON strict** ; le Rust extrait le premier `{…}`,
valide contre le schéma (dates `YYYY-MM-DD`, priorités de l'enum, titre non vide)
et retente **une fois** avec un message correctif si le modèle bavasse.

## Pertinence produit

Pour un outil personnel local, l'IA on-device apporte le plus de valeur là où la
friction de saisie est la plus forte : création rapide de contenu (langage
naturel → structure), planification initiale d'un projet, revue périodique
(résumés, priorisation). La confidentialité on-device est un argument décisif
ici : les notes projet ne quittent jamais la machine. Les fonctions choisies
évitent les usages où le modèle local est faible (calcul exact, longues tables)
— la planification **chiffrée** reste le CPM déterministe du core Rust, l'IA ne
fait que produire la matière première.

## Limites & contournements

| Limite | Traitement |
|---|---|
| Machine non éligible / IA désactivée | `/ai/status` renvoie la raison (`device_not_eligible`, `apple_intelligence_not_enabled`, `model_not_ready`, `os_too_old`) ; l'UI masque ✨ et le panneau Dashboard |
| Windows / Linux | stub vide compilé → même contrat `available: false` ; Aucune divergence d'UI |
| Sorties non-JSON | extraction du bloc `{…}` + retry correctif unique + erreur typée `ai_unavailable` |
| Dates ambiguës (« vendredi prochain ») | le prompt fournit la date du jour ; validation `parse_date` côté Rust |
| Pas de streaming | `respond(to:)` non streaming suffisant pour des réponses courtes (< 2 048 tokens) |

## Faisabilité Windows (note)

Microsoft n'expose pas d'équivalent local universel à Foundation Models ; les
Copilot+ PC (Recall/Phi Silica via Windows AI Foundry) sont restreints à un
matériel NPU spécifique et à des API WinRT séparées. Hors périmètre : sur
Windows, Krello fonctionne intégralement **sans IA** (statut masqué). Une
extension future possible : provider OpenAI-compatible local (Ollama/LM Studio)
branché sur le même trait `AiProvider` — l'abstraction le permet sans toucher
l'UI.
