# Changelog

Toutes les modifications importantes de l'application Loonar Compass (nommée Loonar EA Assistant jusqu'à la version 0.3).

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
