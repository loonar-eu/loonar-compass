# Versionshinweise

Alle wesentlichen Änderungen an Loonar Compass (bis Version 0.3 unter dem Namen Loonar EA Assistant).

## Version 0.9.2 (2026-10-30)

- **Sichtbare Pfeile in Diagrammen:** Konnektoren, die keine Nachrichten eines Sequenzdiagramms sind, werden nicht mehr an ein einziges Diagramm gebunden (bisher erschien z. B. eine im Diagramm „Abhängige Systeme“ gezeichnete Abhängigkeit in keinem anderen). Das Einblenden eines verborgenen Konnektors entfernt seine doppelten Einträge im Diagramm, und die neue Reparatur „Konnektoren im Diagramm nicht sichtbar“ löst gebundene Konnektoren, entfernt Duplikate und blendet auf Wunsch verborgene ein. Bei fehlenden Pfeilen prüft der Assistent zuerst diese Ursachen.
- **Anhänge in Jira:** Der Assistent hängt Diagrammbilder direkt aus dem Modell sowie Bilder und Dateien der Unterhaltung an Vorgänge an – mit einem neuen Werkzeug oder gleich beim Anlegen und Ändern eines Vorgangs. In Jira Server / Data Center erscheinen die Bilder auch in der Beschreibung.
- **Scharfe Diagrammbilder in Jira und Confluence:** An Vorgänge, Seiten und Dokumente angehängte Diagramme werden in doppelter Auflösung exportiert, aus Vektoren gezeichnet (EA-Metadatei), sodass sie beim Vergrößern nicht unscharf werden. In der Beschreibung eines Jira-Server-Vorgangs hat das Bild seine volle Größe statt einer Miniatur, und eine Breite lässt sich angeben (z. B. 900 px).
- **Dokumentvorlagen neu:** beliebig viele Vorlagen, jede mit Werkzeug (Confluence, Jira, Word oder beliebig), eigener Gruppe (z. B. Dokumentationsebene oder Projekt) und dem Modell, das sie schreiben soll. Vorlagen fügen Sie aus Markdown-, Word-, HTML-, PDF- und anderen Dateien oder aus einer Confluence-Seite hinzu (Überschriften, Listen und Tabellen bleiben erhalten); Name, Beschreibung, Werkzeug, Gruppe, Modell und Inhalt legen Sie in einem Formular fest. Die Liste ist nach Werkzeug und Gruppe geordnet, mit den Schaltflächen Verwenden, Eigenschaften, Bearbeiten und Entfernen (in den Ordner bin). Compass liefert keine fertigen Vorlagen mehr (BRD, Prozessbeschreibung…): unveränderte Kopien kommen in den Ordner bin.
- **Dokumentdisziplin:** Bevor der Assistent eine Confluence-Seite oder ein Word-Dokument erstellt, listet er alle Ihre Wünsche zum Dokument aus der ganzen Unterhaltung auf und nennt für jeden den Abschnitt; er hält die Überschriften der Vorlage ein und beschreibt jede genannte Anforderung einzeln mit Nummer und Text aus der Quelle. Compass erstellt kein Dokument, in dem ein Abschnitt der Vorlage fehlt oder ein Wunsch keinen Platz hat. Die Antwort zeigt die Liste: Wunsch → Abschnitt.
- **Ein stärkeres Modell für ein Dokument:** Wenn Sie ein Dokument anfordern (oder eine Vorlage ein stärkeres Modell vorsieht), bietet Compass ein stärkeres Modell desselben Anbieters an – Claude Sonnet statt Haiku, das volle OpenAI-Modell statt mini – nur für diese Antwort und mit dem Hinweis, wie viel teurer es ist. Die nächste Antwort nutzt wieder das Modell aus den Einstellungen.
- **Bilder aus Jira in Dokumenten:** Der Assistent lädt Anhänge von Jira-Vorgängen herunter (auch aus Jira Cloud), sieht die Bilder und fügt sie in Confluence-Seiten und Word-Dokumente ein; beim Lesen eines Vorgangs werden seine Anhänge aufgelistet.
- **Klickbare Links im Chat:** Links in Antworten (z. B. zu Jira-Vorgängen und Confluence-Seiten) öffnen sich im Browser.
- **Lange Unterhaltungen günstiger und genauer:** Die Schwelle für die Zusammenfassung des früheren Teils einer Unterhaltung zählt echte Tokens (Anthropic zählt sie kostenlos) statt einer Schätzung aus Zeichen, die für polnischen Text und Werkzeugschemata zu niedrig war – Unterhaltungen überschreiten die Preisstufe von 100 000 Tokens nicht mehr.
- **Korrekturen aus den Labortests:** „Compass schließen und erneut versuchen“ im Installationsprogramm schließt Compass wirklich (auch im EA-Tab); Nutzungsstatistik: Vor der Antwort auf das Zustimmungsfenster oder nach dem Abschalten im Installationsprogramm wird nichts gesendet, die Hub-Richtlinie sperrt sie mit Begründung, die Liste des Gesendeten aktualisiert sich selbst und eine neue Kennung verwirft ungesendete Zähler; die Karte „Quellcode“ öffnet die Ordnerauswahl über dem Compass-Fenster; eine aus dem Browser kopierte Jira- oder Confluence-Adresse wird auf die Serveradresse gekürzt und ergänzt den Projekt- bzw. Bereichsschlüssel; mit Client-Zertifikat (mTLS) funktioniert die Adressprüfung, ohne Zertifikat nennt die Meldung, was fehlt; ein langes OneDrive-Token passt in die Anmeldeinformationsverwaltung; ein Upload nach Google Drive legt einen fehlenden Ordner an; ein Bericht aus einer EA-Vorlage entsteht ohne EA-Fenster und ohne Angebot eines teureren Modells; Tabellen im Chat scrollen seitlich, statt den Text umzubrechen; der Link „Veröffentlichte Statistik“ im Installationsprogramm und in der Anwendung führt zur neuen Seite loonar.eu/stats/ (Chrome warnte bisher vor seiner Adresse als Nachahmung von loonar.eu).

## Version 0.9.1 (2026-10-28)

- **Jira und Confluence mit Client-Zertifikat:** Firmenserver, die zusätzlich zum Token ein Client-Zertifikat (mTLS) verlangen, funktionieren jetzt mit Compass. Auf den Karten Jira und Confluence in den Einstellungen gibt es die Felder „Client-Zertifikat“ (Pfad zu einer .p12/.pfx- oder PEM-Datei) und „Zertifikatspasswort“ (in der Windows-Anmeldeinformationsverwaltung gespeichert). Ohne Zertifikat schloss ein solcher Server die Verbindung („Eine vorhandene Verbindung wurde vom Remotehost geschlossen“).
- **Einfacheres Installationsprogramm:** ohne die Seite „Konfiguration“ – Schlüssel, Modell und EA-Konto geben Sie in der Anwendung ein, die Sie beim ersten Start durch die Einstellungen führt.
- **Installation wiederholen:** Kann die Installation die Dateien nicht ersetzen, weil Compass läuft, bietet die Fehlerseite „Compass schließen und erneut versuchen“ und „Erneut versuchen“ – ohne von vorn zu beginnen.
- **Aus dem Browser eingefügte Jira- und Confluence-Adressen:** Der Teil einer Seite (z. B. „/spaces/…“, „/display/…“, „/browse/…“) wird abgeschnitten, und schickt der Server eine Webseite statt Daten, weist Compass auf die Adresse hin (vorher meldete die Prüfung „Verbunden“ und Seiten kamen leer zurück).
- **Der Assistent liest Quellcode:** neue Werkzeuge, nur lesend – Dateien eines Ordners, Suche im Code (Datei:Zeile) und Lesen einer Datei – in lokalen Repository-Klonen. In Einstellungen → „Quellcode“ (eine Karte, auch in EA sichtbar; die Schaltfläche „…“ fügt einen Ordner hinzu) eingetragene Ordner liest er ohne Rückfrage, bei anderen fragt er zuerst. So findet er selbst, wo eine Klasse, Operation oder Tabelle implementiert ist und was davon abhängt.
- **OneDrive und Google Drive:** Der Assistent listet Ordner auf, liest Dokumente (Word, Excel, PowerPoint, PDF, Visio, Text; in Google Drive auch Google Docs, Tabellen und Präsentationen) und lädt – jedes Mal mit Ihrer Zustimmung – von ihm erstellte Dokumente hoch. Sie melden sich einmal im Browser mit Ihrem eigenen Microsoft- oder Google-Konto an (OAuth mit PKCE); das Aktualisierungstoken liegt in der Windows-Anmeldeinformationsverwaltung. Einstellungen: Karte „OneDrive“ (App-Registrierung in Entra ID als öffentlicher Client) und Karte „Google Drive“ (OAuth-Client „Desktop-App“ in Google Cloud).
- **Neue Version und Statistik stündlich:** Compass prüft stündlich auf eine neue Version (auch wenn es lange offen ist), und die Statistik wird – wenn eingeschaltet – beim Start (Installations-ID, Version, System, Sprache) und stündlich (Zusammenfassung des Tages: Arten von Aufgaben und ihr Ergebnis, Bewertungen von Antworten, was der Assistent nicht konnte) gesendet. Unter Antworten: 👍/👎 und – wenn der Assistent etwas nicht kann – „💡 Diese Funktion anfragen“.
- **Unterhaltung in einem anderen Modell, Export und Import:** Nach dem Verschieben eines Projekts (z. B. aus einer Datenbank in eine Datei) lässt sich eine Unterhaltung mit dem neuen Modell fortsetzen. Im EA-Tab: „⋯“ → „Unterhaltungen anderer Modelle…“ (auch ein Link unter der Liste) → „Hier fortsetzen“. Es entsteht eine Kopie, das Original bleibt bei seinem Modell, und die Gedächtnisnotizen des früheren Modells können an das neue gebunden werden. Der Assistent erfährt, dass die früheren Nachrichten ein anderes Modell betrafen, und prüft die genannten Elemente zuerst. Im eigenen Fenster wechselt das Modell einer Unterhaltung wie bisher über die Schaltfläche „Modell: …“. Eine Unterhaltung mit Anhängen lässt sich in eine `.lcchat`-Datei exportieren (Menü „⋯“ in EA, Rechtsklick auf eine Unterhaltung im Fenster) und auf einem anderen Computer importieren – sie gehört dann zum dort geöffneten Modell. Die Datei enthält den Dateipfad oder Datenbank- und Servernamen, keine Konten oder Passwörter.
- **Der Assistent weiß, was er getan hat:** Die Schritte jeder Antwort (Lesen, Suchen, Speichern in EA, Confluence-Seiten) bleiben in der Unterhaltung, und der Assistent erhält sie in späteren Runden. Bisher enthielt der Verlauf nur den Text der Antworten, sodass er später bestreiten konnte, z. B. Confluence gelesen zu haben, obwohl er es getan hatte.
- **Der Assistent arbeitet selbstständig bis zum Ende:** Er erledigt die ganze Aufgabe in einer Antwort, statt alle paar Schritte den Stand zu zeigen und „weitermachen?“ zu fragen. Sinnvolle Entscheidungen (Namen, Dateien, Format, Reihenfolge) trifft er selbst und nennt sie in der Antwort; er fragt nur, wenn eine falsche Entscheidung viel im Modell ändern würde. Notizen wie „jetzt lese ich…“ stehen in den Arbeitsschritten, nicht als eigene Nachrichten im Chat. Das Limit liegt bei 40 Schritten; wird es erreicht, sagt der Assistent, was erledigt ist und was bleibt, statt mit einem Fehler zu enden. Hält er trotzdem vor dem Ende an (offene Aufgaben im Plan oder „schreiben Sie weiter“), schickt Compass ihn selbst weiter, bis zu dreimal in einer Antwort.
- **Weniger Rückfragen zu EA-Skripten:** Nach der ersten Freigabe werden weitere Skripte gleichen Aufbaus – derselbe Code mit anderen Daten, z. B. der nächste Satz Aliase – ohne Rückfrage gespeichert und ausgeführt (bis Compass geschlossen wird); anderer Code fragt wie bisher. Alias und Stereotyp von Operationen und Attributen setzt der Assistent jetzt direkt beim Schreiben, sodass Aliase (z. B. REST-Pfade) kein Skript brauchen. Die Freigabekarte eines Skripts hat auch die Schaltfläche „Alle Skripte bis zum Schließen von Compass freigeben“, und Änderungen, die ein gewöhnliches Schreiben erledigt, macht der Assistent ohne Skripte.
- **Eine Freigabe für eine Serie:** Die Freigabekarte für einen Jira-Vorgang, eine Confluence-Veröffentlichung, eine Gedächtnisnotiz, einen MCP-Werkzeugaufruf, einen Upload und ein EA-Skript hat die Schaltfläche „Alle … bis zum Schließen von Compass freigeben“. Zehn Jira-Vorgänge sind ein Klick statt zehn.
- **Stories und Use Cases unter einem Epic in Jira Server / Data Center:** Ein Vorgang mit einem Epic als übergeordnetem Vorgang wird über das Feld „Epic Link“ verknüpft (`parent` gilt dort nur für Unteraufgaben). Bisher lehnte Jira ihn mit „Issue type … is not a sub-task but a parent is specified“ ab. Jira Cloud funktioniert wie bisher.
- **Jira-Vorgänge ändern:** Der Assistent korrigiert bestehende Vorgänge – Zusammenfassung, Beschreibung und Labels (hinzufügen und entfernen), z. B. entfernt er eine unnötige Markierung aus den Akzeptanzkriterien aller Vorgänge auf einmal. Freigabe wie beim Anlegen, auch einmal für alle.
- **Geringere Kosten für das KI-Modell:** Die ganze Unterhaltung der Assistenten-Schleife – Verlauf und Werkzeugergebnisse wie gelesene Quelldateien – liegt jetzt im Anthropic-Cache, sodass jeder weitere Schritt dafür ein Zehntel des Preises zahlt. Das Änderungsprotokoll wird nicht mehr in jede Anfrage eingefügt (etwa 18.000 Tokens weniger); der Assistent liest es mit einem Werkzeug, wenn Sie nach Funktionen von Compass fragen. Im Codeanalyse-Test sanken die Kosten einer Aufgabe um etwa 79 %. Unter jeder Antwort zeigt der Schritt „💲 Kosten dieser Antwort“ Aufrufe, Tokens, Cache-Anteil und Kosten in USD; jeder Aufruf geht außerdem in das lokale Protokoll `usage` (nur Zahlen, kein Inhalt).
- **Sehr lange Unterhaltungen günstiger:** Bevor eine Anfrage 100.000 Tokens erreicht (darüber berechnet Claude Haiku 5.5 die ganze Anfrage 5-fach), wird der frühere Teil der Unterhaltung zu einer Zusammenfassung, die mit ihr gespeichert wird – im Chat steht die Zeile „Der Assistent sieht die Unterhaltung oben als Zusammenfassung“, die letzten 8 Nachrichten bleiben wörtlich. Bei sehr langer Werkzeugarbeit (z. B. Dutzende gelesene Dateien) entfernt Anthropic selbst ab 85.000 Tokens die ältesten Werkzeugergebnisse aus der Sicht des Modells; die 6 neuesten bleiben, und der Assistent kann eine Datei erneut lesen.
- **Der feste Teil einer Anfrage eine Stunde im Cache:** Werkzeuge und Anweisungen des Assistenten (~35.000 Tokens) bleiben 1 Stunde statt 5 Minuten im Anthropic-Cache, sodass sie nach einer Pause zwischen Fragen nicht neu geschrieben werden (ein Schreibvorgang kostet so viel wie ein Dutzend Lesevorgänge). Die Unterhaltung – bei jedem Schritt geschrieben – bleibt bei 5 Minuten.

## Version 0.9 (2026-10-27)

Anmeldung mit dem Firmenkonto am Hub, Modelle auf SQL Server, MySQL und Oracle, die Modellierungsanweisungen des Teams im Repository, Dokumente in PDF, PowerPoint, Visio und Excel, Präsentationen, SharePoint, Wächter, die ein Löschen stoppen, und ein umsichtigerer Assistent: Rückfragen, ein Plan, Sub-Agenten und Schutz vor versteckten Anweisungen.

### Compass Hub

- **Anmeldung mit dem Firmenkonto (OpenID Connect):** AD FS, Microsoft Entra ID, Keycloak, Okta und andere Anbieter, eingerichtet im Abschnitt `"oidc"` von `hub.json`. Die Weboberfläche zeigt „Sign in with your company account“.
  - Authorization Code mit PKCE; der Hub prüft das ID-Token selbst (RS256-Signatur mit den Schlüsseln des Anbieters, Aussteller, Zielgruppe, Ablauf, Nonce).
  - Rollen aus Gruppen (`roles`, standardmäßig Compass-Admins, Compass-Architects, Compass-Viewers): Die höchste Rolle gewinnt, ohne Gruppe kein Zugang. Ein lokaler Benutzer mit demselben Namen wird abgewiesen.
  - Der geheime Clientschlüssel ist das Hub-Geheimnis `OIDC_CLIENT_SECRET`. `"password_login": false` lässt nur die Firmenanmeldung zu; API-Tokens funktionieren weiterhin. Jede Anmeldung landet im Audit-Protokoll.
- **Hub hinter IIS/ARR und Anmelde-Audit:** `allatclaims` wird für AD FS automatisch ergänzt; die Sperre nach falschen Passwörtern nutzt die Client-Adresse aus `X-Forwarded-For` (nur von einem Proxy auf demselben Computer); fehlgeschlagene und abgelehnte Anmeldungen im „Audit trail“ mit ✗; der Tab „Users“ listet Firmenkonten mit Rolle und letzter Anmeldung (nur lesen); der Windows-Dienst stoppt und startet sauber neu. Handbuch: die ARR-Einstellung `reverseRewriteHostInResponseHeaders="false"`.
- **Ereignis-Agenten für das Team:** der Richtlinienschlüssel `"event_agents"` (dasselbe Format wie `event_agents.json`, auch Wächter). Die Agenten arbeiten im Compass in EA aller Teammitglieder, gekennzeichnet mit „(team)“.
- **Reiter „Team policy“ (Admin):** ein JSON-Editor der Richtlinie (min_approval, governance, event_agents, ask_min_role, ask_per_hour).
- **Word- und Excel-Berichte:** „Word report of the package“ und „Excel report of the package“ im Reiter Model check: Beschreibung, Elemente, Beziehungen (ohne Diagrammbilder). Jeder Download wird protokolliert.
- **GraphQL und Power BI:** schreibgeschützt `POST /api/sources/{id}/graphql` (Pakete, Elemente mit Attributen, Operationen, Tagged Values und Beziehungen, Diagramme; Schema unter `GET /api/graphql/schema`) und flache Tabellen `GET /api/sources/{id}/table/{elements|connectors|packages|diagrams|attributes|tagged_values}` für Power BI (Daten abrufen → Web → Erweitert, Header `Authorization: Bearer <API-Token>`). Jede Rolle.

### Modelle auf SQL Server, MySQL und Oracle

- **Neue Repositorys:** Neben PostgreSQL und .qea-Dateien liest Compass EA-Repositorys auf SQL Server, MySQL/MariaDB und Oracle: im eigenständigen Fenster, im Hub und im Prüfmodus. Einstellungen → Modell bietet die Optionen SQL Server, MySQL und Oracle.
- **ODBC-Treiber:** Der 64-Bit-ODBC-Treiber des Herstellers wird automatisch gefunden (SQL Server: ODBC Driver 18 oder 17 oder der Windows-eigene Treiber „SQL Server“). Eine vollständige Verbindungszeichenfolge ohne Passwort lässt sich in `db.odbc` angeben (zum Beispiel für Oracle).
- **Nur lesend:** Jeder Lesezugriff wird zurückgesetzt (Rollback).
- **Speichern über EA** (die EA-Edition muss DBMS-Repositories unterstützen):
  - SQL Server: Compass baut die Verbindung für EA selbst auf (OLE-DB-Anbieter SQLOLEDB), wie bei PostgreSQL.
  - MySQL und Oracle: Die Verbindung für EA kommt aus der neuen Einstellung `ea.conn` in app.properties, dem Text einer EA-Verbindungsverknüpfung über eine ODBC-Datenquelle (z. B. `ea_lab --- DBType=0;Connect=Provider=MSDASQL.1;Data Source=<ODBC-DSN>;…`). Ohne sie laufen Änderungen über Compass in EA, das wie bisher mit jedem in EA geöffneten Repository arbeitet.
- **Docker:** Das Hub-Image enthält unixODBC; den Linux-ODBC-Treiber des Herstellers fügen Sie in einem darauf aufbauenden Image hinzu.

### Modellierungsanweisungen im Repository

- **Das Regelwerk des Teams in EA:** ein Artifact «CompassInstructions» („Loonar Compass – modelling instructions“), dessen Notizen die Regeln des Repositorys sind (Benennung, Typen und Stereotypen, Pakete, Diagramme, Tagged Values, Prüfungen). Der Assistent befolgt sie in jeder Antwort, für alle, die das Repository öffnen.
- **Startassistent:** Der Assistent analysiert das Repository und entwirft die Regeln; Speichern und Ergänzen einer Regel sind Änderungen des Modells, die zur Freigabe angezeigt werden.
  - Beispiel: „Bereite Modellierungsanweisungen für dieses Repository vor“.
  - Beispiel: „Ergänze die Anweisungen: Aktivitäten beginnen mit einem Verb“.

### Unterhaltung

- **Rückfragen als Formular:** Ist etwas Wichtiges unklar, zeigt der Assistent „Fragen an Sie“ mit Auswahlmöglichkeiten oder einer kurzen Antwort („Etwas anderes…“); „Antworten senden“ schickt sie als nächste Nachricht („Meine Antworten:“).
- **Plan größerer Arbeiten:** „📋 Plan: 1 von 4 erledigt“ mit ✓ ▶ ○ in den Schritten (und in System Output von EA), aktualisiert während der Arbeit.
- **Sub-Agenten:** Für größere Analysen übergibt der Assistent eine Aufgabe an einen Sub-Agenten mit der Rolle impact, review oder document, der das Modell in einem eigenen Kontext liest und ein kompaktes Ergebnis liefert („Sub-Agent (impact): …“).
- **Aus dem Modell wählen:** Die Schaltfläche ⌖ neben „Einfügen“ sucht Elemente, Pakete und Diagramme nach Namen (leer = oberste Pakete) und fügt sie als Verweise in die Nachricht ein, etwa «Element: Order (Class, id 26)».
- **Überlegungen des Assistenten:** Die Schaltfläche 💭 neben „Einfügen“ zeigt eine Zusammenfassung der Überlegungen als Schritt „💭 Überlegungen des Assistenten“ (nur Claude; der Aufwand bleibt in den Einstellungen).
- **Code in der Sandbox von Anthropic:** Die Schaltfläche ∑ neben „Einfügen“ (standardmäßig aus; nur Claude) erlaubt dem Assistenten, Code für Berechnungen und Datenanalyse auszuführen; die Ausgabe erscheint als Schritt „Code in der Sandbox“. Die dorthin übergebenen Daten gehen wie die Unterhaltung an Anthropic.
- **Web (🌐):** eine Schaltfläche neben ∑ (standardmäßig aus). Mit Claude Websuche und Seitenlesen über Anthropic; mit OpenAI liest Compass nur Seiten, deren Adressen im Gespräch genannt wurden (nie Adressen dieses Computers oder des Firmennetzes). Erlaubte Domains: `claude.web_domains`. Hub-Richtlinie: `"web_access": false` schaltet es für alle aus, `"web_allowed_domains"` begrenzt die Domains. Suchen und Seiten erscheinen in den Schritten der Antwort.
- **Hinweis auf eine neue Version:** einmal täglich prüft Compass die neueste Version auf GitHub und zeigt eine Leiste „Neuigkeiten und Download / Nicht jetzt“ (in EA: eine Zeile in System Output). Nichts wird von selbst heruntergeladen oder installiert, keine Kennung der Installation. Ausschalten: Einstellungen → Neue Versionen (Newsletter) oder `app.update_check=false`; Hub-Richtlinie `"update_check": false`.
- **Richtlinie des Computers (Gruppenrichtlinie, Intune) und Firmenanmeldung:** der Schlüssel `HKLM/HKCU\SOFTWARE\Policies\Loonar\Compass` – `HubUrl` (Hub-Adresse gesperrt), `RequireHub` (der Assistent nur mit dem Hub und seiner Richtlinie; fällt der Hub aus, die letzte Richtlinie bis zu einer Woche), `WebAccess`, `UpdateCheck`, `MinApproval`, `AllowedProviders`. ADMX/ADML-Vorlagen in `deploy\policies`. „Mit dem Firmenkonto anmelden“ unter Einstellungen → Team: die Anmeldung des Hubs im Browser, die Sitzung kommt an 127.0.0.1 zurück und landet in der Anmeldeinformationsverwaltung.
- **KI-Schlüssel, Jira und Confluence auf dem Hub:** das Gateway `/ai/v1/messages` – der Hub verwahrt die Claude-Schlüssel pro Person (Name oder AD-UPN), Rolle oder für alle, mit Token-Limits pro Tag und Monat und einer Modellliste; Verbrauch pro Person im Tab Users. Jira und Confluence über den Hub mit dem Teamkonto (ein Viewer liest nur, Änderungen im Audit). Richtlinie: `ai_via_hub`, `jira_via_hub`, `confluence_via_hub`.
- **Ein Wächter ohne Entscheidung blockiert:** entscheidet ein Wächter nicht innerhalb von 40 s oder kann er nicht (keine KI, Hub nicht erreichbar), wird das Löschen mit einer Meldung gestoppt (vorher wurde gelöscht).
- **Stille Installation:** `LoonarCompass-Setup --quiet [--all-users] [--dir …] [--lang …] [--no-desktop] [--no-start-menu] [--no-ea-addin]` – Verteilung mit Intune, Configuration Manager oder einem Skript; für alle Benutzer funktioniert das EA-Add-in bei jedem.
- **Wächter für Löschungen in EA:** EA friert während der Entscheidung nicht mehr ein, und der Wächter entscheidet wirklich (vorher wurde nach 40 s ohne Entscheidung gelöscht).
- **Confluence-Seiten lesen:** der Assistent liest eine Seite nach Titel oder ID, auch über den Hub.
- **Installationsanleitung des Hubs für die IT** (Englisch und Polnisch).
- **Compass in EA ab dem Start:** das Compass-Fenster ist ab dem Start von EA sichtbar; ohne Projekt zeigt es „Kein Projekt geöffnet…“, beim Öffnen einer Datei oder eines Repositorys startet Compass von selbst und kehrt nach dem Schließen des Projekts zu diesem Hinweis zurück.
- **Hub-Neustart:** Sitzungen angemeldeter Personen überstehen einen Neustart des Hubs; dasselbe ungültige Token zählt nur einmal für die Adresssperre; Compass fordert bei abgelaufener Hub-Anmeldung sofort zur erneuten Anmeldung auf und beschreibt Hub-Fehler kurz (keine IIS-HTML-Seiten).
- **EA-Projektglossar:** der Standardtyp eines neuen Begriffs ist `Glossary` (vorher `Slownik`).
- **Lange Unterhaltungen:** Ab etwa 100 000 Tokens wird der frühere Teil für die nächsten Antworten durch eine Zusammenfassung ersetzt („Die Unterhaltung ist lang: der frühere Teil wird zusammengefasst“); die neuesten Nachrichten bleiben.
- **Unterhaltung löschen:** Rechtsklick auf eine Unterhaltung → „Unterhaltung löschen“ (in Compass in EA: die Schaltfläche 🗑 neben dem Namen der Unterhaltung). Sie wird nach `history\bin` verschoben und lässt sich zurückholen; nicht, während der Assistent darin arbeitet.

### Dokumente, Präsentationen und SharePoint

- **Dokumente lesen:** Das Werkzeug `read_document` und Anhänge lesen PDF (jede Seite; einen Scan ohne Textebene hängen Sie an, damit das Modell die Seitenbilder liest), PowerPoint .pptx (Folien mit Notizen), Visio .vsdx (Seiten, Formen mit Text, Verbindungen A → B), Excel .xlsx (Blätter, Zellen) und Word. Alte .doc-, .ppt-, .xls- und .vsd-Dateien müssen im neuen Format gespeichert werden.
  - Anforderungen für `trace_requirements` und `--requirements` können aus diesen Formaten stammen.
- **PowerPoint-Präsentationen:** `create_presentation` erstellt einen Foliensatz im Format 16:9: eine Titelfolie, Folien mit Aufzählungen auf zwei Ebenen und/oder einem Diagramm oder Bild, auf Wunsch auch als PDF (über Microsoft PowerPoint).
- **PDF-Dokumente:** `create_document` mit dem Format pdf oder both (über Microsoft Word). PDF braucht Microsoft Office auf dem Computer.
- **SharePoint:** die neue Karte Einstellungen → SharePoint (Website-Adresse, Dokumentbibliothek, Mandanten-ID, Anwendungs-ID (Client-ID), geheimer Clientschlüssel; „Speichern und SharePoint-Verbindung testen“). Sie braucht eine App-Registrierung in Entra ID mit der Microsoft-Graph-Anwendungsberechtigung Sites.Selected (für die Website erteilt) oder Sites.ReadWrite.All.
  - Der Assistent listet einen Ordner auf, liest den Text eines Dokuments und lädt eine von ihm erstellte Datei hoch; jedes Hochladen wird erfragt.

### Enterprise Architect

- **Ereignis-Agenten für 18 Ereignisse:** created, changed und deleting von Elementen, Konnektoren, Diagrammen und Paketen; created und deleting von Attributen, Methoden und Glossarbegriffen.
- **Wächter:** `"mode": "guard"` bei einem Ereignis `*_deleting`. EA fragt Compass vor dem Löschen; der Wächter antwortet ALLOW oder BLOCK mit einem Grund, und EA zeigt „Loonar Compass hat dieses Löschen gestoppt:“ und bricht ab. Ohne Antwort innerhalb von 40 s wird gelöscht.
  - Beispiel: `{"name": "Keep approved", "event": "element_deleting", "mode": "guard", "applies_to": "Requirement", "prompt": "Block deleting a requirement whose status is Approved."}`.
- **MDG-Technologien:** das Werkzeug `mdg_technologies`: die vom Modell verwendeten Profile (mit Anzahl), die Technologiedateien von EA und Ihre eigenen, die in das Modell importierten Technologien, Stereotypen mit Metatyp und Anwendungsbereich sowie ihre Tagged Values mit Typ, Werten und Standardwert.
- **Die EA-API:** Das Werkzeug `ea_api` liest die Automatisierungsschnittstelle aus EA.tlb des installierten EA (Klassen, Eigenschaften nur lesbar oder lesbar/schreibbar, Methoden mit Parametern, Aufzählungen); der Assistent nutzt es, bevor er EA-Skripte schreibt.
- **Versionskontrolle von Paketen:** das Werkzeug `version_control_status` (Pakete unter Versionskontrolle, Konfiguration, Datei, wer sie ausgecheckt hat) und die Modelloperation `version_control` (check_out, check_in, get_latest, put_latest, resync, mit Kommentar). Immer mit Rückfrage; das Paket muss in EA bereits unter Versionskontrolle stehen.

### Sicherheit und Anmeldung

- **Schutz vor versteckten Anweisungen (Prompt Injection):** Sicherheitsregeln in jeder Anfrage; Ergebnisse von Dokumenten, Jira, Confluence, MCP, der CMDB und Code sind als Daten gekennzeichnet; eine Antwort, die die internen Anweisungen wörtlich wiederholt, wird zurückgehalten.
- **MCP-Server über HTTP:** `"auth": {"type": "basic", "user", "password"}` oder `{"type": "oauth2", "token_url", "client_id", "client_secret", "scope"}` in `mcp_servers.json` (Geheimnisse als `${secret:mcp_…}`).
- **Der KI-Dienst mit OAuth2:** neue Felder auf der Karte KI-Modell: „OAuth2-Token-Adresse“ (optional, statt eines API-Schlüssels), dann „OAuth2-Client-ID“, „OAuth2-Bereich (Scope)“ und „Geheimer OAuth2-Clientschlüssel“ (in der Anmeldeinformationsverwaltung). Zum Beispiel Entra ID mit dem Scope `https://cognitiveservices.azure.com/.default`. Gespeichert als `ai.oauth.*`.
- **Claude über eine andere Adresse:** das Feld „Claude-Adresse“ (nur Claude; leer = Anthropic), zum Beispiel Azure AI Foundry `https://<Ressource>.services.ai.azure.com/anthropic` oder ein Firmen-Gateway. Gespeichert als `claude.url`.

## Version 0.8.2 (2026-10-23)

Compass Hub als eigenes Programm (Windows, Linux, Docker) mit Benutzerkonten, die Modellprüfung in CI-Pipelines, ArchiMate-Regeln, die vollständige Rückverfolgbarkeitskette, der Vergleich mit einer CMDB und fertige MCP-Server.

### Compass Hub als eigenes Programm

- **Eigenständiges `compass-hub`:** ein kleines Programm ohne Fenster und ohne EA, für Windows und Linux. Start: `compass-hub hub.json`. `LoonarCompass.exe --hub` funktioniert weiterhin.
- **Windows-Dienst:** `compass-hub --service-install C:\Hub\hub.json` (als Administrator): Der Hub startet mit Windows. `--service-uninstall` entfernt den Dienst.
- **Docker unter Linux:** Image `compass-hub` (`hub/Dockerfile`), Daten und `hub.json` im Volume `/data`, läuft als normaler Benutzer.
  - Beispiel: `docker run -d -p 8730:8730 -v /srv/compass-hub:/data -e LOONAR_SECRET_CLAUDE_API_KEY=… compass-hub`.
- **Geheimnisse des Hubs:** aus Umgebungsvariablen `LOONAR_SECRET_<NAME>` oder Dateien `LOONAR_SECRET_<NAME>_FILE` (z. B. `CLAUDE_API_KEY`, `JIRA_TOKEN`, `CONFLUENCE_TOKEN`, `DB_PASSWORD`).

### Konten und Anmeldung im Hub

- **Anmeldung:** neuer Anmeldebildschirm mit Benutzername und Passwort. Passwörter werden nur als PBKDF2-Hashes gespeichert, eine Sitzung gilt 12 Stunden. Die Anmeldung mit Token bleibt möglich.
- **Standardkonto:** Ein neuer Hub ohne Benutzer hat das Konto **admin / passw0rd** (mit Null). Bei der ersten Anmeldung muss das Passwort geändert werden; vorher erlaubt das Konto nichts anderes.
- **Reiter „Users“ (Admin):**
  - Benutzer anlegen, mit einem ersten Passwort, das der Benutzer ändern muss;
  - Rolle ändern und Passwort zurücksetzen;
  - neues API-Token für Compass und MCP-Clients erzeugen (wird einmal angezeigt);
  - Benutzer entfernen; mindestens ein Admin bleibt.
- **Sperre:** Nach 20 falschen Passwörtern von einer Adresse wird diese Adresse 10 Minuten abgewiesen.

### Jira und Confluence aus dem Hub

- **Dienstkonto:** Der Administrator trägt in `hub.json` den Abschnitt `"integrations"` ein (Jira: Adresse und Projekt; Confluence: Adresse und Bereich). Die Tokens werden als Geheimnisse des Hubs angegeben.
- **Schaltflächen für Architekten in der Weboberfläche:**
  - „Create Jira issue“ bei einem Element und bei einem Änderungssatz;
  - „Publish this package to Confluence“ in der Modellprüfung eines Pakets. Die Seite enthält nur Text (Elemente mit Beschreibungen und Beziehungen), keine Diagrammbilder, denn der Hub hat kein EA.
- **Protokoll:** Jede Aktion landet im Audit-Protokoll. „Ask“ liest das Modell weiterhin nur.

### Modellprüfung in CI-Pipelines

- **Prüfmodus:** `LoonarCompass.exe --check --model model.qea [--code src] [--requirements BRD.docx] [--cmdb cmdb.csv] [--report bericht.md] [--junit junit.xml] [--fail-on error|warning]`.
- **Ergebnis:** ein Markdown-Bericht, eine JUnit-Datei für GitLab, GitHub Actions und Azure DevOps sowie der Exit-Code 0 (OK), 1 (Befunde) oder 2 (Fehler). Ohne KI und ohne EA.

### Modell, Anforderungen, Code und CMDB

- **ArchiMate-3.2-Regeln:** Die Modellprüfung prüft die Beziehungen von ArchiMate-Elementen (z. B. Access nur zu passiver Struktur, Influence nur zu Motivation). Abschalten mit `"archimate": false` in governance.json.
- **Rückverfolgbarkeitskette:** `trace_requirements` mit einem Quellordner verfolgt jede Anforderung → Modell → Code → Test → Vorgang und zeigt, wo die Kette abreißt.
  - Beispiel: „Vergleiche die Anforderungen aus C:\Projekte\BRD.docx mit dem Modell und dem Code in C:\repo\shop“.
- **CMDB gegen Modell:** Ein CSV/JSON-Export (z. B. aus ServiceNow) oder Daten über einen MCP-Server werden mit den Komponenten des Modells verglichen. Der Bericht zeigt, was im Modell fehlt, was nicht in der CMDB steht und wo Verantwortlicher oder Status abweichen.
  - Beispiel: „Vergleiche den CMDB-Export C:\Export\cmdb_ci_appl.csv mit den Komponenten des Modells“.

### MCP-Server

- **Fertige Server:** Azure DevOps, GitHub, ServiceNow / CMDB, lokale Dateien und Compass Hub auf der Karte „MCP-Server“. Aus der Liste wählen und dann Organisation oder Adresse ergänzen.
- **Token eines MCP-Servers:** wird auf der Karte in der Anmeldeinformationsverwaltung gespeichert (Namen beginnen mit `mcp_`).
- **Node.js-Programme:** `npx` in der Konfiguration funktioniert unter Windows (wird als `npx.cmd` gestartet).

## Version 0.8.1 (2026-10-23)

Sicherheitsrelease nach einem Audit der gesamten Anwendung. Empfohlen für alle, besonders für Teams mit Compass Hub. Einige Aktionen fragen jetzt öfter nach Zustimmung als bisher – das ist beabsichtigt.

### Compass Hub

- **Robustheit:** eine fehlerhafte Anfrage stoppt den Hub nicht mehr. Anfragen sind in Größe, Anzahl der Header und Zeit begrenzt, und das Token wird vor dem Lesen des Inhalts geprüft.
- **Tokens:** hub.json kann statt "token" auch "token_sha256" (Hash des Tokens) enthalten; `LoonarCompass.exe --hub-token` erzeugt ein neues zufälliges Token und seinen Hash. Namen und Tokens müssen eindeutig sein. Nach 20 falschen Tokens wird eine Adresse 10 Minuten lang abgewiesen (ein gültiges Token funktioniert weiter).
- **Änderungssätze:** Beschreibung und Vorschau für den Prüfer berechnet der Hub selbst aus den Operationen, und die Weboberfläche zeigt die Operationen. Niemand, auch kein Admin, genehmigt den eigenen Satz. Die Übernahme hat zwei Schritte (ein Compass übernimmt, dann meldet es das Ergebnis), sodass ein Satz nicht zweimal übernommen werden kann. Ein Admin kann einen hängengebliebenen Satz freigeben.
- **Ask:** der Assistent in der Weboberfläche liest nur das Modell (keine Dateien, kein Jira, kein Confluence, keine MCP-Server), beantwortet eine Frage nach der anderen, mit Zeitlimit. Neue Richtlinienwerte: "ask_min_role" (wer fragen darf) und "ask_per_hour" (Standard 20 Fragen pro Person und Stunde).
- **Weboberfläche:** Content-Security-Policy und Schutz vor Einbettung in Frames. Das Token wird nur für die Browsersitzung gemerkt, und es gibt die Schaltfläche „Sign out“. Interne Fehler erscheinen als ID, die Details stehen in hub.log.
- **Audit-Protokoll:** nimmt nur bekannte Felder mit der Zeit des Hubs an. Arbeitsplatzdaten sieht nur der Admin.

### Zustimmung und Speichern im Modell

- **Dateien:** XMI-Export, Dokumente aus EA-Vorlagen und XMI-Import fragen immer nach Zustimmung (auch im Auto-Modus) und zeigen den vollständigen Pfad. Erlaubt sind nur vollständige Pfade auf einem lokalen Laufwerk, keine Netzwerkfreigaben und keine Systemordner.
- **Löschen:** eine ID als Text oder Bruchzahl umgeht die Frage beim Löschen eines Elements mit Beziehungen nicht mehr. Das Löschen eines Pakets fragt immer.
- **Gedächtnis, Vorlagen und Skripte:** das Speichern einer Gedächtnisnotiz und einer Dokumentvorlage sowie das Speichern und Starten eines EA-Skripts fragen immer, mit dem ganzen Text oder Code. Ein Skript läuft genau so wie gezeigt, mit seinen Includes.
- **Dateien lesen:** bevor der Assistent eine Datei außerhalb der Anhänge des Gesprächs liest (Anforderungsdokument, Quellordner), fragt er.
- **SQL-Abfragen:** der Assistent darf nur lesen. Abfragen mit Schreibwörtern (INSERT, UPDATE, DELETE, INTO …) oder ungewöhnlichen Anführungen werden abgelehnt, auch beim Lesen über das EA-Fenster.
- **Ereignis-Agenten:** sie lesen das Modell nur. Objektnamen gehen als Daten in ihren Auftrag, nicht als Anweisungen.
- **Rückgängig:** das Rückgängig-Journal wird vor der Ausführung geprüft, und die Zustimmung zeigt, was gelöscht wird.

### MCP-Server und Verbindungen

- **MCP-Zustimmung:** jeder MCP-Werkzeugaufruf fragt nach Zustimmung, außer der Server hat "trusted": true. Die Angabe „nur lesend“ des Servers genügt nicht mehr.
- **Geheimnisse:** die eigenen Schlüssel und Passwörter von Compass können nicht über ${secret:…} an einen MCP-Server gehen. Ausnahme: ${secret:hub_token} für den /mcp-Endpunkt des Hubs aus den Einstellungen. Ein lokales MCP-Programm erhält nur grundlegende Umgebungsvariablen und die aus "env".
- **Weiterleitungen und Adressen:** HTTP-Weiterleitungen nur innerhalb desselben Servers (API-Schlüssel gehen nirgendwo anders hin). Eine http://-Adresse ist nur für diesen Rechner erlaubt, ohne Benutzername oder Passwort darin.

### Weiteres

- **Diagramm-Editor:** Namen aus dem Modell erscheinen als Text (kein HTML), die Seite hat eine CSP, und nur Objekte des geöffneten Diagramms können gelöscht oder geändert werden.
- **EA-Add-in:** das Protokoll enthält keinen Connection-String mehr, Compass startet in seinem eigenen Ordner, und DLLs werden nicht im aktuellen Ordner gesucht.
- **EA-Fehlerdialoge:** geschlossen werden nur die, die während der Abfrage von Compass erscheinen.
- **SQLite 3.50.2:** behebt bekannte Schwachstellen. Modelldateien werden im Defensivmodus geöffnet.
- **Empfehlung für Unternehmen:** eine eigene PostgreSQL-Rolle mit minimalen Rechten (SELECT, für das Speichern über EA zusätzlich INSERT, UPDATE und DELETE auf den t_*-Tabellen; nie Superuser) und Verbindungsverschlüsselung mit Zertifikatsprüfung.

## Version 0.8 (2026-10-23)

Das bisher größte Release. Compass prüft die Modellqualität, kontrolliert jedes Speichern und macht es rückgängig, vergleicht das Modell mit Anforderungen und mit Code, schätzt Änderungen und arbeitet im Team. Zu jeder Funktion gibt es eine Beispielanfrage: Geben Sie sie so im Gespräch ein oder mit eigenen Worten.

### Wahl des KI-Modells

- **Anbieter und Modelle.** Anthropic (Claude) oder OpenAI, ein beliebiges Modell (aus der Liste oder eigene ID) und die Aufwandsstufe, von „Standard des Modells“ bis max. Wo: Einstellungen → Karte „KI-Modell“.
  - Standard unverändert: Claude Haiku 5.5.
  - Jeder Anbieter hat einen eigenen Schlüssel in der Windows-Anmeldeinformationsverwaltung; Daten gehen nur an den gewählten Anbieter.
- **Jeder OpenAI-kompatible Endpunkt.** Azure OpenAI, Ollama, LM Studio oder ein Firmen-Gateway. Mit einem lokalen Modell verlässt nichts Ihren Rechner.

### Modellqualität und Teamregeln

- **Modellprüfung.** Regelbasiert, ohne Raten durch die KI. Jeder Befund hat einen Schweregrad: Fehler, Warnung oder Information. Gefunden werden:
  - Standard- und leere Namen, Elemente ohne Beschreibung, verwaiste Elemente;
  - doppelte Namen und Beziehungen, Beziehungen zu nicht vorhandenen Elementen;
  - leere Pakete und Diagramme, Flüsse gegen UML- oder BPMN-Regeln.
  - Beispiel: „Prüfe die Qualität des Pakets Domäne und liste die wichtigsten Probleme auf.“
- **Regeln der Organisation.** In governance.json neben den Einstellungen oder vom Team-Hub: Namenskonventionen, Pflicht-Tagged-Values, erlaubte Beziehungen zwischen Typen (Metamodell / MDG).
  - Beispiel: „Prüfe, ob alle Komponenten im Paket Systeme den Tagged Value Owner haben.“
- **Gap-Analyse Ist / Soll.** Was neu ist, was entfällt und was sich geändert hat (Stereotyp, Beschreibung, Beziehungen); auf Wunsch nach Excel.
  - Beispiel: „Vergleiche das Paket Ist mit dem Paket Soll und speichere die Lücken in Excel.“
- **Rechtschreibung in der Compass-Sprache.** Notizen von Elementen und Diagrammen, Szenarioschritte und das Projektglossar werden mit der Windows-Rechtschreibprüfung geprüft. Elementnamen und Glossarbegriffe gelten als bekannte Wörter; Korrekturen erst nach Ihrer Freigabe.
  - Das Speichern der Sprache in den Einstellungen stellt auch die EA-Rechtschreibprüfung auf diese Sprache um, wenn EA ein Wörterbuch dafür hat.
  - Sprache fehlt: Windows-Einstellungen → Zeit und Sprache → Sprache → Sprache hinzufügen.
  - Beispiel: „Prüfe die Rechtschreibung der Notizen im ganzen Modell und korrigiere die Fehler.“

### Speichern, dem Sie vertrauen können

- **Vorschau „vorher → nachher“.** Das Freigabefenster zeigt, was entsteht, welcher Wert sich in welchen ändert und was gelöscht wird.
- **Prüfung nach jedem Speichern.** Compass liest das Modell erneut und vergleicht es mit dem Auftrag. Weicht etwas ab, sagt der Assistent es, statt „fertig“ zu melden. Geprüft wird, ob:
  - angelegte Objekte existieren, Beziehungen die richtigen Enden haben, Elemente im gewünschten Paket liegen;
  - Namen, Beschreibungen, Stereotypen und Tagged Values die neuen Werte haben;
  - gelöschte Objekte verschwunden sind und XMI-Dateien und Berichte geschrieben wurden.
- **Alles oder nichts.** Schlägt ein Speichern auf halbem Weg fehl, werden die bereits angelegten Objekte entfernt.
- **Sicheres Löschen.** Das Löschen eines Elements mit Beziehungen braucht immer Ihre Freigabe (auch im Auto-Modus); das Fenster nennt diese Beziehungen.
- **Zeitmaschine.** Jedes Speichern kommt mit den Umkehroperationen in ein Rückgängig-Journal; rückgängig machen lässt sich, was angelegt oder geändert wurde. Ein Löschen macht Compass nicht rückgängig: Vorher legt es eine EA-Baseline des Pakets an, die Sie in EA wiederherstellen. Vor dem Rückgängigmachen fragt Compass immer nach.
  - Beispiele: „Mach die letzte Änderung rückgängig.“, „Zeig die Historie der Änderungen, die du in diesem Modell gemacht hast.“, „Mach die letzten zwei Änderungen rückgängig.“

### Anforderungen, Code und Schätzung

- **Anforderungen gegen das Modell.** Ein Word-Dokument, Text oder Markdown, eine Confluence-Seite oder in das Gespräch eingefügter Text wird zur Rückverfolgbarkeitsmatrix: abgedeckt, wahrscheinlich abgedeckt, nicht abgedeckt, „gleiche ID, anderer Inhalt“ und Modellelemente, die keine Anforderung verlangt. Trace-Beziehungen entstehen erst nach Ihrer Freigabe.
  - Beispiel: „Vergleiche die Anforderungen aus C:\Projekte\BRD.docx mit dem Modell: Was ist abgedeckt und was fehlt?“, danach „Lege Trace-Beziehungen für die abgedeckten Anforderungen an.“
- **Modell gegen Code.** Ein Quellordner, nur lesend: Klassen, Interfaces, Enums und Records mit Feldern (Java, Kotlin, C#, TypeScript, Rust, Python), SQL-Tabellen mit Spalten, API-Operationen (OpenAPI operationId). Der Bericht zeigt, was im Modell fehlt, was es im Code nicht mehr gibt und wie sich die Felder unterscheiden.
  - Beispiel: „Vergleiche das Paket Domäne mit dem Code in C:\repo\shop – was fehlt im Modell und was im Code?“
- **Schätzung einer Änderung.** Die Auswirkung einer Änderung wird zu einer Aufgabenliste in einfacher Sprache (ändern, anpassen, prüfen) mit Stunden nach Elementtyp und Beziehungsebene, Diagrammaktualisierung, Tests, Analyse, Dokumentation und Risikopuffer; auf Wunsch nach Excel.
  - Die Normen stehen in estimate.json neben den Einstellungen. Das ist eine Art zu schätzen, nicht die einzige: Passen Sie die Normen an Ihr Team an.
  - Beispiel: „Schätze eine Änderung der Klasse Bestellung: Was muss sich ändern und wie lange dauert es?“

### Neue Modelloperationen

- **Automatisches Diagramm-Layout.** Nach unten, rechts, oben oder links, mit dem gewünschten Abstand.
  - Beispiel: „Ordne das Diagramm Domänenmodell von links nach rechts an.“
- **XMI-Export und -Import eines Pakets.** Der Import vergibt neue GUIDs, außer Sie möchten sie behalten.
  - Beispiel: „Exportiere das Paket Domäne nach C:\Export\domaene.xml.“
- **Dokumente aus EA-Dokumentvorlagen.** DOCX, PDF oder RTF, wie mit Publish → Report Builder.
  - Beispiel: „Erstelle einen Bericht aus der Vorlage Model Report für das Paket Domäne nach C:\Berichte\domaene.docx.“
- **Mehr Bearbeitung.** Swimlanes von Diagrammen, verknüpfte Dokumente von Elementen, Umkehren einer Beziehung, Paketdaten (Name, Beschreibung, Version, Status, Stereotyp). Aggregation und Komposition entstehen als echte UML-Beziehungen.
  - Beispiele: „Füge dem Diagramm Bestellprozess die Bahnen Vertrieb und Lager hinzu.“, „Kehre die Beziehung zwischen Kunde und Bestellung um.“
- **Strukturierte Szenarien.** Szenarien von Elementen mit Akteur- und Systemschritten.
  - Beispiel: „Füge dem Anwendungsfall Bestellung aufgeben ein Hauptszenario mit fünf Schritten hinzu.“

### EA-Skripte

- **Skripte lesen, speichern und ausführen.** Der Assistent zeigt Gruppen und Skripte und liest ein Skript. Er speichert JScript, VBScript und JavaScript (eine fehlende Gruppe wird angelegt; Freigabe wie bei jedem Speichern) und führt ein Skript jedes Mal nach Ihrer Freigabe aus, im offenen EA-Fenster oder im Hintergrund.
  - Repository und Session.Output funktionieren wie in EA; „!INC Gruppe.Skript“ wird aus dem Modell aufgelöst.
  - Beispiel: „Erstelle und starte ein EA-Skript in JScript, das die Namen aller Pakete des Modells ausgibt.“

### Teamarbeit: Compass Hub

- **Ein optionaler Team-Dienst.** `LoonarCompass.exe --hub hub.json` auf einem Team-Rechner oder Server. In hub.json stehen die Listen-Adresse, die Modelle des Teams (EA-Datenbanken und -Dateien), Benutzer mit Tokens und Rollen (viewer, architect, admin) und die Richtlinie.
  - In Compass: Einstellungen → Karte „Team (Compass Hub)“, mit der Adresse und Ihrem Token vom Hub-Administrator.
- **Gemeinsame Richtlinie.** Regeln der Modellprüfung und der minimale Freigabemodus gelten an allen Arbeitsplätzen; jedes Speichern geht ins zentrale Audit-Protokoll; gemeinsame Teamnotizen.
- **Änderungssätze zur Prüfung.** Statt zu speichern, schlägt der Assistent die Änderung dem Hub vor. Ein zweiter Architekt kommentiert und genehmigt sie in der Weboberfläche (Vier-Augen-Prinzip); danach übernimmt der Autor den genehmigten Satz mit Prüfung.
  - Beispiele: „Schlage dem Team eine Änderung zur Prüfung vor: füge die Klasse Lager zum Paket Domäne hinzu.“, „Zeig die genehmigten Änderungssätze und übernimm Satz 3.“
- **Eine Weboberfläche für Kollegen ohne EA.** Die Hub-Adresse im Browser, mit den Reitern Ask, Search, Model check, Gaps, Audit trail, Team notes und Change sets. Unter „Ask“ stellt ein Stakeholder eine Frage zum Modell, und der Assistent antwortet nur lesend.
- **Der MCP-Server des Hubs.** Nur lesend, für andere KI-Werkzeuge: Quellen, Paketbaum, Elementsuche und -details, Auswirkungsanalyse, Modellprüfung und Gap-Analyse.

### Jira und eigene MCP-Server

- **Jira (Cloud und Data Center).** Vorgänge per Schlüssel oder JQL-Suche als Kontext für das Modell; nach Ihrer Freigabe neue Vorgänge, Kommentare und Links zu Elementen. Wo: Einstellungen → Karte „Jira“.
  - Beispiele: „Lies den Vorgang SHOP-123 und prüfe, welche Modellelemente er betrifft.“, „Lege in Jira einen Vorgang für die fehlende Beschreibung der Klasse Rechnung an.“
- **Eigene MCP-Server.** Karte „MCP-Server“, gespeichert in mcp_servers.json: ein lokales Programm (stdio) oder HTTP; Tokens aus ${secret:name} (Anmeldeinformationsverwaltung) oder ${env:NAME}.
  - Ein Werkzeug, das nicht nur liest, fragt vor jedem Aufruf nach Freigabe, außer der Server hat "trusted": true.
  - „MCP-Server speichern und testen“ zeigt die Zahl der Werkzeuge jedes Servers.
  - Beispiel: „Liste über den MCP-Server hub die Modellquellen des Teams auf.“

### Compass in Enterprise Architect

- **Laufende Prüfung.** Eine neu gezeichnete Beziehung und ein geändertes Element werden sofort geprüft (Regeln und Rechtschreibung der Notizen); Hinweise erscheinen im Reiter „Loonar Compass“ des Fensters System Output.
- **Ereignis-Agenten.** Karte „Ereignis-Agenten (Compass in EA)“, gespeichert in event_agents.json: Wenn ein Element entsteht oder sich ändert oder eine Beziehung oder ein Diagramm entsteht, tut der Assistent, was die Regel sagt.
  - Er liest das Modell und antwortet in System Output (Zeilen mit 🤖); er ändert das Modell nie selbst; derselbe Agent und dasselbe Objekt höchstens einmal pro Minute.
  - Beispielregel für eine neue Komponente: „prüfe, ob sie eine Beschreibung und einen Verantwortlichen hat, und schlage fehlende Beziehungen vor“.
- **Gespräch umbenennen.** Der Stift neben der Gesprächsliste.
- **Lesen einer in EA geöffneten Modelldatei.** Lesezugriffe laufen über das EA-Fenster und sehen so die letzten Änderungen, bevor EA sie auf die Festplatte schreibt.

### Weitere Änderungen und Korrekturen

- Einstellungskarten ohne Nummern; die erste Karte ist „KI-Modell“.
- Fortschrittsmeldungen sagen „Der Assistent …“ statt „Claude …“.
- Dateimodelle: Nach jedem Speichern sind die Änderungen sofort auf der Festplatte, sodass folgende Lesezugriffe den neuen Stand sehen.
- EA-Fehlerdialoge (SQL, XML) blockieren die Arbeit im Hintergrund nicht mehr; Abfragen mit Ausdrücken und doppelten Spaltennamen liefern korrekte Ergebnisse.
- Benutzerhandbuch in 4 Sprachen: ein neues Kapitel mit Anwendungsfällen und fertigen Anfragen, das Kapitel „Compass in Enterprise Architect“ und die Karten „KI-Modell“, „Jira“, „Team“, „MCP-Server“ und „Ereignis-Agenten“.

## Version 0.7.5 (2026-10-22)

- Korrektur: Compass in EA füllt seinen ganzen Bereich und folgt jeder Größenänderung, auch in einem schwebenden Fenster und in einer Tab-Gruppe.
- „Info“ und die Fußzeile zeigen die vollständige Versionsnummer (z. B. 0.7.5).

## Version 0.7.4 (2026-10-21)

- Korrektur: Compass in EA folgt wieder jeder Größenänderung seines Bereichs (in 0.7.3 behielt es die Größe vom Öffnen).

## Version 0.7.3 (2026-10-21)

- Compass in EA: Tippen funktioniert nach „Zur Compass-Unterhaltung hinzufügen“, nach einem Klick in den Browser oder ein Diagramm und nach dem Wechsel der Bereiche (die Tastatur geht an das Compass-Fenster).
- Das Compass-Fenster passt in den sichtbaren Teil des EA-Bereichs, die untere Symbolleiste bleibt sichtbar.
- Eine fehlerhafte Abfrage des Assistenten an eine in EA geöffnete Datenbank hält EA nicht mehr mit einem Fehlerfenster an: Compass schließt es und gibt den Fehler an den Assistenten weiter, der die Abfrage korrigiert.

## Version 0.7.2 (2026-10-21)

- Compass in EA arbeitet mit jeder in EA geöffneten Datenbank, auch einer, die nicht in den Einstellungen steht: Es liest das Modell über die Verbindung von EA selbst, ohne Passwörter und Konfiguration.
- Das Compass-Fenster dockt immer im EA-Bereich an (vorher blieb es manchmal als eigenes Fenster daneben).

## Version 0.7.1 (2026-10-21)

- EA-Projektglossar: Der Assistent fügt einen Begriff mit Bedeutung und Typ hinzu (standardmäßig „Slownik“); ein polnischer Begriff kommt in allen Formen hinein (Singular und Plural, vom Nominativ bis zum Vokativ), sodass EA jede Form in Notizen hervorhebt. Im Nachrichtenfeld: Rechtsklick auf ein Wort → „Zum EA-Projektglossar hinzufügen“, oder als Bitte im Gespräch.
- Compass in EA folgt der Breite seines Bereichs: Text bricht um, statt rechts zu verschwinden.
- Kontextmenü eines Elements, Pakets oder Diagramms: „Zur Compass-Unterhaltung hinzufügen“ wieder direkt unter Specialize.
- Die Tastatur funktioniert nach dem Wechsel der EA-Bereiche (z. B. Notes → Compass); Compass schließt zusammen mit EA.

## Version 0.7 (2026-10-21)

Compass in Enterprise Architect:

- EA-Add-in: Die Schaltfläche „Loonar Compass“ im Menüband Start (Gruppe Add-Ins) öffnet Compass als angedocktes Fenster in EA, an derselben Stelle wie Properties (EA merkt sich, wo Sie es ablegen). Das Installationsprogramm hat die Option „Loonar Compass zu Enterprise Architect hinzufügen“; sie funktioniert mit jeder EA-Installation, auch mit einer Testversion.
- System Output hat einen Tab „Loonar Compass“: Ihre Nachrichten und die Schritte des Assistenten (Lesen des Modells, Schreiben in EA, Ergebnisse) mit Uhrzeit. Im Chat bleibt eine Zeile mit dem letzten Schritt.
- In EA arbeitet Compass immer mit dem in diesem EA-Fenster geöffneten Projekt (Datei oder Datenbank): kein Modell zu wählen, nichts zu bestätigen. Die Liste zeigt nur die Unterhaltungen dieses Projekts; ein anderes Projekt in EA schaltet Compass um. Ohne geöffnetes Projekt arbeitet Compass nicht.
- Schreibvorgänge laufen über dasselbe EA-Fenster, Änderungen erscheinen sofort im Browser und in Diagrammen.
- „Diagramm zeigen“ und „Diagramm bearbeiten“ öffnen das Diagramm in EA, ohne Bilder im Chat.
- Kontextmenü eines Elements, Pakets oder Diagramms: „Zur Compass-Unterhaltung hinzufügen“ fügt einen Verweis in die Nachricht ein, z. B. «Paket: Shop (id 5)».
- Layout des angedockten Fensters: kleinere Schrift, Unterhaltungen des Projekts in einer Auswahlliste mit „+“, Ansichtssymbole unten, Seiten scrollen seitlich und vertikal; Strg+V fügt Bilder, Dateien und Text ein.
- In EA enthalten die Einstellungen nur Claude, Freigabe, Confluence und Miro; Modell und EA ergeben sich aus dem geöffneten Projekt.

Überall:

- Wörterbuch: Rechtsklick auf ein unterstrichenes Wort im Nachrichtenfeld → „Zum Wörterbuch hinzufügen“. Ein polnischer Ausdruck kommt in allen Formen ins Windows-Wörterbuch (Singular und Plural, vom Nominativ bis zum Vokativ).

## Version 0.6.5 (2026-10-20)

- Das EA-Konto gehört zum Modell, nicht zur App: EA-Konto und Passwort werden in der Karte „2. Modell“ bei Datei oder Datenbank eingestellt. Die Karte „3. Enterprise Architect“ enthält nur den Pfad zu EA.exe und „Speichern und EA-Zugriff prüfen“.
- Datei und Datenbank: Kontrollkästchen „Im Modell ist Security aktiviert (EA-Benutzerkonten)“. Ohne es sind Konto- und Passwortfelder deaktiviert, und EA öffnet das Modell ohne Anmeldung.
- Neue Option „Das Modell erfordert eine Sperre zum Bearbeiten (Require User Lock to Edit)“: damit sperrt der Assistent die geänderten Pakete und Elemente und gibt die Sperren danach frei; ohne sie sperrt er nicht.
- Der Test der Modelleinstellungen und des Modells einer Unterhaltung speichert ein Testpaket im Modell, prüft es und löscht es sofort.
- Eine neue Unterhaltung beginnt mit der Bestätigung ihres Modells („Modell bestätigen und beginnen“). Danach ist das Modell festgelegt und nur noch sichtbar; eine Änderung des Standardmodells in den Einstellungen ändert laufende Unterhaltungen nicht.
- Korrektur: Eine Unterhaltung erscheint gleich nach der ersten Nachricht in der Liste. Vorher fehlte eine Unterhaltung, in der der Assistent noch arbeitete.
- Korrektur: Partitionen (Swimlanes) im Aktivitätsdiagramm, die höher als breit sind, zeichnet EA senkrecht, mit dem Namen oben.
- Korrektur: Die Vorschau eines im EA-Fenster geöffneten Diagramms zeigt den Stand nach dem letzten Schreiben (EA lädt das Diagramm vor dem Bildexport neu).

## Version 0.6.4 (2026-10-19)

- Zeichnen nach einem Bild: Der Assistent übernimmt das Layout des Anhangs (Zeilen, Reihenfolge, Lage zu den Nachbarn, Verlauf der Linien), statt das Diagramm neu anzuordnen; er misst das Bild zuerst aus, hält mindestens 40 px Abstand zwischen Elementen und prüft danach zuerst jede Beziehung, dann das Layout (höchstens zwei Korrekturrunden).
- Ein Linienstil für das ganze Diagramm wird nach dem Anlegen der Beziehungen angewendet und behält von Hand geführte Linien.
- Ein Objektknoten (ObjectNode) im Aktivitätsdiagramm wird als EA-Objekt gespeichert, statt das Speichern abzubrechen.
- Kurze Notizen des Assistenten zwischen den Schritten sind in der Sprache der Unterhaltung.

## Version 0.6.3 (2026-10-19)

- Diagrammeditor: Beim Speichern in EA bleiben die Linien genau so, wie der Editor sie zeigt (waagerechte und senkrechte Abschnitte, Berührungspunkte mit den Elementen), auch bei neuen Beziehungen. Vorher erhielt EA nur von Hand verschobene Punkte und zeichnete die Linien schräg.

## Version 0.6.2 (2026-10-19)

- Korrektur: Eine Gabelung/Vereinigung (Fork/Join) im EA-Aktivitätsdiagramm wurde waagerecht gezeichnet, obwohl sie senkrecht sein sollte. Die Ausrichtung setzt die Anwendung jetzt aus der Form (höher als breit = senkrecht).
- Diagrammeditor: „Gabelung / Vereinigung“ in der Werkzeugleiste des Aktivitätsdiagramms.

## Version 0.6.1 (2026-10-19)

- Korrektur: Der Diagrammeditor öffnete keine Diagramme eines Modells in einer PostgreSQL-Datenbank (Fehler „column \"default\" does not exist“). Modelle in .qea/.qeax-Dateien funktionierten.

## Version 0.6 (2026-10-18)

Diagrammeditor:

- Jedes EA-Diagramm in der Unterhaltung hat die Schaltfläche „Diagramm bearbeiten“. Sie öffnet das Diagramm in einem Editorfenster (WebView2): Verschieben, Größe ändern, Ausrichten, Umbenennen, Rückgängig (Strg+Z / Strg+Y). Der Editor ändert das Modell, kein Bild: Gespeichert wird über EA, mit dem Freigabebereich und dem Änderungsprotokoll.
- Neue Elemente aus einer zur Diagrammart passenden Werkzeugleiste: Klassen, Komponenten, Anwendungsfälle, Aktivitäten, BPMN, Zustandsautomat (Zustände, Start, Ende, Auswahl), Verteilung (Knoten, Geräte, Ausführungsumgebungen, Artefakte), Pakete (echte Pakete) und Sequenz. Ein neues Element kommt an eine freie Stelle; Verbinden durch Ziehen von Element zu Element.
- Verbinden: Ein Element unter der Maus zeigt einen blauen Pfeil; ziehen Sie ihn auf ein anderes Element. Die Art der Beziehung kommt aus der Liste „Verbindung“.
- BPMN: Aufgabentypen (Benutzer, Service, manuell, Skript, Senden, Empfangen, Geschäftsregel), Ereignistypen (Nachricht, Zeitgeber, Signal, Fehler…) und Gateway-Typen (exklusiv, inklusiv, parallel, komplex, ereignisbasiert) als Symbole und Markierungen, Auswahl des Typs auf der Registerkarte „Allgemein“, ein Zwischenereignis in der Werkzeugleiste. Klassen wachsen von selbst, wenn Attribute und Operationen hinzukommen.
- „Aus dem Modell…“: fügt Elemente, die bereits im Modell existieren, mit einer Suche in das Diagramm ein.
- Löschen fragt: aus diesem Diagramm oder aus dem Modell. Beim Löschen aus dem Modell zeigt es, in wie vielen und welchen anderen Diagrammen das Element ist und wie viele Beziehungen es hat.
- Bereich „Eigenschaften“: die Felder der allgemeinen Registerkarte von EA (Name, Alias, Stereotyp, Schlüsselwörter, Status, Version, Autor, Phase, Komplexität, abstrakt, Notizen; Paket, GUID und Datum nur zum Lesen), Attribute und Operationen von Klassen, Tagged Values (mit den Auswahllisten des Profils, z. B. BPMN), Darstellung im Diagramm (Farbe, Linienstil). Jedes Feld ist mit „Modell“ oder „Diagramm“ gekennzeichnet.
- Beziehungen: Name, Stereotyp, Richtung, Multiplizitäten und Rollen beider Enden, Notizen, Tagged Values.
- Sequenzdiagramme: Lebenslinien, Nachrichten in ihrer EA-Reihenfolge (umsortieren mit ▲ ▼ oder Alt+↑ / Alt+↓), synchrone, asynchrone und Antwortnachrichten, ein Name aus einer Operation des Empfängers.
- „Mit KI aufräumen“: speichert die Änderungen und bittet den Assistenten in der Unterhaltung, das Diagramm gut lesbar anzuordnen.
- Neue Schreiboperationen für den Assistenten: Beziehungen ändern, Attribute und Operationen ändern und löschen, Untertyp des Elements (Pseudozustände), Sequenznachrichten mit Nummer, Art und Antwortkennzeichen; Tagged Value entfernen.

Miro (von EA nach Miro):

- Einstellungskarte „7. Miro“: Zugriffstoken (in der Windows-Anmeldeinformationsverwaltung) und Verbindungstest.
- „An Miro senden“ unter einem Diagramm: Board-Auswahl, eine Miniatur des Boards mit dem Platz für das Diagramm (die Anwendung findet eine freie Stelle; der Rahmen lässt sich ziehen), gesendet als bearbeitbare Formen und Verbindungen in einem Rahmen mit dem Namen des Diagramms. Sequenzdiagramme: Lebenslinien und nummerierte Nachrichten in ihrer EA-Reihenfolge.

Dokumentation:

- Neue Kapitel des Handbuchs: „9. Diagrammeditor“ und „10. Diagramme an Miro senden“ (die folgenden Kapitel sind um zwei verschoben); auf der Website auf dem Telefon ein Dokumentationssymbol in der Kopfzeile.

## Version 0.5 (2026-10-17)

Ein Modell pro Unterhaltung:

- Jede Unterhaltung hat ihr eigenes Modell. Der Kopf der Unterhaltung zeigt es an („Modell: …“); ein Klick öffnet das Modell der Unterhaltung, das Sie dort prüfen oder ändern können.
- Eine neue Unterhaltung zeigt, welches Modell sie verwendet: das Standardmodell aus den Einstellungen. Mit „Anderes Modell wählen…“ wählen Sie eine andere PostgreSQL-Datenbank (Server, Port, Datenbank, Konto, Passwort, Verschlüsselung) oder eine EA-Datei (.qea, .qeax) mit ihrem EA-Konto und prüfen die Verbindung.
- Passwörter einer Verbindung, die von den Einstellungen abweicht, liegen in der Windows-Anmeldeinformationsverwaltung; die Deinstallation entfernt sie zusammen mit den übrigen Geheimnissen, wenn Sie das wählen.
- Wenn Sie eine .qea- oder .qeax-Datei auf das Fenster ziehen, wird sie zum Modell der Unterhaltung (in einer Unterhaltung mit Nachrichten beginnt eine neue Unterhaltung).
- Bestehende Unterhaltungen erhalten eine Kopie des Modells aus den Einstellungen und arbeiten so auch nach einer späteren Änderung der Einstellungen damit weiter.
- Das Gedächtnis des Assistenten bleibt für alle Modelle gemeinsam. Eine Notiz gilt nur dann für ein einzelnes Modell, wenn Sie darum bitten („merke dir das nur für dieses Modell“).

## Version 0.4 (2026-10-17)

Modelle in Dateien:

- Das Modell kann jetzt eine Enterprise-Architect-Projektdatei .qea oder .qeax sein: Einstellungen → „2. Modell“ → „EA-Datei (.qea, .qeax)“ und der Pfad zur Datei. Ein Datenbankserver, ein Konto oder ein Passwort sind nicht nötig.
- Der Assistent liest die Datei direkt und nur lesend. Änderungen speichert EA: über Ihr EA-Fenster, wenn Sie die Datei dort geöffnet haben (ein zweites EA öffnet nie dieselbe Datei), sonst über EA im Hintergrund.
- Die Benutzersicherheit ist in einer Datei optional: ohne sie bleiben EA-Konto und Passwort leer. Eine Datei mit aktivierter Sicherheit funktioniert wie ein Datenbankmodell, mit einem EA-Konto.
- Das Installationsprogramm bietet dieselbe Auswahl auf der Konfigurationsseite.
- Der Assistent schreibt seine Abfragen im SQL des Speicherorts des Modells (SQLite für Dateien, PostgreSQL für Datenbanken).

## Version 0.3 (2026-10-17)

Neuer Name und Veröffentlichung:

- Neuer Name: Loonar Compass (zuvor Loonar EA Assistant). Einstellungen, Unterhaltungen, Gedächtnis, Vorlagen und Passwörter aus der vorherigen Version werden beim ersten Start automatisch übernommen; Dateien mit polnischen Namen im Datenordner haben englische Namen erhalten (`audit_log.jsonl`, `confluence_pages.json`).
- Der Quellcode ist öffentlich auf GitHub (github.com/loonar-eu/loonar-compass), und die Website loonar.eu beschreibt die Anwendung in vier Sprachen, mit Fragen und Antworten (FAQ).
- Neue Freeware-Lizenz (verbindlich ist die polnische Fassung, Übersetzungen EN/DE/FR): Nutzung auch kommerziell, ohne zeitliche und räumliche Beschränkung, mit Regeln zu KI-Ergebnissen, Daten und Open-Source-Komponenten. Die Lizenzen der Open-Source-Komponenten sind in der Datei THIRD-PARTY-NOTICES.txt und in der Ansicht „Info“ aufgeführt.
- Ansicht „Projekt unterstützen“ (Herzsymbol in der Navigationsleiste): freiwillige Unterstützung über suppi.pl/loonar.
- Datenschutzerklärung und Impressum auf loonar.eu; in der Anwendung kurze Hinweise beim API-Schlüssel (was an Anthropic geht), bei der Fehlermeldung und beim Newsletter sowie unter dem Nachrichtenfeld die Erinnerung, dass die Antworten von einem KI-Modell erstellt werden.
- Neuer Hintergrund für den Startbildschirm und das Banner des Setups (Himmelskarte mit Globusgitter und Kompassrose).

Sicherheit und Datenschutz:

- Das Datenbankpasswort liegt in der Windows-Anmeldeinformationsverwaltung, wie der API-Schlüssel, das Passwort des EA-Kontos und das Confluence-Token; ein zuvor in der Einstellungsdatei gespeichertes Passwort wird automatisch dorthin übertragen.
- Verschlüsselung der Verbindung zur Modelldatenbank (TLS), Karte „2. Modelldatenbank“: bevorzugt (Standard), erforderlich oder erforderlich mit Prüfung des Zertifikats im Windows-Zertifikatspeicher. Der Verbindungstest meldet, ob die Verbindung verschlüsselt ist, und warnt vor einem Superuser-Konto.
- Confluence nur über HTTPS (http:// ausschließlich für einen Server auf demselben Computer). Vor der Veröffentlichung prüft der Assistent, ob es bereits eine Seite mit diesem Titel gibt, und sagt ausdrücklich, dass ihr Inhalt ersetzt wird.
- Löschen, Verschieben und Kopieren von Paketen erfordert auch im Modus plan eine eigene Freigabe, und das Freigabepanel nennt die Namen der Objekte. Das Speichern im Gedächtnis des Assistenten und das Überschreiben einer Vorlage durchlaufen die Freigabe wie Änderungen am Modell.
- Inhalte von Anhängen, Dokumenten und Modell behandelt der Assistent als Daten, nicht als Anweisungen. Ein vom Assistenten gezeichnetes Bild kann keine Datei von der Festplatte oder aus dem Netz laden, und seine Größe ist begrenzt.
- Lesen aus der Datenbank: Zeitlimit für Abfragen und Sperre von Serverfunktionen, die Dateien lesen oder andere Sitzungen beeinflussen. Die Verbindungszeichenfolge des geöffneten EA-Fensters mit Anmeldedaten gelangt nicht an das Modell.
- Systemprogramme und -bibliotheken werden ausschließlich aus dem Windows-Ordner gestartet, nie aus dem Download-Ordner.

Installation:

- Installation ohne Administratorrechte: standardmäßig nur für den aktuellen Benutzer (`%LOCALAPPDATA%\Programs\Loonar Compass`); eine Installation für alle Benutzer fragt nach Administratorrechten. Einstellungen und Passwörter werden immer im Profil des angemeldeten Benutzers gespeichert.
- Eine frühere Version (Loonar EA Assistant), die nur für Sie installiert ist, wird bei der Installation entfernt; eine für alle Benutzer installierte können Sie mit einer Schaltfläche im Setup entfernen.
- Die Deinstallation entfernt auf Wunsch den gesamten Datenordner sowie alle Geheimnisse aus der Anmeldeinformationsverwaltung (auch das Confluence-Token und das Datenbankpasswort).
- Das Setup-Fenster passt auch auf kleine Bildschirme und auf Bildschirme mit hoher Skalierung; entfernt wurden die Verknüpfung „Schnellstart“, das Feld „Modell“ und das Feld Workspace-ID.

Stabilität:

- Anfragen an die Claude-API werden bei Überlastung, beim Anfragelimit und bei vorübergehenden Serverfehlern wiederholt; ein falscher API-Schlüssel und eine Sperre durch einen Proxy haben verständliche Meldungen.
- Proxy aus den Windows-Interneteinstellungen (zusätzlich zur Variablen HTTPS_PROXY).
- Bei einem unerwarteten Fehler zeigt die Anwendung eine Meldung und speichert die Details in einer Datei, statt zu verschwinden; ein Fehler in der Einstellungsdatei wird beschrieben statt „keine Einstellungen“.
- Behoben: Die Anwendung wurde bei langen Bildbeschriftungen mit Zeichen außerhalb von ASCII auf Confluence-Seiten beendet.
- Anwendung und Setup starten auch unter älteren Versionen von Windows 10 und ohne das Paket Visual C++ Redistributable.

Sprachen:

- Vier Sprachen: Polnisch, Englisch, Deutsch und Französisch. Das Setup beginnt mit der Sprachauswahl (standardmäßig Englisch), die die Sprache der Anwendung, der Antworten des Assistenten und der Standard-Dokumentvorlagen festlegt. Die Sprache können Sie in den Einstellungen ändern (Karte „4. Sprache und Freigabe von Änderungen“).
- Alles, was ohne KI verfügbar ist, gibt es in vier Sprachen: Anwendungs- und Setup-Fenster, Dokumentvorlagen, Lizenz, Benutzerhandbuch und Änderungsliste.
- Der Assistent antwortet in der Sprache der Anwendung, es sei denn, Sie schreiben ihm in einer anderen Sprache.
- Nach einem Sprachwechsel wechseln unveränderte Standardvorlagen sofort in die neue Sprache; das Gedächtnis des Assistenten sowie eigene und geänderte Vorlagen übersetzt Claude. Ohne Verbindung zu Claude erscheint auf den Symbolen „Einstellungen“, „Gedächtnis des Assistenten“ und „Dokumentvorlagen“ ein orangefarbenes Ausrufezeichen mit einer Erklärung im Tooltip, und die Übersetzung wird von selbst abgeschlossen, sobald die Verbindung funktioniert (oder nach „Jetzt übersetzen“). Der Unterhaltungsverlauf wird nie übersetzt.
- Neues, schlichteres Logo (flach, zweifarbig, auch klein und im Druck gut lesbar) im Symbol der Anwendung und des Installationsprogramms sowie in der Ansicht „Info“.

Dokumentation für Confluence:

- Confluence-Seiten, die der Assistent aus dem Modell und der Unterhaltung erstellt: Seitencode im Format des Quelltext-Editors (Storage-XHTML) mit Überschriften, Tabellen, Listen, Inhaltsverzeichnis, Panels (Info, Hinweis, Warnung, Tipp, aufklappbarer Abschnitt), Status, Jira-Vorgängen, Erwähnungen von Personen und Codeblöcken; Diagramme aus EA und Bilder als Anhänge. Im Chat eine Seitenkarte mit den Schaltflächen „Seitencode kopieren“ und „Bilderordner öffnen“.
- Veröffentlichung in Confluence über die REST-API (auf Anfrage, immer nach Freigabe, auch im Modus auto): neue Seite unter einer übergeordneten Seite oder Aktualisierung einer bestehenden (nach ID oder Titel), Bilder als Anhänge (vorhandene erhalten eine neue Version). Diagramme werden bei jeder Veröffentlichung neu exportiert, sodass eine erneute Veröffentlichung die Seite nach Änderungen im Modell aktualisiert; der Assistent merkt sich veröffentlichte Seiten.
- Einstellungen, Karte „6. Confluence“: Adresse, Token (Data Center: persönliches Zugriffstoken; Cloud: E-Mail und API-Token), Bereichsschlüssel und übergeordnete Seite, mit der Schaltfläche „Speichern und Confluence-Verbindung prüfen“.

Dokumentvorlagen:

- Vorlagen im Ordner `templates` neben dem Unterhaltungsverlauf, die der Assistent mit Inhalten aus dem Modell füllt (Confluence-Seite oder Word-Dokument): Vorläufige Machbarkeitsanalyse, BRD - Geschäftsanforderungen, Beschreibung der Systemänderungen, Prozessbeschreibung. Vorlagen sind Markdown-Dateien mit Hinweisen für den Assistenten; Sie können sie ändern, eigene hinzufügen oder den Assistenten bitten, eine Vorlage aus einem Beispieldokument zu erstellen.
- Ansicht „Dokumentvorlagen“ in der Navigationsleiste: Liste mit Beschreibung, „Verwenden“ (fügt im Chat eine Anfrage zur Erstellung des Dokuments ein), „Bearbeiten“ und „Vorlagenordner öffnen“.

## Version 0.2 (2026-10-15)

Gedächtnis:

- Gedächtnis über Unterhaltungen hinweg: Der Assistent speichert Festlegungen (Modellierungskonventionen, Präferenzen, Korrekturen, Fakten zu Projekten, alles nach „merke dir…“) als lokale Markdown-Dateien und kennt sie in jeder neuen Unterhaltung. Eine Festlegung zum selben Thema wird aktualisiert statt verdoppelt, veraltete werden entfernt. Das Speichern einer Festlegung während einer Antwort unterbricht die weitere Arbeit nicht mehr (zuvor gab die API einen Fehler 400 zur Signatur des „thinking“-Blocks zurück).
- Ansicht „Gedächtnis des Assistenten“ in der Navigationsleiste: Liste der Festlegungen mit Datum, Schaltfläche „Entfernen“ und „Gedächtnisordner öffnen“ (Ordner `memory` neben dem Unterhaltungsverlauf, mit der Übersichtsdatei `MEMORY.md`).

Enterprise Architect:

- Führen eines Konnektors entlang einer vorgegebenen Route, z. B. nach einer Skizze auf einem Screenshot: Knickpunkte und Berührungspunkte mit den Seiten der Elemente (Linien nur waagerecht und senkrecht oder beliebig).
- Ändern des Linienverlaufs von Konnektoren im Diagramm (wie Line Style in EA): direkt, automatisch, benutzerdefiniert, orthogonal eckig, orthogonal abgerundet, Baum und lateral; für ausgewählte Konnektoren oder für alle im Diagramm.
- Auswirkungsanalyse einer Änderung: von einem angegebenen Element entlang der Beziehungen in die Tiefe (bis zu 6 Ebenen, in eine oder beide Richtungen, optional nur ausgewählte Beziehungstypen), mit der Beziehung, über die die Auswirkung ankommt, dem Pfad und den Diagrammen, auf denen die betroffenen Elemente liegen; Ergebnis als Beschreibung, Bild, Dokument oder Excel-Datei.
- Wo Operationen (Services) einer Klasse, einer Komponente oder einer Schnittstelle verwendet werden: Sequenzdiagramme (Nachrichten an das Element oder seine Lebenslinie, mit der Operation verknüpft oder mit übereinstimmendem Namen) und Aktivitätsdiagramme (Aktionen, die die Operation aufrufen), mit Paket und Aufrufer, sowie eine Liste der Operationen, die in keinem Diagramm verwendet werden.
- Attribute und Operationen von Klassen (mit Parametern), Kopie eines Pakets mit dem gesamten Inhalt sowie Baseline (gespeicherte Version) eines Pakets. Vor dem Löschen oder einem größeren Umbau eines bestehenden Pakets soll der Assistent eine Baseline erstellen (das ist eine Anweisung an den Assistenten, keine Sperre: Bitten Sie bei wichtigen Paketen ausdrücklich um eine Baseline); wiederhergestellt wird sie in EA (Manage Baselines → Compare/Restore), da die Wiederherstellung per Automatisierung EA zum Hängen bringt.
- Zusammenarbeit mit dem geöffneten EA-Fenster (dasselbe Modell): Der Assistent weiß, welches Diagramm Sie geöffnet haben und was darin ausgewählt ist (auch im Projekt-Browser), öffnet Diagramme, zeigt Elemente im Projekt-Browser an und aktualisiert nach dem Speichern die geänderten Diagramme und Pakete in Ihrem EA-Fenster selbst, ohne F5.
- Änderungsprotokoll: jede Speicherung des Assistenten im Modell (auch fehlgeschlagene) in der Datei `dziennik_zmian.jsonl` neben dem Unterhaltungsverlauf; Ansicht „Änderungsprotokoll“ in der Navigationsleiste mit Ergebnis, Zeit, Konto und Datenbank sowie Export nach Excel.
- Schlüsselwörter von Elementen (Feld Keywords in EA) beim Anlegen und Bearbeiten.
- Diagrammlegende mit automatischer Einfärbung wie in EA: Filter nach einer Eigenschaft des Elements (standardmäßig Schlüsselwort, auch Stereotyp, Status, Typ) oder des Konnektors (z. B. Stereotyp), Einträge mit Wert, Farbe und Beschriftung.
- Ändern von Name, Beschreibung und Stereotyp eines bestehenden Diagramms.
- Ausblenden ausgewählter Konnektoren in einem Diagramm (und erneutes Einblenden): Die Beziehungen bleiben im Modell und in anderen Diagrammen erhalten, sodass sich ein dichtes Diagramm entlasten lässt, ohne Beziehungen zu löschen.

- Ändern von Position und Größe eines Elements, das bereits im Diagramm ist (z. B. Vergrößern eines Rahmens, eines Pools oder eines Teilprozesses), ohne es aus dem Diagramm zu entfernen; Konnektoren bleiben erhalten.
- Verknüpfen eines Elements (z. B. eines BPMN-Teilprozesses) mit einem bestehenden Diagramm als Composite-Diagramm, sodass ein Doppelklick das Diagramm öffnet. Die Operation war in der Anwendung vorhanden, aber der Assistent kannte sie nicht und verwies auf eine manuelle Änderung in EA.
- Eingebettete BPMN-Teilprozesse (embedded): Schritte als Kindelemente des Teilprozesses, in dessen Rechteck im selben Diagramm gezeichnet (so wie der BPMN-Import aus Camunda sie speichert); neben eingeklappten Teilprozessen mit Schritten in einem separaten Diagramm.
- Hinzufügen von Schritten zu einem bestehenden Teilprozess: Das Kindelement wird im Paket des Teilprozesses angelegt und mit ihm verknüpft (wie in der EA-Datenbank). Zuvor lehnte EA dies mit dem Fehler „Element locked“ ab, obwohl die Sperre vom Assistenten selbst gehalten wurde.

Start:

- Das Schließen der Anwendung schließt auch ihre EA-Sitzung. Zuvor blieb der EA-Prozess im Hintergrund und belegte eine Lizenz, sodass weitere EA-Sitzungen endlos warten konnten.
- Das Fenster öffnet sich auch auf weniger leistungsfähigen Grafikkarten (z. B. in Wine über Vulkan): Die Anwendung verlangt von der Karte nur das, was die Karte unterstützt.
- Kommunikation mit EA über COM ohne die Systembibliothek propsys, sodass sie auch in Wine funktioniert.

Dokumente und Bilder:

- Word-Dokumente (DOCX), die der Assistent erstellt: Überschriften, Absätze, Listen, Tabellen, Diagramme aus EA und zuvor gezeichnete Bilder; im Chat eine Dateikarte mit den Schaltflächen „Öffnen“ und „Speichern unter…“.
- Excel-Arbeitsmappen (XLSX), die der Assistent erstellt: mehrere Tabellenblätter, Kopfzeilen mit Filter und fixierter erster Zeile, Zahlen als Zahlen (z. B. Elementliste, Matrix, Ergebnis der Auswirkungsanalyse).
- Vom Assistenten gezeichnete Bilder (z. B. Matrix aus Anwendungsfällen und Anforderungen, Tabelle, Diagramm, Schema) als PNG im Chat, mit der Schaltfläche „Speichern unter…“; der Assistent sieht das Ergebnis und kann es verbessern.

Kontakt:

- Ansicht „Fehler melden“ in der Navigationsleiste: Betreff, Beschreibung, Bilder (in das Fenster gezogen, aus der Zwischenablage eingefügt oder von der Festplatte gewählt, mit Miniaturansichten), optionale E-Mail für eine Antwort und technische Informationen (ohne Passwörter und Schlüssel). „Meldung senden“ öffnet im Windows-E-Mail-Programm eine fertige Nachricht an mariusz@loonar.eu mit den Bildern als Anhängen (Simple MAPI); unterstützt das E-Mail-Programm dies nicht, öffnet es die Nachricht ohne Bilder und den Ordner mit den Bildern zum Hineinziehen.
- Einstellungen, Karte „5. Neue Versionen (Newsletter)“: E-Mail-Adresse und Einwilligung zu Informationen über neue Versionen; die Auswahl wird in den Einstellungen gespeichert, An- und Abmeldung gehen als Nachricht aus dem E-Mail-Programm an den Autor.

Erscheinungsbild:

- Neues Fensterdesign, Funktionen unverändert: dunkelblaue Navigationsleiste links mit Symbolen (Chat, Einstellungen, Versionshinweise, Info) statt Registerkarten oben, warmer heller Hintergrund wie in Claude, Schriftart Segoe UI mit halbfetten Überschriften.
- Unterhaltungsliste gruppiert nach „Heute“, „Gestern“, „Letzte 7 Tage“ und „Älter“; neue Unterhaltung mit der Schaltfläche „+“, ausgewählte Unterhaltung hervorgehoben, Statuspunkte unverändert.
- Kopfzeile der Unterhaltung mit Titel und der Anzeige „Claude arbeitet“, während der Assistent arbeitet.
- Ihre Nachrichten in einer beigen Sprechblase wie in Claude; Antworten des Assistenten mit dem Anwendungssymbol, in angenehmer Lesebreite.
- Nachrichtenfeld als eine Karte: Text, Schaltfläche zum Einfügen, Modell und Freigabemodus sowie runde Schaltflächen zum Senden und Abbrechen.
- Schrittliste als Karte: während der Arbeit aufgeklappt, nach Abschluss auf eine Zeile eingeklappt (ein Klick auf die Kopfzeile klappt sie auf).
- Bildschirm für eine neue Unterhaltung mit Logo und einem Hinweis, was Sie tun können.
- Einstellungen als weiße Karten mit Schatten; Versionshinweise und Info mit Seitenrändern.

Einstellungen:

- Neutrale Standardwerte, die nur verwendet werden, wenn noch keine Einstellungen vorhanden sind (bestehende werden weder vom Setup noch von der Anwendung geändert): Datenbankserver, Datenbankname, EA-Konto und Passwörter leer (beim Server ein grauer Hinweis „z. B. 127.0.0.1“), Port 5432, EA-Modell „Model“, Max. Tokens 10000.
- Der Pfad zu EA.exe kann in einem Dateiauswahldialog angegeben werden (Schaltfläche „…“), auch im Setup.
- Das Sperren von Paketen beim Speichern ist immer aktiviert; die Option wurde aus den Einstellungen entfernt.
- Verweigert EA das Speichern wegen einer Sperre, nennt der Fehler, wer das Element, sein Paket und sein Diagramm sperrt (Konto oder Gruppe, seit wann), oder dass im Modell keine Sperre besteht.

Installation:

- Seite „Neu in Version …“ im Setup, direkt nach der Begrüßung: bei einer Aktualisierung die Änderungen seit der installierten Version, bei einer Neuinstallation die Änderungen der aktuellen Version.
- Fortschrittsbalken für Installation und Deinstallation (Prozent und aktueller Schritt) statt eines sich drehenden Kreises.

## Version 0.1 (2026-10-13)

Erste Version, vollständig: Alle Funktionen in der folgenden Liste sind geprüft. Eigenständige Desktopanwendung für die Arbeit mit einem Enterprise-Architect-Modell mithilfe von Claude Haiku 5.5.

### Funktioniert (geprüft)

Modell und Unterhaltung:

- Unterhaltung mit dem Modell Claude Haiku 5.5 über die Anthropic-API, mit Unterstützung für die Workspace-ID.
- Nachrichten während der Arbeit des Assistenten ergänzen: Die Nachricht kommt in eine Warteschlange, und der Assistent berücksichtigt sie beim nächsten Schritt, ohne Unterbrechung.
- Schrittliste im Chat während der Arbeit des Assistenten: was er tut (Datenbankabfrage, Umfang der Speicherung in EA, Abruf eines Diagramms), wie lange es dauert und welches Ergebnis jeder Schritt hatte. Die Liste wird nicht im Unterhaltungsverlauf gespeichert.
- Arbeit in mehreren Unterhaltungen gleichzeitig: Während der Assistent arbeitet, können Sie eine neue Unterhaltung beginnen oder zu einer anderen zurückkehren, und die begonnene Arbeit läuft im Hintergrund weiter (orangefarbener Punkt neben dem Namen der Unterhaltung während der Arbeit, grüner nach deren Abschluss, bis Sie die Unterhaltung öffnen). Speicherungen in EA aus verschiedenen Unterhaltungen werden nacheinander ausgeführt.
- Schaltfläche „Abbrechen“: beendet die Arbeit nach dem aktuellen Schritt; weitere geplante Schritte werden nicht ausgeführt.
- Freigabemodi für das Speichern: ask (Panel mit Beschreibung der Änderungen und den Schaltflächen „Freigeben“ / „Ablehnen“ vor jeder Speicherung; eine abgelehnte Speicherung ändert das Modell nicht), plan (eine Rückfrage vor der ersten Speicherung in einer Antwort, weitere Speicherungen dieser Antwort ohne Rückfrage) und auto (ohne Rückfrage).
- Der Assistent kennt die Anwendung, in der er läuft: Fragen zu seinen Funktionen und Einschränkungen beantwortet er auf Grundlage dieser Versionshinweise.
- Der Assistent bezeichnet Modellobjekte mit Name, Paket und GUID, ohne interne Nummern aus der Datenbank.

Enterprise Architect:

- Lesen des EA-Modells aus einer PostgreSQL-Datenbank (Pakete, Elemente, Diagramme, Konnektoren). Die Datenbanksitzung ist schreibgeschützt. Ein anderer Datenbankport als 5432 funktioniert auch beim Speichern über EA.
- Speichern im EA-Modell über COM: Der Assistent erstellt Pakete, Diagramme, Elemente und Konnektoren und ordnet Elemente im Diagramm an. Er meldet sich mit dem EA-Konto aus den Einstellungen an und hebt nach dem Speichern die Sperren auf, damit sich das Ergebnis sofort bearbeiten lässt.
- Dauerhafte EA-Sitzung für die gesamte Anwendung: Der erste EA-Aufruf dauert einige bis über zehn Sekunden (Start von EA und Öffnen des Modells), die folgenden unter einer Sekunde. Die Anwendung kommuniziert mit EA direkt über COM, ohne PowerShell. Die Sitzung wird nach 10 Minuten Inaktivität und beim Schließen der Anwendung beendet.
- Zeichnen durch den Assistenten direkt, ohne separate Importer: aus einer BPMN-Datei (Elemente an den Positionen aus der Datei), aus einem Foto, aus einer Skizze und allein aus einer Beschreibung.
- Korrekte BPMN-2.0-Typen in EA: Tasks, Teilprozesse, Ereignisse, Gateways und Flüsse, mit Stereotypen und Tags.
- Verschieben bestehender Elemente, Diagramme und Pakete in ein anderes Paket; Konnektoren bleiben erhalten.
- Löschen aus dem Modell: Elemente, Konnektoren, Diagramme und Pakete sowie Entfernen eines Elements aus einem Diagramm.
- Füllfarben: Standardfarbe eines Elements und Farbe eines Elements in einem ausgewählten Diagramm.
- Navigationszellen (navigation cells): Kacheln als Links zu Diagrammen im Startdiagramm, mit Beschriftung (Name des Zieldiagramms) und einem zu dessen Typ passenden Symbol.
- Abruf eines Diagramms aus EA als PNG-Bild (so, wie EA es zeichnet) und Anzeige im Chat, mit den Schaltflächen „Speichern unter…“ und „In voller Größe öffnen“.
- Optimistic Locking beim Speichern: Der Assistent kann Objekte angeben, die sich seit seinem Lesen nicht geändert haben dürfen; bei einer Änderung wird die Speicherung abgelehnt.

Anhänge:

- Dateien in das Fenster ziehen, aus dem Explorer und aus dem Browser (heruntergeladene Dateien): bis zu 5 Dateien, jeweils bis zu 10 MB.
- Einfügen aus der Zwischenablage als Anhang: ein Bild (z. B. ein Screenshot) oder im Explorer kopierte Dateien, mit Strg+V oder der Schaltfläche neben dem Nachrichtenfeld.
- PDF-Dateien und Bilder gehen direkt an das Modell, DOCX als Text, Textdateien (BPMN, XML, CSV, Code…) als Text.
- ZIP-Dateien werden entpackt: Bilder, PDF- und Textdateien daraus gehen an das Modell, als wären sie einzeln angehängt (bis zu 20 Dateien).
- Ein Anhang, dessen Datei nicht mehr auf der Festplatte ist, hinterlässt in der Unterhaltung einen Hinweis und blockiert die weitere Arbeit in dieser Unterhaltung nicht.

Fenster und Chat:

- Mehrzeiliges Nachrichtenfeld: Langer Text wird umbrochen, Eingabe sendet, Umschalt+Eingabe fügt eine neue Zeile ein.
- Pfeil nach oben in einem leeren Nachrichtenfeld ruft Ihre letzte eigene Nachricht aus der Unterhaltung auf; weiteres Drücken geht zu früheren zurück, Pfeil nach unten geht wieder vor.
- Rechtschreibprüfung im Nachrichtenfeld mit dem Windows-Systemwörterbuch, in der Sprache der Systemeinstellungen: Falsch geschriebene Wörter werden rot unterstrichen.
- Formatierung der Antworten des Assistenten im Chat: Fettdruck, Aufzählungen und nummerierte Listen, Tabellen, Überschriften, Inline-Code und Codeblöcke.
- Uhrzeit bei jeder Nachricht im Chat (bei älteren Nachrichten auch das Datum).
- Eine geöffnete Unterhaltung wird vom Ende her angezeigt, also ab den neuesten Nachrichten.
- Kreis mit Pfeil über dem Nachrichtenfeld, wenn der Chat nach oben gescrollt ist: springt zum Ende der Unterhaltung.
- Suche in Unterhaltungen: nach Titel und nach Nachrichteninhalt.
- Umbenennen einer Unterhaltung: Stift neben der ausgewählten Unterhaltung, Eingabe speichert, Esc bricht ab.
- Unterhaltungsverlauf in Dateien gespeichert und Rückkehr zu früheren Unterhaltungen.
- Das Fenster bleibt während der Arbeit des Assistenten bedienbar (die Arbeit läuft im Hintergrund).
- Gleichbleibende Fenstergröße beim Verschieben zwischen Monitoren mit unterschiedlicher Skalierung.
- Helles Farbschema des Fensters, Registerkarten „Versionshinweise“ und „Info“, großes Anwendungslogo auf der Registerkarte „Info“.
- Startbildschirm mit der Grafik der Loonar-Visitenkarte (Globus, Tastatur, Sterne), Name und Version der Anwendung sowie Daten des Eigentümers.
- Start ohne Terminalfenster, Leiste mit den Daten des Eigentümers.
- Anwendungssymbol in der exe-Datei: sichtbar im Explorer und nach dem Anheften an die Taskleiste.

Einstellungen:

- Einstellungen im Fenster in vier umrahmten Gruppen: API-Schlüssel, Modelldatenbank, EA-Konto, Freigabemodus. API-Schlüssel und Passwort des EA-Kontos liegen in der Windows-Anmeldeinformationsverwaltung. Das Claude-Modell wird schreibgeschützt angezeigt.
- Jede Gruppe wird einzeln gespeichert (Schaltflächen „Speichern und … prüfen“), mit dem Testergebnis unter der Gruppe: API-Schlüssel (Anfrage an das Modell), Datenbank (Verbindung und Lesen), EA (Anmeldung über COM mit dem angegebenen Konto).

Installation:

- Setup als Installationsassistent: Lizenz, Zielordner (standardmäßig Program Files), Verknüpfungen (Startmenü, Desktop, Schnellstart), optionale Konfiguration (API-Schlüssel, Datenbank, EA-Konto, Freigabemodus) und Zusammenfassung.
- Aktualisierung durch erneutes Ausführen des Setups: Einstellungen, Unterhaltungsverlauf und Anhänge bleiben erhalten.
- Deinstallation über das Startmenü oder die Windows-Einstellungen; Benutzerdaten und gespeicherte Zugangsdaten werden nur auf Wunsch entfernt.
- Einstellungen, Unterhaltungsverlauf und Anhänge liegen im Benutzerprofil (`%APPDATA%\Loonar EA Assistant`), getrennt für jeden Benutzer des Computers.
- Benutzerhandbuch (PDF) im Installationsordner und im Startmenü.

### Voraussetzungen

- Das EA-Modell ist eine Projektdatei .qea oder .qeax oder liegt in einer PostgreSQL-Datenbank. Ältere Dateiformate (.eap, .eapx, .feap) und andere Datenbanken werden nicht unterstützt; eine .eap/.eapx-Datei überträgt EA mit einem Projekttransfer nach .qea.
- Ein Modell in einer Datenbank muss die Benutzersicherheit aktiviert haben (Enable Security, Modus „Require User Lock to Edit“) und ein Benutzerkonto für den Assistenten (beliebiger Name) mit Passwort und Bearbeitungsrechten enthalten.
- In einer Modelldatei ist die Benutzersicherheit optional; ist sie aktiviert, braucht auch die Datei ein solches Konto.

### Bekannte Einschränkungen

- Der erste EA-Aufruf nach dem Start der Anwendung (oder nach 10 Minuten Pause) dauert einige bis über zehn Sekunden. In dieser Zeit läuft im Hintergrund ein EA-Prozess, der einen Lizenzplatz belegt.
- Große Diagramme speichert der Assistent in Teilen; bei einer sehr langen Antwort kann das Token-Limit nicht ausreichen (Einstellung „Max. Tokens“).
- Schlägt eine der Speicheroperationen fehl, bleiben die vorherigen Operationen derselben Speicherung im Modell; die Meldung nennt, was gespeichert wurde.
- Kursivschrift in den Antworten des Assistenten wird nicht formatiert.
- Auf einem Monitor mit anderer Skalierung als der Hauptmonitor ist das Fensterbild etwas weniger scharf.
- Ist das EA-Fenster nicht geöffnet oder zeigt es ein anderes Modell, sind die vom Assistenten gespeicherten Änderungen nach dem Aktualisieren des Pakets (F5) sichtbar.
- Proxys mit PAC-Skript sowie die NTLM-/Kerberos-Authentifizierung am Proxy werden nicht unterstützt.
- Die TLS-Verschlüsselung in den Einstellungen betrifft das Lesen des Modells durch den Assistenten; die Verbindung von EA selbst zur Datenbank hängt von den Einstellungen in EA ab.
- Das Setup ist noch nicht digital signiert, daher kann Windows SmartScreen beim ersten Start warnen.
