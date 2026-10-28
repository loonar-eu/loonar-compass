# Changelog

Toutes les modifications importantes de l'application Loonar Compass (nommée Loonar EA Assistant jusqu'à la version 0.3).

## Version 0.9.1 (2026-10-27)

- **Jira et Confluence avec un certificat client :** les serveurs d’entreprise qui exigent un certificat client (mTLS) en plus du jeton fonctionnent désormais avec Compass. Les cartes Jira et Confluence des Paramètres ont les champs « Certificat client » (chemin d’un fichier .p12/.pfx ou PEM) et « Mot de passe du certificat » (conservé dans le Gestionnaire d’identification Windows). Sans certificat, un tel serveur fermait la connexion (« Une connexion existante a dû être fermée par l’hôte distant »).
- **Installateur simplifié :** plus de page « Configuration » – la clé, le modèle et le compte EA se saisissent dans l’application, qui vous guide au premier démarrage.
- **Réessayer l’installation :** quand l’installation ne peut pas remplacer les fichiers parce que Compass est lancé, la page d’erreur propose « Fermer Compass et réessayer » et « Réessayer » – sans tout recommencer.
- **Adresses Jira et Confluence collées depuis le navigateur :** la partie d’une page (par ex. « /spaces/… », « /display/… », « /browse/… ») est retirée, et quand le serveur renvoie une page web au lieu de données, Compass invite à vérifier l’adresse (auparavant la vérification indiquait « Connecté » et les pages revenaient vides).
- **L’assistant lit le code source :** nouveaux outils en lecture seule – fichiers d’un dossier, recherche dans le code (fichier:ligne) et lecture d’un fichier – dans des clones locaux de dépôts. Les dossiers indiqués dans Paramètres → « Code source » (une carte visible aussi dans EA ; le bouton « … » ajoute un dossier) sont lus sans demander ; pour les autres il demande d’abord. Il trouve ainsi seul où une classe, une opération ou une table est implémentée et ce qui en dépend.
- **OneDrive et Google Drive :** l’assistant liste les dossiers, lit les documents (Word, Excel, PowerPoint, PDF, Visio, texte ; sur Google Drive aussi Google Docs, Sheets et Slides) et, chaque fois avec votre accord, envoie les documents qu’il a créés. Vous vous connectez une fois dans le navigateur avec votre propre compte Microsoft ou Google (OAuth avec PKCE) ; le jeton d’actualisation est conservé dans le Gestionnaire d’identification Windows. Paramètres : carte « OneDrive » (inscription d’application dans Entra ID comme client public) et carte « Google Drive » (client OAuth « Application de bureau » dans Google Cloud).
- **Nouvelle version et statistiques toutes les heures :** Compass vérifie toutes les heures s’il existe une nouvelle version (aussi quand il reste ouvert), et les statistiques – si elles sont activées – sont envoyées au démarrage (identifiant d’installation, version, système, langue) et toutes les heures (résumé de la journée : types de tâches et leur issue, évaluations des réponses, ce que l’assistant n’a pas su faire). Sous les réponses : 👍/👎 et – quand l’assistant ne sait pas faire quelque chose – « 💡 Demander cette fonction ».
- **Conversation dans un autre modèle, export et import :** après le déplacement d’un projet (par exemple d’une base de données vers un fichier), une conversation peut continuer avec le nouveau modèle. Dans l’onglet EA : « ⋯ » → « Conversations d’autres modèles… » (aussi un lien sous la liste) → « Continuer ici ». Une copie est créée, l’original reste avec son modèle, et les notes de mémoire de l’ancien modèle peuvent être rattachées au nouveau. L’assistant est informé que les messages précédents concernaient un autre modèle et vérifie d’abord les éléments cités. Dans la fenêtre séparée, le modèle d’une conversation se change comme avant avec le bouton « Modèle : … ». Une conversation et ses pièces jointes s’exportent dans un fichier `.lcchat` (menu « ⋯ » dans EA, clic droit sur une conversation dans la fenêtre) et s’importent sur un autre ordinateur – elle rejoint le modèle ouvert dans cette fenêtre. Le fichier contient le chemin du fichier ou le nom de la base et du serveur, sans comptes ni mots de passe.
- **L’assistant se souvient de ce qu’il a fait :** les étapes de chaque réponse (lectures, recherches, enregistrements dans EA, pages Confluence) restent dans la conversation et l’assistant les reçoit aux tours suivants. Auparavant, l’historique ne contenait que le texte des réponses ; interrogé plus tard, il pouvait nier avoir, par exemple, lu Confluence alors qu’il l’avait fait.
- **L’assistant travaille seul jusqu’au bout :** il fait toute la tâche en une réponse au lieu de montrer l’état et de demander « je continue ? » toutes les quelques étapes. Les choix raisonnables (noms, fichiers, format, ordre), il les fait lui-même et les cite dans la réponse ; il ne demande que lorsqu’une mauvaise décision changerait beaucoup dans le modèle. Les notes du type « je lis maintenant… » sont dans les étapes du travail, pas en messages séparés dans le chat. La limite est de 40 étapes ; une fois atteinte, l’assistant dit ce qui est fait et ce qui reste au lieu de finir sur une erreur. S’il s’arrête quand même avant la fin (tâches ouvertes dans son plan, ou « écrivez continuer »), Compass le relance de lui-même, jusqu’à 3 fois par réponse.
- **Moins de questions sur les scripts EA :** après la première validation, les scripts suivants de même structure – le même code avec d’autres données, par exemple le lot suivant d’alias – sont enregistrés et exécutés sans demander (jusqu’à la fermeture de Compass) ; un autre code demande comme avant. L’assistant règle désormais directement l’alias et le stéréotype des opérations et des attributs à l’écriture, les alias (par exemple les chemins REST) ne nécessitent donc plus de script. La carte de validation d’un script a aussi le bouton « Approuver tous les scripts jusqu’à la fermeture de Compass », et les modifications possibles par une écriture ordinaire sont faites sans script.
- **Une validation pour une série :** la carte de validation d’un ticket Jira, d’une publication Confluence, d’une note de mémoire, d’un appel d’outil MCP, d’un envoi de fichier et d’un script EA a le bouton « Approuver tous les … jusqu’à la fermeture de Compass ». Dix tickets Jira font un clic au lieu de dix.
- **Stories et cas d’utilisation sous un epic dans Jira Server / Data Center :** un ticket dont le parent est un epic est rattaché par le champ « Epic Link » (`parent` n’y sert qu’aux sous-tâches). Auparavant, Jira le refusait avec « Issue type … is not a sub-task but a parent is specified ». Jira Cloud fonctionne comme avant.
- **Modifier les tickets Jira :** l’assistant corrige les tickets existants – résumé, description et étiquettes (ajout et retrait), par exemple retire une mention inutile des critères d’acceptation de tous les tickets d’un coup. Validation comme à la création, aussi une fois pour toutes.
- **Coûts du modèle d’IA réduits :** toute la conversation de la boucle de l’assistant – historique et résultats d’outils, comme les fichiers de code lus – est désormais dans le cache d’Anthropic : chaque étape suivante la paie au dixième du prix. Le journal des modifications n’est plus collé dans chaque requête (environ 18 000 jetons de moins) ; l’assistant le lit avec un outil quand vous l’interrogez sur les fonctions de Compass. Dans le test d’analyse de code, le coût d’une tâche a baissé d’environ 79 %. Sous chaque réponse, l’étape « 💲 Coût de cette réponse » indique les appels, les jetons, la part du cache et le coût en USD ; chaque appel va aussi dans le journal local `usage` (des chiffres seulement, aucun contenu).
- **Très longues conversations moins chères :** avant qu’une requête n’approche 100 000 jetons (au-delà, Claude Haiku 5.5 facture toute la requête 5×), la partie ancienne de la conversation devient un résumé conservé avec elle – le chat affiche la ligne « L’assistant voit la conversation ci-dessus sous forme de résumé », et les 8 derniers messages restent mot pour mot. Lors d’un très long travail avec les outils (par exemple des dizaines de fichiers lus), Anthropic retire lui-même de la vue du modèle les plus anciens résultats d’outils au-delà de 85 000 jetons ; les 6 plus récents restent et l’assistant peut relire un fichier.
- **La partie fixe d’une requête en cache pendant une heure :** les outils et instructions de l’assistant (~35 000 jetons) restent 1 heure au lieu de 5 minutes dans le cache d’Anthropic : après une pause entre deux questions, ils ne sont pas réécrits (une écriture coûte autant qu’une douzaine de lectures). La conversation – écrite à chaque étape – reste à 5 minutes.

## Version 0.9 (2026-10-27)

Connexion d'entreprise au hub, modèles sur SQL Server, MySQL et Oracle, instructions de modélisation de l'équipe dans le référentiel, documents PDF, PowerPoint, Visio et Excel, présentations, SharePoint, gardiens qui arrêtent les suppressions, et un assistant plus attentif : questions, plan, sous-agents et protection contre les instructions cachées.

### Compass Hub

- **Connexion avec le compte d'entreprise (OpenID Connect) :** AD FS, Microsoft Entra ID, Keycloak, Okta et d'autres fournisseurs, configurés dans la section `"oidc"` de `hub.json`. La vue web affiche « Sign in with your company account ».
  - Authorization code avec PKCE ; le hub vérifie lui-même le jeton d'identité (signature RS256 avec les clés du fournisseur, émetteur, audience, expiration, nonce).
  - Rôles à partir des groupes (`roles`, par défaut Compass-Admins, Compass-Architects, Compass-Viewers) : le rôle le plus élevé l'emporte, sans groupe pas d'accès. Un utilisateur local portant le même nom est refusé.
  - Le secret client est le secret du hub `OIDC_CLIENT_SECRET`. `"password_login": false` ne laisse que la connexion d'entreprise ; les jetons API fonctionnent toujours. Chaque connexion est consignée dans le journal d'audit.
- **Hub derrière IIS/ARR et audit des connexions :** `allatclaims` ajouté automatiquement pour AD FS ; le blocage après des mots de passe erronés utilise l'adresse du client tirée de `X-Forwarded-For` (seulement depuis un proxy sur le même ordinateur) ; connexions échouées et refusées dans « Audit trail » avec ✗ ; l'onglet « Users » liste les comptes d'entreprise avec rôle et dernière connexion (lecture seule) ; le service Windows s'arrête et redémarre proprement. Manuel : le paramètre ARR `reverseRewriteHostInResponseHeaders="false"`.
- **Agents d'événements pour l'équipe :** la clé de politique `"event_agents"` (même format que `event_agents.json`, gardiens compris). Les agents fonctionnent dans le Compass dans EA de chacun, avec la mention « (team) ».
- **Onglet « Team policy » (admin) :** un éditeur JSON de la politique (min_approval, governance, event_agents, ask_min_role, ask_per_hour).
- **Rapports Word et Excel :** « Word report of the package » et « Excel report of the package » dans l'onglet Model check : description, éléments, relations (sans images des diagrammes). Chaque téléchargement est consigné.
- **GraphQL et Power BI :** `POST /api/sources/{id}/graphql` en lecture seule (paquetages, éléments avec attributs, opérations, tagged values et relations, diagrammes ; schéma à `GET /api/graphql/schema`) et tables plates `GET /api/sources/{id}/table/{elements|connectors|packages|diagrams|attributes|tagged_values}` pour Power BI (Obtenir les données → Web → Avancé, en-tête `Authorization: Bearer <jeton API>`). Tous les rôles.

### Modèles sur SQL Server, MySQL et Oracle

- **Nouveaux référentiels :** outre PostgreSQL et les fichiers .qea, Compass lit les référentiels EA sur SQL Server, MySQL/MariaDB et Oracle : dans la fenêtre autonome, dans le hub et en mode de contrôle. Paramètres → Modèle propose les choix SQL Server, MySQL et Oracle.
- **Pilote ODBC :** le pilote ODBC 64 bits de l'éditeur est trouvé automatiquement (SQL Server : ODBC Driver 18 ou 17, ou le pilote « SQL Server » propre à Windows). Une chaîne de connexion complète sans le mot de passe peut être indiquée dans `db.odbc` (par exemple pour Oracle).
- **Lecture seule :** chaque lecture est annulée (rollback).
- **Enregistrement par EA** (l'édition d'EA doit prendre en charge les référentiels SGBD) :
  - SQL Server : Compass construit lui-même la connexion d'EA (fournisseur OLE DB SQLOLEDB), comme pour PostgreSQL.
  - MySQL et Oracle : la connexion d'EA vient du nouveau paramètre `ea.conn` dans app.properties, le texte d'un raccourci de connexion EA par une source de données ODBC (p. ex. `ea_lab --- DBType=0;Connect=Provider=MSDASQL.1;Data Source=<DSN ODBC>;…`). Sans lui, les modifications passent par Compass dans EA, qui fonctionne comme avant avec tout référentiel ouvert dans EA.
- **Docker :** l'image du hub contient unixODBC ; ajoutez le pilote ODBC Linux de l'éditeur dans une image construite sur celle-ci.

### Instructions de modélisation dans le référentiel

- **Le règlement de l'équipe dans EA :** un Artifact «CompassInstructions» (« Loonar Compass – modelling instructions ») dont les notes sont les règles du référentiel (nommage, types et stéréotypes, paquetages, diagrammes, tagged values, contrôles). L'assistant les suit dans chaque réponse, pour tous ceux qui ouvrent le référentiel.
- **Assistant de démarrage :** l'assistant analyse le référentiel et rédige un projet de règles ; l'enregistrement et l'ajout d'une règle sont des modifications du modèle soumises à validation.
  - Exemple : « Prépare des instructions de modélisation pour ce référentiel ».
  - Exemple : « Ajoute aux instructions : les activités commencent par un verbe ».

### Conversation

- **Questions de clarification sous forme de formulaire :** lorsqu'un point important n'est pas clair, l'assistant affiche « Questions pour vous » avec des choix ou une réponse courte (« Autre chose… ») ; « Envoyer les réponses » les envoie comme message suivant (« Mes réponses : »).
- **Plan d'un travail plus important :** « 📋 Plan : 1 sur 4 faits » avec ✓ ▶ ○ dans les étapes (et dans System Output d'EA), mis à jour au fil du travail.
- **Sous-agents :** pour les analyses plus vastes, l'assistant confie une tâche à un sous-agent de rôle impact, review ou document, qui lit le modèle dans son propre contexte et renvoie un résultat compact (« Sous-agent (impact) : … »).
- **Choisir dans le modèle :** le bouton ⌖ à côté de Coller cherche des éléments, paquetages et diagrammes par leur nom (vide = paquetages principaux) et les ajoute au message sous forme de références, comme «Élément: Order (Class, id 26)».
- **Le raisonnement de l'assistant :** le bouton 💭 à côté de Coller affiche un résumé du raisonnement comme étape « 💭 Raisonnement de l’assistant » (Claude uniquement ; l'effort reste réglé dans les Paramètres).
- **Code dans le bac à sable d'Anthropic :** le bouton ∑ à côté de Coller (désactivé par défaut ; Claude uniquement) permet à l'assistant d'exécuter du code pour des calculs et l'analyse de données ; son résultat est l'étape « Code dans le bac à sable ». Les données qui y sont transmises vont à Anthropic, comme la conversation.
- **Web (🌐) :** un bouton à côté de ∑ (désactivé par défaut). Avec Claude, recherche web et lecture de pages via Anthropic ; avec OpenAI, Compass lit seulement les pages dont l'adresse a été donnée dans la conversation (jamais d'adresses de cet ordinateur ni du réseau de l'entreprise). Domaines autorisés : `claude.web_domains`. Politique du hub : `"web_access": false` le désactive pour tous, `"web_allowed_domains"` limite les domaines. Les recherches et les pages apparaissent dans les étapes de la réponse.
- **Avis de nouvelle version :** une fois par jour, Compass vérifie la dernière version sur GitHub et affiche une barre « Nouveautés et téléchargement / Pas maintenant » (dans EA : une ligne dans System Output). Rien n'est téléchargé ni installé tout seul, aucun identifiant d'installation. Désactivation : Paramètres → Nouvelles versions (newsletter) ou `app.update_check=false` ; politique du hub `"update_check": false`.
- **Stratégie de l'ordinateur (stratégie de groupe, Intune) et connexion d'entreprise :** la clé `HKLM/HKCU\SOFTWARE\Policies\Loonar\Compass` – `HubUrl` (adresse du hub verrouillée), `RequireHub` (l'assistant uniquement avec le hub et sa politique ; si le hub est en panne, la dernière politique jusqu'à une semaine), `WebAccess`, `UpdateCheck`, `MinApproval`, `AllowedProviders`. Modèles ADMX/ADML dans `deploy\policies`. « Se connecter avec le compte d'entreprise » dans Paramètres → Équipe : la connexion du hub dans le navigateur, la session revient sur 127.0.0.1 et va dans le Gestionnaire d'identification.
- **Clés d'IA, Jira et Confluence sur le hub :** la passerelle `/ai/v1/messages` – le hub conserve les clés Claude par personne (nom ou UPN AD), rôle ou pour tous, avec des limites de jetons par jour et par mois et une liste de modèles ; consommation par personne dans l'onglet Users. Jira et Confluence via le hub avec le compte de l'équipe (un viewer ne fait que lire, modifications dans l'audit). Politique : `ai_via_hub`, `jira_via_hub`, `confluence_via_hub`.
- **Un gardien sans décision bloque :** si un gardien ne décide pas en 40 s ou ne le peut pas (pas d'IA, hub indisponible), la suppression est arrêtée avec un message (auparavant, elle avait lieu).
- **Installation silencieuse :** `LoonarCompass-Setup --quiet [--all-users] [--dir …] [--lang …] [--no-desktop] [--no-start-menu] [--no-ea-addin]` – déploiement avec Intune, Configuration Manager ou un script ; pour tous les utilisateurs, le complément EA fonctionne pour chacun.
- **Gardien des suppressions dans EA :** EA ne se fige plus pendant la décision, et le gardien décide vraiment (auparavant, la suppression avait lieu après 40 s sans décision).
- **Lecture des pages Confluence :** l'assistant lit une page par titre ou id, aussi via le hub.
- **Guide d'installation du hub pour l'IT** (anglais et polonais).
- **Compass dans EA dès le démarrage :** la fenêtre Compass est visible dès le lancement d'EA ; sans projet elle affiche « Aucun projet ouvert… », Compass démarre seul à l'ouverture d'un fichier ou d'un référentiel et revient à ce message à la fermeture du projet.
- **Redémarrage du hub :** les sessions des personnes connectées survivent à un redémarrage du hub ; un même jeton invalide ne compte qu'une fois pour le blocage de l'adresse ; Compass demande aussitôt de se reconnecter quand la connexion au hub a expiré et décrit brièvement les erreurs du hub (sans pages HTML d'IIS).
- **Glossaire du projet EA :** le type par défaut d'un nouveau terme est `Glossary` (auparavant `Slownik`).
- **Longues conversations :** au-delà d'environ 100 000 jetons, la première partie est remplacée par un résumé pour les réponses suivantes (« La conversation est longue : résumé de sa première partie ») ; les messages les plus récents restent.
- **Supprimer une conversation :** clic droit sur une conversation → « Supprimer la conversation » (dans Compass dans EA : le bouton 🗑 à côté du nom de la conversation). Elle est déplacée dans `history\bin` et peut être récupérée ; pas pendant que l'assistant y travaille.

### Documents, présentations et SharePoint

- **Lecture de documents :** l'outil `read_document` et les pièces jointes lisent les PDF (chaque page ; joignez un scan sans couche de texte pour que le modèle lise les images des pages), PowerPoint .pptx (diapositives avec notes), Visio .vsdx (pages, formes avec texte, connexions A → B), Excel .xlsx (feuilles, cellules) et Word. Les anciens fichiers .doc, .ppt, .xls et .vsd doivent être enregistrés dans le nouveau format.
  - Les exigences pour `trace_requirements` et `--requirements` peuvent provenir de ces formats.
- **Présentations PowerPoint :** `create_presentation` crée une présentation 16:9 : une diapositive de titre, des diapositives avec des puces sur deux niveaux et/ou un diagramme ou une image, en option aussi en PDF (par Microsoft PowerPoint).
- **Documents PDF :** `create_document` avec le format pdf ou both (par Microsoft Word). Le PDF nécessite Microsoft Office sur l'ordinateur.
- **SharePoint :** la nouvelle carte Paramètres → SharePoint (adresse du site, bibliothèque de documents, ID du locataire, ID d'application (client), secret client ; « Enregistrer et tester la connexion SharePoint »). Elle nécessite une inscription d'application dans Entra ID avec l'autorisation d'application Microsoft Graph Sites.Selected (accordée au site) ou Sites.ReadWrite.All.
  - L'assistant liste un dossier, lit le texte d'un document et envoie un fichier qu'il a créé ; chaque envoi est soumis à validation.

### Enterprise Architect

- **Agents d'événements pour 18 événements :** création, modification et suppression (deleting) des éléments, connecteurs, diagrammes et paquetages ; création et suppression des attributs, méthodes et termes du glossaire.
- **Gardiens :** `"mode": "guard"` sur un événement `*_deleting`. EA consulte Compass avant de supprimer ; le gardien répond ALLOW ou BLOCK avec une raison, et EA affiche « Loonar Compass a arrêté cette suppression : » et annule. Sans réponse dans les 40 s, la suppression a lieu.
  - Exemple : `{"name": "Keep approved", "event": "element_deleting", "mode": "guard", "applies_to": "Requirement", "prompt": "Block deleting a requirement whose status is Approved."}`.
- **Technologies MDG :** l'outil `mdg_technologies` : les profils utilisés par le modèle (avec leur nombre), les fichiers de technologies d'EA et les vôtres, les technologies importées dans le modèle, les stéréotypes avec la métaclasse et ce à quoi ils s'appliquent, et leurs tagged values avec type, valeurs et valeur par défaut.
- **L'API d'EA :** l'outil `ea_api` lit l'interface d'automatisation dans EA.tlb de l'EA installé (classes, propriétés en lecture ou lecture/écriture, méthodes avec paramètres, énumérations) ; l'assistant s'en sert avant d'écrire des scripts EA.
- **Contrôle de version des paquetages :** l'outil `version_control_status` (paquetages sous contrôle de version, configuration, fichier, qui les a extraits) et l'opération de modèle `version_control` (check_out, check_in, get_latest, put_latest, resync, avec un commentaire). Toujours soumise à validation ; le paquetage doit déjà être sous contrôle de version dans EA.

### Sécurité et connexion

- **Protection contre les instructions cachées (prompt injection) :** des règles de sécurité dans chaque requête ; les résultats des documents, de Jira, de Confluence, des serveurs MCP, de la CMDB et du code sont marqués comme des données ; une réponse qui reprend mot pour mot les instructions internes est retenue.
- **Serveurs MCP sur HTTP :** `"auth": {"type": "basic", "user", "password"}` ou `{"type": "oauth2", "token_url", "client_id", "client_secret", "scope"}` dans `mcp_servers.json` (secrets sous la forme `${secret:mcp_…}`).
- **Le service d'IA avec OAuth2 :** nouveaux champs sur la carte Modèle d'IA : « Adresse du jeton OAuth2 » (facultatif, à la place d’une clé API), puis « ID client OAuth2 », « Portée OAuth2 (scope) » et « Secret client OAuth2 » (conservé dans le Gestionnaire d'identification). Par exemple Entra ID avec la portée `https://cognitiveservices.azure.com/.default`. Enregistrés sous `ai.oauth.*`.
- **Claude par une autre adresse :** le champ « Adresse de Claude » (Claude uniquement ; vide = Anthropic), par exemple Azure AI Foundry `https://<ressource>.services.ai.azure.com/anthropic` ou une passerelle d'entreprise. Enregistré sous `claude.url`.

## Version 0.8.2 (2026-10-23)

Compass Hub devient un programme à part entière (Windows, Linux, Docker) avec des comptes utilisateurs. Cette version apporte aussi le contrôle du modèle dans les pipelines CI, les règles ArchiMate, la chaîne de traçabilité complète, la comparaison avec une CMDB et des serveurs MCP prêts à l'emploi.

### Compass Hub, programme à part entière

- **`compass-hub` autonome :** un petit programme sans fenêtre et sans EA, pour Windows et Linux. Lancement : `compass-hub hub.json`. `LoonarCompass.exe --hub` fonctionne toujours.
- **Service Windows :** `compass-hub --service-install C:\Hub\hub.json` (en administrateur) fait démarrer le hub avec Windows. `--service-uninstall` retire le service.
- **Docker sous Linux :** l'image `compass-hub` (`hub/Dockerfile`) garde ses données et `hub.json` dans le volume `/data` et s'exécute en utilisateur normal.
  - Exemple : `docker run -d -p 8730:8730 -v /srv/compass-hub:/data -e LOONAR_SECRET_CLAUDE_API_KEY=… compass-hub`.
- **Secrets du hub :** lus dans les variables d'environnement `LOONAR_SECRET_<NOM>` ou dans les fichiers `LOONAR_SECRET_<NOM>_FILE` (p. ex. `CLAUDE_API_KEY`, `JIRA_TOKEN`, `CONFLUENCE_TOKEN`, `DB_PASSWORD`).

### Comptes et connexion au hub

- **Écran de connexion :** connexion par nom d'utilisateur et mot de passe. Les mots de passe ne sont conservés que sous forme d'empreintes PBKDF2, et une session dure 12 heures. La connexion par jeton reste possible.
- **Compte par défaut :** un nouveau hub sans utilisateurs a le compte **admin / passw0rd** (avec un zéro). Il faut changer son mot de passe à la première connexion ; d'ici là, le compte ne permet rien d'autre.
- **Onglet « Users » (admin) :**
  - ajouter un utilisateur, avec un premier mot de passe qu'il devra changer ;
  - changer son rôle ou réinitialiser son mot de passe ;
  - créer un nouveau jeton d'API pour Compass et les clients MCP (affiché une seule fois) ;
  - supprimer un utilisateur (il reste toujours au moins un admin).
- **Blocage :** après 20 mots de passe erronés depuis une adresse, le hub refuse cette adresse pendant 10 minutes.

### Jira et Confluence depuis le hub

- **Compte technique :** l'administrateur ajoute une section `"integrations"` dans `hub.json` (Jira : adresse et projet ; Confluence : adresse et espace) et fournit les jetons comme secrets du hub.
- **Boutons pour les architectes dans la vue web :**
  - « Create Jira issue » sur un élément et sur un lot de modifications ;
  - « Publish this package to Confluence » dans le contrôle d'un paquetage. La page ne contient que du texte (éléments avec descriptions et relations), sans images de diagrammes, car le hub n'a pas d'EA.
- **Journal et Ask :** chaque action est inscrite au journal d'audit. « Ask » ne fait toujours que lire le modèle.

### Contrôle du modèle dans les pipelines CI

- **Mode contrôle :** `LoonarCompass.exe --check --model modele.qea [--code src] [--requirements BRD.docx] [--cmdb cmdb.csv] [--report rapport.md] [--junit junit.xml] [--fail-on error|warning]`.
- **Résultat :** un rapport Markdown, un fichier JUnit pour GitLab, GitHub Actions et Azure DevOps, et un code de sortie 0 (OK), 1 (constats) ou 2 (erreur). Sans IA et sans EA.

### Modèle, exigences, code et CMDB

- **Règles ArchiMate 3.2 :** le contrôle du modèle vérifie les relations des éléments ArchiMate (p. ex. access uniquement vers la structure passive, influence uniquement vers la motivation). Pour les désactiver : `"archimate": false` dans governance.json.
- **Chaîne de traçabilité :** `trace_requirements` avec un dossier de code suit chaque exigence → modèle → code → test → ticket, et montre où la chaîne se rompt.
  - Exemple : « Compare les exigences de C:\Projets\BRD.docx avec le modèle et le code dans C:\repo\boutique ».
- **CMDB face au modèle :** un export CSV/JSON (p. ex. de ServiceNow), ou des données obtenues par un serveur MCP, est comparé aux composants du modèle. Le rapport montre ce qui manque dans le modèle, ce qui est absent de la CMDB, et où le responsable ou le statut diffère.
  - Exemple : « Compare l'export CMDB C:\Export\cmdb_ci_appl.csv avec les composants du modèle ».

### Serveurs MCP

- **Serveurs prêts à l'emploi :** Azure DevOps, GitHub, ServiceNow / CMDB, fichiers locaux et Compass Hub sur la carte « Serveurs MCP ». Choisissez-en un dans la liste, puis complétez l'organisation ou l'adresse.
- **Jeton d'un serveur MCP :** enregistré depuis la carte dans le Gestionnaire d'identification (les noms commencent par `mcp_`).
- **Programmes Node.js :** `npx` dans la configuration fonctionne sous Windows (lancé en `npx.cmd`).

## Version 0.8.1 (2026-10-23)

Version de correctifs de sécurité après un audit de toute l'application. Recommandée pour tous, en particulier pour les équipes avec Compass Hub. Certaines actions demandent maintenant votre accord plus souvent qu'avant : c'est voulu.

### Compass Hub

- **Robustesse :** une requête malformée n'arrête plus le hub. Les requêtes sont limitées en taille, en nombre d'en-têtes et en durée, et le jeton est vérifié avant la lecture du contenu.
- **Jetons :** hub.json peut contenir "token_sha256" (empreinte du jeton) au lieu de "token" ; `LoonarCompass.exe --hub-token` crée un nouveau jeton aléatoire et son empreinte. Noms et jetons doivent être uniques. Après 20 jetons erronés, une adresse est refusée pendant 10 minutes (un jeton valide fonctionne toujours).
- **Lots de modifications :** le hub calcule lui-même, à partir des opérations, la description et l'aperçu que voit le relecteur, et la vue web montre les opérations. Personne, pas même un admin, n'approuve son propre lot. L'application se fait en deux étapes (prise par un Compass, puis résultat), un lot ne peut donc pas être appliqué deux fois. Un admin peut libérer un lot resté bloqué.
- **Ask :** l'assistant de la vue web ne fait que lire le modèle (pas de fichiers, ni Jira, ni Confluence, ni serveurs MCP) et répond à une question à la fois, avec une limite de temps. Nouveaux réglages de politique : "ask_min_role" (qui peut poser des questions) et "ask_per_hour" (20 questions par personne et par heure par défaut).
- **Vue web :** Content-Security-Policy et protection contre l'intégration dans un cadre. Le jeton n'est conservé que pour la session du navigateur, avec un bouton « Sign out ». Les erreurs internes s'affichent sous forme d'identifiant ; les détails vont dans hub.log.
- **Journal d'audit :** n'accepte que les champs connus, horodatés par le hub. Les données des postes ne sont visibles que par l'admin.

### Accords et enregistrements dans le modèle

- **Fichiers :** l'export XMI, les documents issus des modèles EA et l'import XMI demandent toujours votre accord (même en mode auto) et affichent le chemin complet. Seuls les chemins complets sur un lecteur local sont acceptés, sans partages réseau ni dossiers système.
- **Suppression :** un identifiant donné en texte ou en nombre décimal ne contourne plus la question lors de la suppression d'un élément qui a des relations. Supprimer un paquetage demande toujours.
- **Mémoire, modèles et scripts :** enregistrer une note de mémoire ou un modèle de document, et enregistrer ou exécuter un script EA, demandent toujours, avec tout le texte ou tout le code. Un script s'exécute exactement comme affiché, avec ses inclusions.
- **Lecture de fichiers :** avant de lire un fichier hors des pièces jointes de la conversation (document d'exigences, dossier de code), l'assistant demande.
- **Requêtes SQL :** l'assistant ne peut que lire. Les requêtes contenant des mots d'écriture (INSERT, UPDATE, DELETE, INTO…) ou des guillemets inhabituels sont refusées, y compris à la lecture par la fenêtre EA.
- **Agents d'événements :** ils ne font que lire le modèle. Les noms d'objets entrent dans leur demande comme des données, pas comme des instructions.
- **Annulation :** le journal d'annulation est vérifié avant exécution, et l'accord montre ce qui sera supprimé.

### Serveurs MCP et connexions

- **Accord MCP :** chaque appel d'outil MCP demande votre accord, sauf si le serveur a "trusted": true. La déclaration « lecture seule » du serveur ne suffit plus.
- **Secrets :** les clés et mots de passe de Compass lui-même ne peuvent pas être transmis à un serveur MCP par ${secret:…}. Exception : ${secret:hub_token} pour le point /mcp du hub des Paramètres. Un programme MCP local ne reçoit que les variables d'environnement de base et celles de "env".
- **Redirections et adresses :** les redirections HTTP ne sont suivies que sur le même serveur, les clés d'API ne partent donc pas ailleurs. Une adresse http:// n'est autorisée que pour cet ordinateur, sans nom d'utilisateur ni mot de passe.

### Autres

- **Éditeur de diagrammes :** les noms du modèle s'affichent comme du texte (pas de HTML), la page a une CSP, et seuls les objets du diagramme ouvert peuvent être supprimés ou modifiés.
- **Complément EA :** le journal ne contient plus la chaîne de connexion, Compass démarre dans son propre dossier, et les DLL ne sont pas cherchées dans le dossier courant.
- **Boîtes d'erreur EA :** seules celles qui apparaissent pendant la requête de Compass sont fermées.
- **SQLite 3.50.2 :** corrige des vulnérabilités connues. Les fichiers de modèle sont ouverts en mode défensif.
- **Recommandation pour les entreprises :** un rôle PostgreSQL dédié avec des droits minimaux (SELECT, plus INSERT, UPDATE et DELETE sur les tables t_* pour les enregistrements via EA ; jamais superutilisateur), et un chiffrement de la connexion avec vérification du certificat.

## Version 0.8 (2026-10-23)

La plus grande version à ce jour. Compass contrôle la qualité du modèle, vérifie chaque enregistrement et permet de l'annuler, compare le modèle aux exigences et au code, estime les modifications et travaille en équipe. Chaque fonction a un exemple de demande : tapez-la telle quelle dans la conversation, ou avec vos propres mots.

### Choix du modèle d'IA

- **Fournisseurs et modèles.** Anthropic (Claude) ou OpenAI, n'importe quel modèle (dans la liste ou par son identifiant) et le niveau d'effort, de « par défaut du modèle » à max. Où : Paramètres → carte « Modèle d'IA ».
  - Par défaut, rien ne change : Claude Haiku 5.5.
  - Chaque fournisseur a sa propre clé dans le Gestionnaire d'identification Windows ; les données ne vont qu'au fournisseur choisi.
- **Tout point d'accès compatible avec l'API OpenAI.** Azure OpenAI, Ollama, LM Studio ou une passerelle d'entreprise. Avec un modèle local, rien ne quitte votre ordinateur.

### Qualité du modèle et règles d'équipe

- **Contrôle du modèle.** Fondé sur des règles, sans que l'IA devine. Chaque constat a une gravité : erreur, avertissement ou information. Il trouve :
  - les noms par défaut et vides, les éléments sans description, les éléments orphelins ;
  - les noms et relations en double, les relations vers des éléments inexistants ;
  - les paquetages et diagrammes vides, les flux contraires aux règles UML ou BPMN.
  - Exemple : « Contrôle la qualité du paquetage Domaine et liste les principaux problèmes. »
- **Règles de l'organisation.** Dans governance.json à côté des paramètres, ou depuis le hub d'équipe : conventions de nommage, tagged values obligatoires, relations autorisées entre types (métamodèle / MDG).
  - Exemple : « Vérifie que tous les composants du paquetage Systèmes ont la tagged value Owner. »
- **Analyse des écarts existant / cible.** Ce qui est nouveau, ce qui est à retirer et ce qui a changé (stéréotype, description, relations) ; vers Excel sur demande.
  - Exemple : « Compare le paquetage Existant avec le paquetage Cible et enregistre les écarts dans Excel. »
- **Orthographe dans la langue de Compass.** Les notes des éléments et des diagrammes, les étapes des scénarios et le glossaire du projet sont vérifiés avec le correcteur orthographique de Windows. Les noms d'éléments et les termes du glossaire comptent comme mots connus ; corrections seulement après votre accord.
  - Enregistrer la langue dans les paramètres règle aussi le correcteur d'EA sur cette langue, si EA a un dictionnaire pour elle.
  - Langue absente : Paramètres Windows → Heure et langue → Langue → ajouter la langue.
  - Exemple : « Vérifie l'orthographe des notes dans tout le modèle et corrige les fautes. »

### Des enregistrements fiables

- **Aperçu « avant → après ».** La fenêtre de validation montre ce qui est créé, quelle valeur devient laquelle et ce qui est supprimé.
- **Vérification après chaque enregistrement.** Compass relit le modèle et le compare à la demande. Si quelque chose diffère, l'assistant le dit au lieu d'annoncer « terminé ». Il vérifie que :
  - les objets créés existent, les relations ont les bonnes extrémités, les éléments sont dans le paquetage demandé ;
  - les noms, descriptions, stéréotypes et tagged values ont les nouvelles valeurs ;
  - les objets supprimés ont disparu, les fichiers XMI et les rapports ont été écrits.
- **Tout ou rien.** Si un enregistrement échoue à mi-chemin, les objets qu'il avait déjà créés sont supprimés.
- **Suppression sûre.** Supprimer un élément qui a des relations demande toujours votre accord (même en mode auto), et la fenêtre liste ces relations.
- **Machine à remonter le temps.** Chaque enregistrement entre dans un journal d'annulation avec les opérations inverses ; ce qui a été créé ou modifié peut être annulé. Compass n'annule pas une suppression : avant elle, il crée une baseline EA du paquetage, que vous restaurez dans EA. Compass demande toujours avant d'annuler.
  - Exemples : « Annule la dernière modification. », « Montre l'historique des modifications que tu as faites dans ce modèle. », « Annule les deux dernières modifications. »

### Exigences, code et estimation

- **Exigences face au modèle.** Un document Word, du texte ou du Markdown, une page Confluence ou un texte collé dans la conversation devient une matrice de traçabilité : couvertes, probablement couvertes, non couvertes, « même identifiant, contenu différent », et les éléments du modèle qu'aucune exigence ne demande. Les relations de traçabilité (trace) ne sont créées qu'après votre accord.
  - Exemple : « Compare les exigences de C:\Projets\BRD.docx avec le modèle : qu'est-ce qui est couvert et qu'est-ce qui manque ? », puis « Ajoute des relations trace pour les exigences couvertes. »
- **Modèle face au code.** Un dossier de code, en lecture seule : classes, interfaces, enums et records avec leurs champs (Java, Kotlin, C#, TypeScript, Rust, Python), tables SQL avec leurs colonnes, opérations d'API (OpenAPI operationId). Le rapport montre ce qui manque dans le modèle, ce qui n'existe plus dans le code et en quoi les champs diffèrent.
  - Exemple : « Compare le paquetage Domaine avec le code dans C:\repo\boutique – que manque-t-il dans le modèle et que manque-t-il dans le code ? »
- **Estimation d'une modification.** L'impact d'une modification devient une liste de tâches en langage simple (modifier, adapter, vérifier) avec des heures selon le type d'élément et le niveau de relation, la mise à jour des diagrammes, les tests, l'analyse, la documentation et une marge de risque ; vers Excel sur demande.
  - Les normes sont dans estimate.json à côté des paramètres. C'est une façon d'estimer parmi d'autres : adaptez les normes à votre équipe.
  - Exemple : « Estime une modification de la classe Commande : qu'est-ce qui doit changer et combien de temps cela prendra-t-il ? »

### Nouvelles opérations sur le modèle

- **Disposition automatique des diagrammes.** Vers le bas, la droite, le haut ou la gauche, avec l'espacement choisi.
  - Exemple : « Dispose le diagramme Modèle du domaine de gauche à droite. »
- **Export et import XMI d'un paquetage.** L'import attribue de nouveaux GUID, sauf si vous demandez de les conserver.
  - Exemple : « Exporte le paquetage Domaine vers C:\Export\domaine.xml. »
- **Documents à partir des modèles de documents EA.** DOCX, PDF ou RTF, comme avec Publish → Report Builder.
  - Exemple : « Génère un rapport à partir du modèle Model Report pour le paquetage Domaine dans C:\Rapports\domaine.docx. »
- **Plus d'édition.** Couloirs (swimlanes) des diagrammes, documents liés des éléments, inversion d'une relation, données du paquetage (nom, description, version, statut, stéréotype). L'agrégation et la composition sont créées comme de vraies relations UML.
  - Exemples : « Ajoute au diagramme Processus de commande les couloirs Ventes et Entrepôt. », « Inverse la relation entre Client et Commande. »
- **Scénarios structurés.** Scénarios des éléments avec des étapes de l'acteur et du système.
  - Exemple : « Ajoute au cas d'utilisation Passer une commande un scénario principal en cinq étapes. »

### Scripts EA

- **Lire, enregistrer et exécuter des scripts.** L'assistant liste les groupes et les scripts et lit un script. Il enregistre du JScript, du VBScript et du JavaScript (un groupe manquant est créé ; validé comme tout enregistrement) et exécute un script chaque fois après votre accord, dans la fenêtre EA ouverte ou en arrière-plan.
  - Repository et Session.Output fonctionnent comme dans EA ; les inclusions « !INC Groupe.Script » sont résolues depuis le modèle.
  - Exemple : « Crée et exécute un script EA en JScript qui affiche les noms de tous les paquetages du modèle. »

### Travail en équipe : Compass Hub

- **Un service d'équipe facultatif.** `LoonarCompass.exe --hub hub.json` sur un ordinateur ou un serveur d'équipe. hub.json contient l'adresse d'écoute, les modèles de l'équipe (bases et fichiers EA), les utilisateurs avec jetons et rôles (viewer, architect, admin) et la politique.
  - Dans Compass : Paramètres → carte « Équipe (Compass Hub) », avec l'adresse et votre jeton fournis par l'administrateur du hub.
- **Politique commune.** Les règles du contrôle du modèle et le mode de validation minimal s'appliquent sur tous les postes ; chaque enregistrement va dans le journal d'audit central ; notes d'équipe partagées.
- **Lots de modifications à revoir.** Au lieu d'enregistrer, l'assistant propose la modification au hub. Un second architecte la commente et l'approuve dans la vue web (principe des quatre yeux) ; l'auteur applique ensuite le lot approuvé, avec vérification.
  - Exemples : « Propose à l'équipe une modification à revoir : ajoute la classe Entrepôt au paquetage Domaine. », « Montre les lots de modifications approuvés et applique le lot 3. »
- **Une vue web pour les collègues sans EA.** L'adresse du hub dans un navigateur, avec les onglets Ask, Search, Model check, Gaps, Audit trail, Team notes et Change sets. Dans « Ask », une partie prenante pose une question sur le modèle et l'assistant répond en lecture seule.
- **Le serveur MCP du hub.** En lecture seule, pour d'autres outils d'IA : sources, arborescence des paquetages, recherche et détails des éléments, analyse d'impact, contrôle du modèle et analyse des écarts.

### Jira et vos propres serveurs MCP

- **Jira (Cloud et Data Center).** Tickets par clé ou recherche JQL comme contexte pour le modèle ; après votre accord, nouveaux tickets, commentaires et liens vers les éléments. Où : Paramètres → carte « Jira ».
  - Exemples : « Lis le ticket SHOP-123 et vérifie quels éléments du modèle il concerne. », « Crée dans Jira un ticket pour la description manquante de la classe Facture. »
- **Vos propres serveurs MCP.** Carte « Serveurs MCP », enregistrés dans mcp_servers.json : un programme local (stdio) ou HTTP ; jetons depuis ${secret:nom} (Gestionnaire d'identification) ou ${env:NOM}.
  - Un outil qui n'est pas en lecture seule demande votre accord avant chaque appel, sauf si son serveur a "trusted": true.
  - « Enregistrer et tester les serveurs MCP » affiche le nombre d'outils de chaque serveur.
  - Exemple : « Par le serveur MCP hub, liste les sources de modèles de l'équipe. »

### Compass dans Enterprise Architect

- **Contrôle en continu.** Une relation nouvellement dessinée et un élément modifié sont vérifiés aussitôt (règles et orthographe des notes) ; les remarques apparaissent dans l'onglet « Loonar Compass » de la fenêtre System Output.
- **Agents d'événements.** Carte « Agents d'événements (Compass dans EA) », enregistrés dans event_agents.json : quand un élément est créé ou modifié, ou qu'une relation ou un diagramme est créé, l'assistant fait ce que dit la règle.
  - Il lit le modèle et répond dans System Output (lignes commençant par 🤖) ; il ne modifie jamais le modèle de lui-même ; le même agent et le même objet au plus une fois par minute.
  - Exemple de règle pour un nouveau composant : « vérifie qu'il a une description et un responsable, et propose les relations manquantes ».
- **Renommer une conversation.** Le crayon à côté de la liste des conversations.
- **Lecture d'un fichier de modèle ouvert dans EA.** Les lectures passent par la fenêtre EA et voient ainsi les dernières modifications avant qu'EA ne les écrive sur le disque.

### Autres modifications et corrections

- Cartes des paramètres sans numéros ; la première carte est « Modèle d'IA ».
- Les messages de progression disent « L'assistant … » au lieu de « Claude … ».
- Modèles en fichier : après chaque enregistrement, les modifications sont aussitôt sur le disque, et les lectures suivantes voient le nouvel état.
- Les boîtes d'erreur d'EA (SQL, XML) ne bloquent plus le travail en arrière-plan ; les requêtes avec des expressions et des noms de colonnes répétés renvoient des résultats corrects.
- Guide de l'utilisateur en 4 langues : un nouveau chapitre de cas d'utilisation avec des demandes prêtes à l'emploi, le chapitre « Compass dans Enterprise Architect » et les cartes « Modèle d'IA », « Jira », « Équipe », « Serveurs MCP » et « Agents d'événements ».

## Version 0.7.5 (2026-10-22)

- Correction : Compass dans EA remplit tout son panneau et suit chaque changement de taille, y compris dans une fenêtre flottante et un groupe d'onglets.
- « À propos » et le pied de page affichent le numéro de version complet (p. ex. 0.7.5).

## Version 0.7.4 (2026-10-21)

- Correction : Compass dans EA suit de nouveau chaque changement de taille de son panneau (en 0.7.3, il gardait la taille d'ouverture).

## Version 0.7.3 (2026-10-21)

- Compass dans EA : la saisie fonctionne après « Ajouter à la conversation Compass », après un clic dans le navigateur ou un diagramme et après un changement de panneau (le clavier va à la fenêtre Compass).
- La fenêtre Compass tient dans la partie visible du panneau EA : la barre d'icônes du bas reste à l'écran.
- Une requête erronée de l'assistant vers une base ouverte dans EA ne bloque plus EA par une boîte d'erreur : Compass la ferme et transmet l'erreur à l'assistant, qui corrige la requête.

## Version 0.7.2 (2026-10-21)

- Compass dans EA fonctionne avec toute base ouverte dans EA, même absente des paramètres : il lit le modèle via la connexion d'EA lui-même, sans mot de passe ni configuration.
- La fenêtre Compass s'ancre toujours dans le panneau EA (auparavant, elle restait parfois à côté comme fenêtre séparée).

## Version 0.7.1 (2026-10-21)

- Glossaire du projet EA : l'assistant ajoute un terme avec sa définition et son type (par défaut « Slownik ») ; un terme polonais est ajouté sous toutes ses formes (singulier et pluriel, du nominatif au vocatif), pour qu'EA surligne chaque forme dans les notes. Dans le champ de message : clic droit sur un mot → « Ajouter au glossaire du projet EA », ou demande dans la conversation.
- Compass dans EA suit la largeur de son panneau : le texte revient à la ligne au lieu de disparaître à droite.
- Menu contextuel d'un élément, paquetage ou diagramme : « Ajouter à la conversation Compass » de nouveau directement sous Specialize.
- Le clavier fonctionne après un changement de panneau EA (p. ex. Notes → Compass) ; Compass se ferme avec EA.

## Version 0.7 (2026-10-21)

Compass dans Enterprise Architect :

- Complément EA : le bouton « Loonar Compass » du ruban Start (groupe Add-Ins) ouvre Compass dans une fenêtre ancrée d'EA, au même endroit que Properties (EA retient où vous la placez). Le programme d'installation propose « Ajouter Loonar Compass à Enterprise Architect » ; cela fonctionne avec toute installation d'EA, y compris une version d'essai.
- System Output a un onglet « Loonar Compass » : vos messages et les étapes de l'assistant (lectures du modèle, écritures dans EA, résultats) avec l'heure. Le chat garde une ligne avec la dernière étape.
- Dans EA, Compass travaille toujours sur le projet ouvert dans cette fenêtre EA (fichier ou base) : aucun modèle à choisir, rien à confirmer. La liste ne montre que les conversations de ce projet ; ouvrir un autre projet dans EA bascule Compass. Sans projet ouvert, Compass ne fonctionne pas.
- Les écritures passent par la même fenêtre EA : les modifications apparaissent aussitôt dans le navigateur et les diagrammes.
- « Afficher le diagramme » et « Modifier le diagramme » ouvrent le diagramme dans EA, sans images dans le chat.
- Menu contextuel d'un élément, paquetage ou diagramme : « Ajouter à la conversation Compass » insère une référence dans le message, p. ex. « Paquetage : Shop (id 5) ».
- Mise en page de la fenêtre ancrée : police plus petite, conversations du projet dans une liste déroulante avec « + », icônes des vues en bas, pages défilant horizontalement et verticalement ; Ctrl+V colle images, fichiers et texte.
- Dans EA, les paramètres ne gardent que Claude, l'approbation, Confluence et Miro ; le modèle et EA viennent du projet ouvert.

Partout :

- Dictionnaire : clic droit sur un mot souligné dans le champ de message → « Ajouter au dictionnaire ». Une expression polonaise entre dans le dictionnaire Windows sous toutes ses formes (singulier et pluriel, du nominatif au vocatif).

## Version 0.6.5 (2026-10-20)

- Le compte EA appartient au modèle, pas à l'application : le compte et le mot de passe EA se règlent dans la carte « 2. Modèle », avec le fichier ou la base. La carte « 3. Enterprise Architect » ne contient que le chemin d'EA.exe et « Enregistrer et tester l'accès à EA ».
- Fichier et base : case « La sécurité est activée dans le modèle (comptes utilisateurs EA) ». Sans elle, les champs du compte et du mot de passe sont désactivés et EA ouvre le modèle sans connexion.
- Nouvelle option « Le modèle exige un verrou pour modifier (Require User Lock to Edit) » : avec elle, l'assistant verrouille les paquetages et éléments qu'il modifie, puis libère les verrous ; sans elle, il ne verrouille pas.
- Le test des paramètres du modèle et du modèle d'une conversation enregistre un paquetage de test dans le modèle, le vérifie et le supprime aussitôt.
- Une nouvelle conversation commence par la confirmation de son modèle (« Confirmer le modèle et commencer »). Le modèle est ensuite fixé et affiché en lecture seule ; changer le modèle par défaut dans les paramètres ne modifie pas les conversations en cours.
- Correction : une conversation apparaît dans la liste dès son premier message. Avant, une conversation où l'assistant travaillait encore manquait dans la liste.
- Correction : les partitions (couloirs) d'un diagramme d'activité plus hautes que larges sont dessinées verticalement dans EA, avec le nom en haut.
- Correction : l'aperçu d'un diagramme ouvert dans la fenêtre EA montre l'état après la dernière écriture (EA recharge le diagramme avant d'exporter l'image).

## Version 0.6.4 (2026-10-19)

- Dessin d'après une image : l'assistant reproduit la disposition de la pièce jointe (rangées, ordre, position par rapport aux voisins, tracé des lignes) au lieu de réorganiser le diagramme ; il mesure d'abord l'image, garde au moins 40 px entre les éléments, puis vérifie d'abord chaque relation, ensuite la disposition (deux séries de corrections au plus).
- Un style de ligne pour tout le diagramme s'applique après la création des relations et conserve les lignes tracées à la main.
- Un nœud d'objet (ObjectNode) d'un diagramme d'activité est enregistré comme objet EA au lieu d'interrompre l'enregistrement.
- Les courtes notes de l'assistant entre les étapes sont dans la langue de la conversation.

## Version 0.6.3 (2026-10-19)

- Éditeur de diagrammes : l'enregistrement dans EA conserve les lignes exactement comme l'éditeur les affiche (segments horizontaux et verticaux, points de contact avec les éléments), aussi pour les nouvelles relations. Avant, EA ne recevait que les points déplacés à la main et traçait les lignes en biais.

## Version 0.6.2 (2026-10-19)

- Correction : une bifurcation/jonction (fork/join) d'un diagramme d'activité EA était dessinée horizontalement alors qu'elle devait être verticale. L'application fixe maintenant l'orientation d'après la forme (plus haute que large = verticale).
- Éditeur de diagrammes : « Bifurcation / jonction » dans la boîte à outils du diagramme d'activité.

## Version 0.6.1 (2026-10-19)

- Correction : l'éditeur de diagrammes n'ouvrait pas les diagrammes d'un modèle dans une base PostgreSQL (erreur « column \"default\" does not exist »). Les modèles en fichiers .qea/.qeax fonctionnaient.

## Version 0.6 (2026-10-18)

Éditeur de diagrammes :

- Chaque diagramme d'EA dans la conversation a un bouton « Modifier le diagramme ». Il ouvre le diagramme dans une fenêtre d'éditeur (WebView2) : déplacer, redimensionner, aligner, renommer, annuler (Ctrl+Z / Ctrl+Y). L'éditeur modifie le modèle, pas une image : l'enregistrement passe par EA, avec le panneau d'approbation et le journal des modifications.
- De nouveaux éléments depuis une boîte à outils adaptée au type de diagramme : classes, composants, cas d'utilisation, activités, BPMN, machine à états (états, début, fin, choix), déploiement (nœuds, équipements, environnements d'exécution, artefacts), paquetages (de vrais paquetages) et séquence. Un nouvel élément va à un emplacement libre ; on relie en faisant glisser d'un élément à un autre.
- Relier : un élément sous la souris affiche une flèche bleue ; faites-la glisser vers un autre élément. Le type de relation vient de la liste « Connecteur ».
- BPMN : types de tâches (utilisateur, service, manuelle, script, envoi, réception, règle métier), d'événements (message, minuterie, signal, erreur…) et de passerelles (exclusive, inclusive, parallèle, complexe, basée sur les événements) sous forme d'icônes et de marqueurs, choix du type dans l'onglet « Général », un événement intermédiaire dans la boîte à outils. Les classes s'agrandissent seules quand des attributs et des opérations s'ajoutent.
- « Depuis le modèle… » : place dans le diagramme des éléments qui existent déjà dans le modèle, avec une recherche.
- La suppression demande : de ce diagramme ou du modèle. Pour une suppression du modèle, elle indique dans combien et dans quels autres diagrammes figure l'élément et combien de relations il a.
- Panneau « Propriétés » : les champs de l'onglet général d'EA (nom, alias, stéréotype, mots-clés, statut, version, auteur, phase, complexité, abstrait, notes ; paquetage, GUID et dates en lecture seule), attributs et opérations des classes, valeurs étiquetées (avec les listes du profil, p. ex. BPMN), apparence dans le diagramme (couleur, style de ligne). Chaque champ porte la marque « modèle » ou « diagramme ».
- Relations : nom, stéréotype, direction, multiplicités et rôles des deux extrémités, notes, valeurs étiquetées.
- Diagrammes de séquence : lignes de vie, messages dans leur ordre d'EA (réordonnés avec ▲ ▼ ou Alt+↑ / Alt+↓), messages synchrones, asynchrones et réponses, un nom tiré d'une opération du destinataire.
- « Réorganiser avec l'IA » : enregistre les modifications et demande à l'assistant, dans la conversation, de disposer le diagramme pour qu'il se lise bien.
- Nouvelles opérations d'écriture pour l'assistant : modifier les relations, modifier et supprimer les attributs et opérations, le sous-type de l'élément (pseudo-états), les messages de séquence avec leur numéro, leur type et l'indicateur de réponse ; retirer une valeur étiquetée.

Miro (d'EA vers Miro) :

- Carte de paramètres « 7. Miro » : jeton d'accès (dans le Gestionnaire d'identification de Windows) et test de connexion.
- « Envoyer vers Miro » sous un diagramme : choix du tableau, miniature du tableau avec l'emplacement du diagramme (l'application trouve un emplacement libre ; le cadre peut être déplacé), envoi sous forme de formes et de connecteurs modifiables dans un cadre portant le nom du diagramme. Diagrammes de séquence : lignes de vie et messages numérotés dans leur ordre d'EA.

Documentation :

- Nouveaux chapitres du guide : « 9. Éditeur de diagrammes » et « 10. Envoyer des diagrammes vers Miro » (les chapitres suivants sont décalés de deux) ; sur le site, une icône de documentation dans l'en-tête sur téléphone.

## Version 0.5 (2026-10-17)

Un modèle par conversation :

- Chaque conversation a son propre modèle. L'en-tête de la conversation l'affiche (« Modèle : … ») ; un clic ouvre le modèle de la conversation, où vous pouvez le vérifier ou le modifier.
- Une nouvelle conversation indique quel modèle elle utilisera : celui par défaut des paramètres. « Choisir un autre modèle… » permet de choisir une autre base PostgreSQL (serveur, port, base, compte, mot de passe, chiffrement) ou un fichier EA (.qea, .qeax), avec son compte EA, et de tester la connexion.
- Les mots de passe d'une connexion différente de celle des paramètres sont conservés dans le Gestionnaire d'identification Windows ; la désinstallation les supprime avec les autres secrets si vous le choisissez.
- Déposer un fichier .qea ou .qeax sur la fenêtre en fait le modèle de la conversation (dans une conversation qui contient des messages, une nouvelle conversation commence).
- Les conversations existantes reçoivent une copie du modèle des paramètres et continuent donc à l'utiliser même si les paramètres changent plus tard.
- La mémoire de l'assistant reste commune à tous les modèles. Une note ne concerne un seul modèle que si vous le demandez (« retiens cela uniquement pour ce modèle »).

## Version 0.4 (2026-10-17)

Modèles dans des fichiers :

- Le modèle peut désormais être un fichier de projet Enterprise Architect .qea ou .qeax : Paramètres → « 2. Modèle » → « Fichier EA (.qea, .qeax) » et le chemin du fichier. Aucun serveur de base de données, compte ni mot de passe n'est nécessaire.
- L'assistant lit le fichier directement et en lecture seule. Les modifications sont enregistrées par EA : via votre fenêtre EA si le fichier y est ouvert (un second EA n'ouvre jamais le même fichier), sinon par EA en arrière-plan.
- La sécurité utilisateur est facultative dans un fichier : sans elle, le compte et le mot de passe EA restent vides. Un fichier avec la sécurité activée fonctionne comme un modèle en base, avec un compte EA.
- Le programme d'installation propose le même choix sur la page de configuration.
- L'assistant écrit ses requêtes dans le SQL du support du modèle (SQLite pour les fichiers, PostgreSQL pour les bases).

## Version 0.3 (2026-10-17)

Nouveau nom et publication :

- Nouveau nom : Loonar Compass (auparavant Loonar EA Assistant). Les paramètres, les conversations, la mémoire, les modèles et les mots de passe de la version précédente sont repris automatiquement au premier démarrage ; les fichiers aux noms polonais du dossier des données ont reçu des noms anglais (`audit_log.jsonl`, `confluence_pages.json`).
- Le code source est public sur GitHub (github.com/loonar-eu/loonar-compass), et le site loonar.eu présente l'application en quatre langues, avec une foire aux questions (FAQ).
- Nouvelle licence freeware (la version polonaise fait foi, traductions EN/DE/FR) : utilisation y compris commerciale, sans limite de durée ni de territoire, avec des règles concernant les résultats de l'IA, les données et les composants open source. La liste des licences des composants open source se trouve dans le fichier THIRD-PARTY-NOTICES.txt et dans la vue « À propos ».
- Vue « Soutenir le projet » (icône de cœur dans la barre de navigation) : soutien volontaire via suppi.pl/loonar.
- Politique de confidentialité et mentions légales sur loonar.eu ; dans l'application, de courtes informations à côté de la clé API (ce qui est envoyé à Anthropic), dans le signalement de bug et pour la newsletter, et, sous le champ de message, un rappel que les réponses sont préparées par un modèle d'IA.
- Nouvel arrière-plan de l'écran de démarrage et du bandeau du programme d'installation (carte du ciel avec la grille d'un globe et une rose des vents).

Sécurité et confidentialité :

- Le mot de passe de la base se trouve dans le Gestionnaire d'identification Windows, comme la clé API, le mot de passe du compte EA et le jeton Confluence ; un mot de passe enregistré auparavant dans le fichier de paramètres y est transféré automatiquement.
- Chiffrement de la connexion à la base du modèle (TLS), carte « 2. Base de données du modèle » : préféré (par défaut), obligatoire, ou obligatoire avec vérification du certificat dans le magasin de certificats Windows. Le test de connexion indique si la connexion est chiffrée et met en garde contre un compte superutilisateur.
- Confluence uniquement via HTTPS (http:// seulement pour un serveur situé sur le même ordinateur). Avant la publication, l'assistant vérifie si une page portant ce titre existe déjà et indique explicitement que son contenu sera remplacé.
- La suppression et le déplacement d'objets ainsi que la copie de paquetages nécessitent une validation distincte, y compris en mode plan, et le panneau de validation indique les noms des objets. L'enregistrement dans la mémoire de l'assistant et le remplacement d'un modèle de document passent par la validation, comme les modifications du modèle.
- L'assistant traite le contenu des pièces jointes, des documents et du modèle comme des données, et non comme des instructions. Une image dessinée par l'assistant ne peut charger aucun fichier depuis le disque ni depuis le réseau, et sa taille est limitée.
- Lecture de la base : limite de durée des requêtes et blocage des fonctions du serveur qui lisent des fichiers ou agissent sur d'autres sessions. La chaîne de connexion de la fenêtre EA ouverte, qui contient les identifiants, n'est pas transmise au modèle Claude.
- Les programmes système et les bibliothèques sont lancés uniquement depuis le dossier Windows, jamais depuis le dossier des téléchargements.

Installation :

- Installation sans droits d'administrateur : par défaut pour l'utilisateur actuel uniquement (`%LOCALAPPDATA%\Programs\Loonar Compass`) ; l'installation pour tous les utilisateurs demande des droits d'administrateur. Les paramètres et les mots de passe sont toujours enregistrés dans le profil de l'utilisateur connecté.
- La version précédente (Loonar EA Assistant) installée pour vous uniquement est supprimée pendant l'installation ; celle installée pour tous les utilisateurs peut être supprimée avec un bouton du programme d'installation.
- La désinstallation supprime, sur demande, tout le dossier des données ainsi que tous les secrets du Gestionnaire d'identification (y compris le jeton Confluence et le mot de passe de la base).
- La fenêtre du programme d'installation tient sur les petits écrans et avec une mise à l'échelle élevée ; le raccourci « Lancement rapide », le champ « Modèle » et le champ Workspace ID ont été supprimés.

Stabilité :

- Les requêtes à l'API Claude sont répétées en cas de surcharge, de limite de requêtes et d'erreurs temporaires du serveur ; une clé API erronée et un blocage par un proxy donnent des messages clairs.
- Proxy issu des paramètres Internet de Windows (en plus de la variable HTTPS_PROXY).
- En cas d'erreur inattendue, l'application affiche un message et enregistre les détails dans un fichier au lieu de disparaître ; une erreur dans le fichier de paramètres est décrite au lieu de « paramètres absents ».
- Correction de la fermeture intempestive de l'application avec de longues légendes d'images contenant des lettres non ASCII sur les pages Confluence.
- L'application et le programme d'installation démarrent aussi sur les anciennes éditions de Windows 10 et sans le paquet Visual C++ Redistributable.

Langues :

- Quatre langues : polonais, anglais, allemand et français. Le programme d'installation commence par le choix de la langue (anglais par défaut), qui définit la langue de l'application, des réponses de l'assistant et des modèles de documents par défaut. Vous pouvez changer de langue dans les Paramètres (carte « 4. Langue et validation des modifications »).
- Tout ce qui est disponible sans IA existe en quatre langues : les fenêtres de l'application et du programme d'installation, les modèles de documents, la licence, le guide de l'utilisateur et la liste des modifications.
- L'assistant répond dans la langue de l'application, sauf si vous lui écrivez dans une autre langue.
- Après un changement de langue, les modèles par défaut non modifiés passent immédiatement à la nouvelle langue, tandis que la mémoire de l'assistant ainsi que vos modèles personnels et modifiés sont traduits par Claude. Sans connexion à Claude, un point d'exclamation orange avec une explication dans l'info-bulle apparaît sur les icônes Paramètres, Mémoire et Modèles, et la traduction se termine d'elle-même dès que la connexion fonctionne (ou après « Traduire maintenant »). L'historique des conversations n'est jamais traduit.
- Nouveau logo, plus simple (plat, bicolore, lisible en petite taille et adapté à l'impression), dans l'icône de l'application et du programme d'installation ainsi que dans la vue « À propos ».

Documentation pour Confluence :

- Pages Confluence préparées par l'assistant à partir du modèle et de la conversation : code de la page au format de l'éditeur de source (storage XHTML) avec titres, tableaux, listes, table des matières, panneaux (info, remarque, avertissement, astuce, section dépliable), statuts, tickets Jira, mentions de personnes et blocs de code ; diagrammes d'EA et images en pièces jointes. Dans le chat, une carte de page avec les boutons « Copier le code de la page » et « Ouvrir le dossier des images ».
- Publication dans Confluence via l'API REST (sur demande, toujours après validation, y compris en mode auto) : nouvelle page sous une page parente ou mise à jour d'une page existante (par ID ou par titre), images en pièces jointes (les existantes reçoivent une nouvelle version). Les diagrammes sont réexportés à chaque publication, de sorte qu'une nouvelle publication actualise la page après des modifications du modèle ; l'assistant mémorise les pages publiées.
- Paramètres, carte « 6. Confluence » : adresse, jeton (Data Center : jeton d'accès personnel ; Cloud : e-mail et jeton API), clé de l'espace et page parente, avec le bouton « Enregistrer et tester la connexion à Confluence ».

Modèles de documents :

- Modèles dans le dossier `templates` à côté de l'historique des conversations, que l'assistant remplit avec le contenu du modèle EA (page Confluence ou document Word) : Analyse de faisabilité préliminaire, BRD - Exigences métier, Description des modifications du système, Description du processus. Les modèles sont des fichiers Markdown contenant des instructions pour l'assistant ; vous pouvez les modifier, en ajouter de nouveaux ou demander à l'assistant de créer un modèle à partir d'un document exemple.
- Vue « Modèles de documents » dans la barre de navigation : liste avec description, « Utiliser » (insère dans le chat une demande de préparation du document), « Modifier » et « Ouvrir le dossier des modèles ».

## Version 0.2 (2026-10-15)

Mémoire :

- Mémoire entre les conversations : l'assistant enregistre les points convenus (conventions de modélisation, préférences, corrections, faits sur les projets, tout ce qui suit « retiens… ») sous forme de fichiers Markdown locaux et les connaît dans chaque nouvelle conversation. Un point sur le même sujet est mis à jour au lieu d'être dupliqué ; les points obsolètes sont supprimés. L'enregistrement d'un point en cours de réponse n'interrompt plus la suite du travail (auparavant, l'API renvoyait une erreur 400 concernant la signature du bloc « thinking »).
- Vue « Mémoire de l'assistant » dans la barre de navigation : liste des points avec leur date, boutons « Supprimer » et « Ouvrir le dossier de la mémoire » (dossier `memory` à côté de l'historique des conversations, avec l'index `MEMORY.md`).

Enterprise Architect :

- Tracé d'un connecteur selon un itinéraire donné, par exemple d'après un croquis sur une capture d'écran : points de passage et points de contact avec les côtés des éléments (lignes uniquement horizontales et verticales, ou libres).
- Modification du tracé des connecteurs sur un diagramme (comme Line Style dans EA) : direct, automatique, ligne libre, orthogonal carré, orthogonal arrondi, arbre et latéral ; pour les connecteurs sélectionnés ou pour tous ceux du diagramme.
- Analyse d'impact d'une modification : à partir d'un élément donné, en profondeur le long des relations (jusqu'à 6 niveaux, dans un sens ou dans les deux, éventuellement uniquement pour certains types de relations), avec la relation par laquelle l'impact arrive, le chemin et les diagrammes sur lesquels figurent les éléments concernés ; résultat sous forme de description, d'image, de document ou de fichier Excel.
- Où sont utilisées les opérations (services) d'une classe, d'un composant ou d'une interface : diagrammes de séquence (messages vers l'élément ou sa ligne de vie, liés à l'opération ou portant un nom correspondant) et d'activité (actions appelant l'opération), avec le paquetage et l'appelant, ainsi que la liste des opérations qui ne sont utilisées sur aucun diagramme.
- Attributs et opérations des classes (avec paramètres), copie d'un paquetage avec tout son contenu et baseline (version enregistrée) d'un paquetage. Avant une suppression ou une restructuration importante d'un paquetage existant, l'assistant doit créer une baseline (c'est une consigne donnée à l'assistant, pas un verrou : pour les paquetages importants, demandez explicitement une baseline) ; elle se restaure dans EA (Manage Baselines → Compare/Restore), car la restauration par automatisation fige EA.
- Collaboration avec la fenêtre EA ouverte (même modèle) : l'assistant sait quel diagramme vous avez ouvert et ce qui y est sélectionné (y compris dans le navigateur de projet), ouvre des diagrammes et indique des éléments dans le navigateur, et, après un enregistrement, actualise lui-même les diagrammes et paquetages modifiés dans votre fenêtre EA, sans F5.
- Journal des modifications : chaque enregistrement de l'assistant dans le modèle (y compris les échecs) est consigné dans le fichier `dziennik_zmian.jsonl` à côté de l'historique des conversations ; vue « Journal des modifications » dans la barre de navigation avec le résultat, l'heure, le compte et la base, et export vers Excel.
- Mots-clés des éléments (champ Keywords dans EA) lors de la création et de la modification.
- Légende de diagramme avec coloration automatique, comme dans EA : filtre sur une propriété de l'élément (mot-clé par défaut, mais aussi stéréotype, statut, type) et/ou du connecteur (par exemple stéréotype), entrées avec valeur, couleur et libellé.
- Modification du nom, de la description et du stéréotype d'un diagramme existant.
- Masquage de connecteurs sélectionnés sur un diagramme (et réaffichage) : les relations restent dans le modèle et sur les autres diagrammes, ce qui permet d'alléger un diagramme chargé sans supprimer de relations.

- Modification de la position et de la taille d'un élément déjà présent sur un diagramme (par exemple agrandissement d'un cadre, d'un pool ou d'un sous-processus), sans le retirer du diagramme ; les connecteurs sont conservés.
- Liaison d'un élément (par exemple un sous-processus BPMN) à un diagramme existant en tant que diagramme composite, de sorte qu'un double-clic ouvre le diagramme. L'opération existait dans l'application, mais l'assistant ne la connaissait pas et renvoyait à une modification manuelle dans EA.
- Sous-processus BPMN intégrés (embedded) : étapes en tant qu'éléments enfants du sous-processus, dessinées dans son rectangle sur le même diagramme (comme les enregistre l'import BPMN depuis Camunda) ; en plus des sous-processus réduits, avec les étapes sur un diagramme séparé.
- Ajout d'étapes à un sous-processus existant : l'élément enfant est créé dans le paquetage du sous-processus et lié à celui-ci (comme dans la base EA). Auparavant, EA refusait cette opération avec l'erreur « Element locked », alors que le verrou était détenu par l'assistant lui-même.

Démarrage :

- La fermeture de l'application ferme aussi sa session EA. Auparavant, le processus EA restait en arrière-plan et occupait une licence, si bien que les sessions EA suivantes pouvaient attendre indéfiniment.
- La fenêtre s'ouvre aussi sur des cartes graphiques moins performantes (par exemple sous Wine via Vulkan) : l'application n'exige de la carte que ce que la carte déclare.
- Communication avec EA via COM sans la bibliothèque système propsys, et donc aussi sous Wine.

Documents et images :

- Documents Word (DOCX) créés par l'assistant : titres, paragraphes, listes, tableaux, diagrammes d'EA et images dessinées auparavant ; dans le chat, une carte de fichier avec les boutons « Ouvrir » et « Enregistrer sous… ».
- Classeurs Excel (XLSX) créés par l'assistant : plusieurs feuilles, en-têtes avec filtre et première ligne figée, nombres enregistrés comme nombres (par exemple liste d'éléments, matrice, résultat d'une analyse d'impact).
- Images dessinées par l'assistant (par exemple matrice cas d'utilisation/exigences, tableau, graphique, schéma) en PNG dans le chat, avec « Enregistrer sous… » ; l'assistant voit le résultat et peut le corriger.

Contact :

- Vue « Signaler un bug » dans la barre de navigation : objet, description, images (glissées dans la fenêtre, collées depuis le presse-papiers ou choisies sur le disque, avec miniatures), e-mail facultatif pour la réponse et informations techniques (sans mots de passe ni clés). « Envoyer le signalement » ouvre dans le logiciel de messagerie Windows un message prêt à l'envoi à mariusz@loonar.eu avec les images en pièces jointes (Simple MAPI) ; si le logiciel de messagerie ne le prend pas en charge, il ouvre le message sans images ainsi que le dossier des images à glisser.
- Paramètres, carte « 5. Nouvelles versions (newsletter) » : adresse e-mail et consentement à recevoir des informations sur les nouvelles versions ; le choix est mémorisé dans les paramètres, et l'inscription comme la désinscription sont transmises à l'auteur par un message du logiciel de messagerie.

Apparence :

- Nouvelle apparence de la fenêtre, fonctions inchangées : barre de navigation bleu marine à gauche avec des icônes (Chat, Paramètres, Nouveautés, À propos) au lieu d'onglets en haut, fond clair et chaud comme dans Claude, police Segoe UI avec titres en demi-gras.
- Liste des conversations regroupée en « Aujourd'hui », « Hier », « 7 derniers jours » et « Plus ancien » ; nouvelle conversation avec le bouton « + », conversation sélectionnée mise en évidence, points d'état inchangés.
- En-tête de conversation avec le titre et l'indicateur « Claude travaille » pendant que l'assistant travaille.
- Vos messages dans une bulle beige comme dans Claude ; les réponses de l'assistant avec l'icône de l'application, dans une largeur confortable pour la lecture.
- Champ de message sous forme d'une seule carte : texte, bouton de collage, modèle et mode de validation, et boutons ronds d'envoi et d'arrêt.
- Liste des étapes sous forme de carte : dépliée pendant le travail, réduite à une ligne une fois le travail terminé (un clic sur l'en-tête la déplie).
- Écran de nouvelle conversation avec le logo et une indication de ce que vous pouvez faire.
- Paramètres sous forme de cartes blanches avec ombre ; Nouveautés et À propos avec des marges de page.

Paramètres :

- Valeurs par défaut neutres, utilisées uniquement lorsqu'aucun paramètre n'existe encore (l'installation et l'application ne modifient pas les paramètres existants) : serveur de la base, nom de la base, compte EA et mots de passe vides (avec une indication grise « p. ex. 127.0.0.1 » pour le serveur), port 5432, modèle EA « Model », Jetons max. 10000.
- Le chemin d'EA.exe peut être indiqué dans une fenêtre de sélection de fichier (bouton « … »), y compris dans le programme d'installation.
- Le verrouillage des paquetages lors de l'enregistrement est toujours activé ; l'option a disparu des paramètres.
- Lorsqu'EA refuse un enregistrement à cause d'un verrou, l'erreur indique qui verrouille l'élément, son paquetage et son diagramme (compte ou groupe, depuis quand), ou qu'il n'existe aucun verrou dans le modèle.

Installation :

- Page « Nouveautés » dans le programme d'installation, juste après l'accueil : lors d'une mise à jour, les modifications depuis la version installée ; lors d'une nouvelle installation, les modifications de la version actuelle.
- Barre de progression de l'installation et de la désinstallation (pourcentage et étape en cours) au lieu d'une roue qui tourne.

## Version 0.1 (2026-10-13)

Première version, complète : toutes les fonctions de la liste ci-dessous sont vérifiées. Application de bureau autonome pour travailler sur un modèle Enterprise Architect avec l'aide de Claude Haiku 5.5.

### Fonctionne (vérifié)

Modèle et conversation :

- Conversation avec le modèle Claude Haiku 5.5 via l'API Anthropic, avec prise en charge du Workspace ID.
- Ajout de messages pendant le travail de l'assistant : le message est mis en file d'attente et l'assistant en tient compte à l'étape suivante, sans interruption.
- Liste des étapes dans le chat pendant le travail de l'assistant : ce qu'il fait (requête à la base, portée de l'enregistrement dans EA, récupération d'un diagramme), combien de temps cela prend et le résultat de chaque étape. La liste n'est pas enregistrée dans l'historique de la conversation.
- Travail dans plusieurs conversations à la fois : pendant le travail de l'assistant, vous pouvez commencer une nouvelle conversation ou revenir à une autre, et le travail commencé se poursuit en arrière-plan (point orange à côté du nom de la conversation pendant le travail, vert une fois terminé, jusqu'à ce que vous ouvriez cette conversation). Les enregistrements dans EA provenant de différentes conversations s'exécutent l'un après l'autre.
- Bouton « Arrêter » : arrête le travail après l'étape en cours ; les étapes suivantes prévues ne sont pas exécutées.
- Modes de validation des enregistrements : ask (panneau avec la description des modifications et les boutons « Valider » / « Refuser » avant chaque enregistrement ; un enregistrement refusé ne modifie pas le modèle), plan (une seule demande de validation avant le premier enregistrement d'une réponse, les enregistrements suivants de cette réponse sans nouvelle demande) et auto (sans validation).
- L'assistant connaît l'application dans laquelle il fonctionne : il répond aux questions sur ses fonctions et ses limites en s'appuyant sur ce changelog.
- L'assistant désigne les objets du modèle par leur nom, leur paquetage et leur GUID, sans numéros internes de la base.

Enterprise Architect :

- Lecture du modèle EA depuis une base PostgreSQL (paquetages, éléments, diagrammes, connecteurs). La session de base de données est en lecture seule. Un port de base de données autre que 5432 fonctionne aussi lors de l'enregistrement via EA.
- Enregistrement dans le modèle EA via COM : l'assistant crée des paquetages, des diagrammes, des éléments et des connecteurs, et dispose les éléments sur le diagramme. Il se connecte avec le compte EA des paramètres et lève les verrous après l'enregistrement, pour que le résultat soit immédiatement modifiable.
- Session EA permanente pour toute l'application : le premier appel à EA prend de quelques secondes à plus de dix secondes (démarrage d'EA et ouverture du modèle), les suivants moins d'une seconde. L'application communique avec EA directement via COM, sans PowerShell. La session se ferme après 10 minutes d'inactivité et à la fermeture de l'application.
- Dessin par l'assistant à la volée, sans importateurs séparés : à partir d'un fichier BPMN (éléments aux positions du fichier), d'une photo, d'un croquis ou d'une simple description.
- Types BPMN 2.0 corrects dans EA : tâches, sous-processus, événements, passerelles et flux, avec stéréotypes et tags.
- Déplacement d'éléments, de diagrammes et de paquetages existants vers un autre paquetage ; les connecteurs sont conservés.
- Suppression dans le modèle : éléments, connecteurs, diagrammes et paquetages, et retrait d'un élément d'un diagramme.
- Couleurs de remplissage : couleur par défaut de l'élément et couleur de l'élément sur un diagramme donné.
- Cellules de navigation (navigation cells) : tuiles-liens vers des diagrammes sur le diagramme de départ, avec un libellé (nom du diagramme cible) et une icône adaptée à son type.
- Récupération d'un diagramme d'EA sous forme d'image PNG (tel qu'EA le dessine) et affichage dans le chat, avec les boutons « Enregistrer sous… » et « Ouvrir en taille réelle ».
- Verrouillage optimiste des enregistrements : l'assistant peut indiquer des objets qui ne doivent pas avoir changé depuis sa lecture ; en cas de modification, l'enregistrement est refusé.

Pièces jointes :

- Glisser-déposer de fichiers dans la fenêtre, depuis l'Explorateur et depuis le navigateur (fichiers téléchargés) : jusqu'à 5 fichiers, jusqu'à 10 Mo chacun.
- Coller depuis le presse-papiers en pièce jointe : une image (par exemple une capture d'écran) ou des fichiers copiés dans l'Explorateur, avec Ctrl+V ou le bouton à côté du champ de message.
- Les PDF et les images sont transmis directement au modèle Claude, les DOCX sous forme de texte, les fichiers texte (BPMN, XML, CSV, code…) sous forme de texte.
- Les fichiers ZIP sont décompressés : les images, PDF et fichiers texte qu'ils contiennent sont transmis au modèle Claude comme s'ils étaient joints séparément (jusqu'à 20 fichiers).
- Une pièce jointe dont le fichier n'est plus sur le disque laisse une mention dans la conversation et ne bloque pas la suite du travail dans cette conversation.

Fenêtre et chat :

- Champ de message multiligne : le texte long passe à la ligne, Entrée envoie, Maj+Entrée insère une nouvelle ligne.
- La flèche vers le haut dans un champ de message vide rappelle votre dernier message de la conversation ; les appuis suivants remontent vers les précédents, la flèche vers le bas revient en avant.
- Vérification orthographique dans le champ de message avec le dictionnaire système de Windows, dans la langue des paramètres système : les mots erronés sont soulignés en rouge.
- Mise en forme des réponses de l'assistant dans le chat : gras, listes à puces et numérotées, tableaux, titres, code en ligne et blocs de code.
- Heure à côté de chaque message dans le chat (date en plus pour les messages plus anciens).
- Une conversation ouverte s'affiche à partir de la fin, c'est-à-dire des derniers messages.
- Cercle avec une flèche au-dessus du champ de message lorsque le chat a défilé vers le haut : ramène à la fin de la conversation.
- Recherche dans les conversations : par titre et par contenu des messages.
- Renommage d'une conversation : crayon à côté de la conversation sélectionnée, Entrée enregistre, Échap annule.
- Historique des conversations enregistré dans des fichiers et retour aux conversations précédentes.
- La fenêtre reste utilisable pendant le travail de l'assistant (le travail s'effectue en arrière-plan).
- Taille de fenêtre constante lors du déplacement entre des écrans ayant des mises à l'échelle différentes.
- Couleurs claires de la fenêtre, onglets « Nouveautés » et « À propos », grand logo de l'application dans l'onglet « À propos ».
- Écran d'accueil avec le visuel de la carte de visite Loonar (globe, clavier, étoiles), le nom et la version de l'application et les coordonnées du propriétaire.
- Démarrage sans fenêtre de terminal, barre avec les coordonnées du propriétaire.
- Icône de l'application dans le fichier exe : visible dans l'Explorateur et après épinglage à la barre des tâches.

Paramètres :

- Paramètres dans la fenêtre en quatre groupes encadrés : clé API, base de données du modèle, compte EA, mode de validation. La clé API et le mot de passe du compte EA sont stockés dans le Gestionnaire d'identification Windows. Le modèle Claude est affiché en lecture seule.
- Enregistrement de chaque groupe séparément (boutons « Enregistrer et tester… »), avec le résultat du test sous le groupe : clé API (requête au modèle), base (connexion et lecture), EA (connexion via COM avec le compte indiqué).

Installation :

- Programme d'installation sous forme d'assistant : licence, dossier de destination (Program Files par défaut), raccourcis (menu Démarrer, bureau, lancement rapide), configuration facultative (clé API, base, compte EA, mode de validation) et récapitulatif.
- Mise à jour en relançant le programme d'installation : les paramètres, l'historique des conversations et les pièces jointes sont conservés.
- Désinstallation depuis le menu Démarrer ou depuis les paramètres Windows ; les données utilisateur et les secrets ne sont supprimés que sur demande.
- Les paramètres, l'historique des conversations et les pièces jointes se trouvent dans le profil utilisateur (`%APPDATA%\Loonar EA Assistant`), séparément pour chaque utilisateur de l'ordinateur.
- Guide de l'utilisateur (PDF) dans le dossier d'installation et dans le menu Démarrer.

### Prérequis

- Le modèle EA est un fichier de projet .qea ou .qeax, ou se trouve dans une base PostgreSQL. Les anciens formats de fichier (.eap, .eapx, .feap) et les autres bases ne sont pas pris en charge ; EA convertit un fichier .eap/.eapx en .qea par un transfert de projet.
- Un modèle dans une base doit avoir la sécurité utilisateur activée (Enable Security, mode « Require User Lock to Edit ») et contenir un compte utilisateur pour l'assistant (nom quelconque), avec un mot de passe et des droits de modification.
- Dans un fichier de modèle, la sécurité utilisateur est facultative ; si elle est activée, le fichier a lui aussi besoin d'un tel compte.

### Limitations connues

- Le premier appel à EA après le démarrage de l'application (ou après 10 minutes d'inactivité) prend de quelques secondes à plus de dix secondes. Pendant ce temps, un processus EA tourne en arrière-plan et occupe une place de licence.
- L'assistant enregistre les grands diagrammes par lots ; pour une réponse très longue, la limite de jetons peut être insuffisante (paramètre « Jetons max. »).
- Lorsqu'une des opérations d'un enregistrement échoue, les opérations précédentes de ce même enregistrement restent dans le modèle ; le message indique ce qui a été enregistré.
- L'italique dans les réponses de l'assistant n'est pas mis en forme.
- Sur un écran dont la mise à l'échelle diffère de celle de l'écran principal, l'image de la fenêtre est légèrement moins nette.
- Lorsque la fenêtre EA n'est pas ouverte ou affiche un autre modèle, les modifications enregistrées par l'assistant sont visibles après l'actualisation du paquetage (F5).
- Les proxys avec script PAC et l'authentification NTLM/Kerberos sur le proxy ne sont pas pris en charge.
- Le chiffrement TLS défini dans les paramètres concerne la lecture du modèle par l'assistant ; la connexion d'EA lui-même à la base dépend des paramètres d'EA.
- Le programme d'installation n'est pas encore signé numériquement ; Windows SmartScreen peut donc afficher un avertissement lors du premier lancement.
