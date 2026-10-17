# Versionshinweise

Alle wesentlichen Änderungen an Loonar Compass (bis Version 0.3 unter dem Namen Loonar EA Assistant).

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
