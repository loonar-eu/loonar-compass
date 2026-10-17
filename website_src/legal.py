# -*- coding: utf-8 -*-
"""Privacy policy and legal notice of loonar.eu in four languages (HTML fragments for build.py).

The Polish text is authoritative in meaning; the English, German and French texts say the same.
Placeholders filled by build.py: {gh_privacy} (GitHub General Privacy Statement), {gh_pages} (GitHub Pages
documentation), {gc_privacy} (GoatCounter privacy policy) and {optout} (the "do not count my visits" switch,
shown only with JavaScript). The owner data below fills the remaining placeholders.
Re-check sections 2-5 whenever the hosting, the statistics (site.js) or the links on the website change.
"""

OWNER = {
    "firm": "Loonar Mariusz Głowacki",
    "person": "Mariusz Głowacki",
    "street": "ul. Gombrowicza 12",
    "city": "73-110 Stargard",
    "email": "mariusz@loonar.eu",
    "nip": "8542139307",
    "regon": "320090308",
}
UPDATED = {"en": "17 October 2026", "pl": "17 października 2026", "de": "17. Oktober 2026", "fr": "17 octobre 2026"}
UPDATED_ISO = "2026-10-17"

PRIVACY = {
    "en": {
        "title": "Privacy policy",
        "desc": "How loonar.eu and Loonar Compass handle personal data.",
        "body": """
<h2>1. Controller</h2>
<p>{person}, running the business {firm}, {street}, {city}, Poland, NIP (tax ID) {nip}, REGON {regon}. Contact: <a href="mailto:{email}">{email}</a>.</p>
<h2>2. Hosting of this website (GitHub Pages)</h2>
<p>The website is hosted on GitHub Pages, a service of GitHub, Inc., 88 Colin P. Kelly Jr. Street, San Francisco, CA 94107, USA. When you open a page, your browser connects to GitHub’s servers, which receive your IP address and the usual technical data of the request (requested address, time, browser, referring page). GitHub processes your IP address to deliver the website and, according to the <a href="{gh_pages}">GitHub Pages documentation</a>, logs it for security purposes; we have no access to these logs. We use GitHub Pages on the basis of our legitimate interest in delivering the website securely and reliably (Art. 6(1)(f) GDPR).</p>
<p>GitHub processes personal data in the USA, among other places. For transfers from the European Union, it relies on the EU-U.S. Data Privacy Framework, in which it participates, and on the European Commission’s standard contractual clauses. Details: <a href="{gh_privacy}">GitHub General Privacy Statement</a>.</p>
<h2>3. Visit statistics (GoatCounter)</h2>
<p>To know how many people visit the website and download the application, we use GoatCounter (goatcounter.com, run by Martin Tournoij, Ireland; data stored in the EU, on Hetzner servers in Finland and Germany). The counter is part of this website’s own code: no external script is loaded, and no cookies or similar identifiers are used.</p>
<p>When you open a page, your browser sends GoatCounter the page address and title, the referring page and the screen width; when you click the download button or a link to GitHub, suppi.pl, the privacy policy or the legal notice, it sends the name of that event and the page on which you clicked. From the request itself, GoatCounter determines the browser and operating system, the language and the country (from the IP address). It stores only aggregate counts of these data – not your IP address, not the full browser identification (User-Agent) and no identifier of you. To avoid counting a repeated visit twice, it keeps the combination of IP address and browser identification in memory only, for at most 8 hours. Without JavaScript, nothing is counted. Legal basis: our legitimate interest in anonymous statistics on the use of the website (Art. 6(1)(f) GDPR). More: <a href="{gc_privacy}">GoatCounter privacy policy</a>.</p>
{optout}
<h2>4. Data stored in your browser</h2>
<p>If you choose a language (or close the language suggestion), your browser remembers the choice in its local storage (localStorage, key <code>loonar-lang</code>); if you switch the counting off, it remembers that too (key <code>loonar-nocount</code>). These entries never reach us, and you can delete them in your browser settings.</p>
<h2>5. Downloads and links to other services</h2>
<p>You download the installer from GitHub Releases (github.com), and the links to the source code also lead to GitHub; there, GitHub, Inc. processes your data under its own privacy statement. The button in the “Support the project” section leads to suppi.pl, a service that handles payments under its own terms and privacy policy, and the link in the “About me” section leads to LinkedIn. If you support the project, we receive only the data that suppi.pl passes on to the recipient of the support, and we use it to record the payment and, where the law requires it, for accounting and tax purposes (Art. 6(1)(c) GDPR).</p>
<h2>6. E-mails and bug reports</h2>
<p>When you write to us or send a bug report from the application, we process your e-mail address, your name if given, the message, attached screenshots and, if you leave the option on, technical information (application version, Windows version, AI model, approval mode, last message). We use them to answer and to improve the application (Art. 6(1)(f) GDPR). We keep them until the matter is closed and then for up to 2 years. Please do not include other people’s data or confidential information.</p>
<h2>7. News about new versions</h2>
<p>If you sign up in the application (Settings → New releases), we use your e-mail address only to send news about new versions, based on your consent (Art. 6(1)(a) GDPR), until you withdraw it – by unticking the option in the application or by e-mail. Withdrawal does not affect messages sent before. We keep the sign-up and withdrawal messages as proof for up to 3 years (Art. 6(1)(f) GDPR).</p>
<h2>8. The application</h2>
<p>Loonar Compass sends us no data (no telemetry). Settings, conversations, memory and templates stay on your computer. To answer, the application sends the conversation, attachments, remembered notes, the templates in use and the model excerpts it needs (including diagram images) directly to Anthropic (Claude API) under your API key and your agreement with Anthropic; for that processing we are not the controller. Confluence pages go only to the server you configure.</p>
<h2>9. Recipients</h2>
<p>The recipients of data are our service providers: GitHub, Inc. (website hosting and the installer download), GoatCounter (visit statistics) and our e-mail provider. Where data is processed outside the European Economic Area (for GitHub, in the USA), this happens on the basis of an adequacy decision of the European Commission (EU-U.S. Data Privacy Framework) or of the European Commission’s standard contractual clauses.</p>
<h2>10. Your rights</h2>
<p>You have the right to access, rectify and erase your data, to restrict processing, to data portability, to object and to withdraw consent at any time. The visit statistics contain no data that identifies you, so we cannot attribute them to you (Art. 11 GDPR). You can lodge a complaint with the President of the Personal Data Protection Office (UODO), ul. Stawki 2, 00-193 Warsaw, Poland. Providing data is voluntary. We do not use automated decision-making or profiling.</p>
""",
    },
    "pl": {
        "title": "Polityka prywatności",
        "desc": "Jak loonar.eu i Loonar Compass przetwarzają dane osobowe.",
        "body": """
<h2>1. Administrator</h2>
<p>{person}, prowadzący działalność gospodarczą pod firmą {firm}, {street}, {city}, NIP {nip}, REGON {regon}. Kontakt: <a href="mailto:{email}">{email}</a>.</p>
<h2>2. Hosting strony (GitHub Pages)</h2>
<p>Strona jest hostowana w usłudze GitHub Pages firmy GitHub, Inc., 88 Colin P. Kelly Jr. Street, San Francisco, CA 94107, USA. Gdy otwierasz stronę, Twoja przeglądarka łączy się z serwerami GitHub, które otrzymują Twój adres IP i zwykłe dane techniczne żądania (adres strony, czas, przeglądarkę, stronę odsyłającą). GitHub przetwarza adres IP, aby dostarczyć stronę, a według <a href="{gh_pages}">dokumentacji GitHub Pages</a> zapisuje go w logach w celach bezpieczeństwa; nie mamy dostępu do tych logów. Z GitHub Pages korzystamy na podstawie naszego prawnie uzasadnionego interesu, jakim jest bezpieczne i niezawodne udostępnianie strony (art. 6 ust. 1 lit. f RODO).</p>
<p>GitHub przetwarza dane osobowe m.in. w USA. Przy przekazywaniu danych z Unii Europejskiej opiera się na ramach ochrony danych UE–USA (EU-U.S. Data Privacy Framework), w których uczestniczy, oraz na standardowych klauzulach umownych Komisji Europejskiej. Szczegóły: <a href="{gh_privacy}">oświadczenie GitHub o ochronie prywatności</a> (GitHub General Privacy Statement).</p>
<h2>3. Statystyki odwiedzin (GoatCounter)</h2>
<p>Aby wiedzieć, ile osób odwiedza stronę i pobiera aplikację, korzystamy z GoatCounter (goatcounter.com; usługę prowadzi Martin Tournoij, Irlandia; dane są przechowywane w UE, na serwerach Hetzner w Finlandii i w Niemczech). Licznik jest częścią kodu tej strony: nie ładujemy zewnętrznego skryptu i nie używamy plików cookie ani podobnych identyfikatorów.</p>
<p>Przy otwarciu strony przeglądarka wysyła do GoatCounter adres i tytuł strony, adres strony odsyłającej i szerokość ekranu, a przy kliknięciu przycisku pobierania lub odnośnika do GitHub, suppi.pl, polityki prywatności albo noty prawnej – nazwę tego zdarzenia i stronę, na której kliknięto. Z samego żądania GoatCounter ustala przeglądarkę i system operacyjny, język oraz kraj (na podstawie adresu IP). Przechowuje wyłącznie zbiorcze liczniki tych danych – nie zapisuje adresu IP, pełnego identyfikatora przeglądarki (User-Agent) ani żadnego identyfikatora odwiedzającego. Aby nie liczyć dwa razy tej samej wizyty, przechowuje połączenie adresu IP i identyfikatora przeglądarki tylko w pamięci, najwyżej przez 8 godzin. Bez JavaScriptu nic nie jest liczone. Podstawa prawna: nasz prawnie uzasadniony interes, jakim są anonimowe statystyki korzystania ze strony (art. 6 ust. 1 lit. f RODO). Więcej: <a href="{gc_privacy}">polityka prywatności GoatCounter</a>.</p>
{optout}
<h2>4. Dane zapisywane w przeglądarce</h2>
<p>Jeśli wybierzesz język (albo zamkniesz podpowiedź języka), przeglądarka zapamięta wybór w swojej pamięci lokalnej (localStorage, klucz <code>loonar-lang</code>); jeśli wyłączysz liczenie odwiedzin, zapamięta również to (klucz <code>loonar-nocount</code>). Te informacje nie trafiają do nas; możesz je usunąć w ustawieniach przeglądarki.</p>
<h2>5. Pobieranie i odnośniki do innych serwisów</h2>
<p>Instalator pobierasz z GitHub Releases (github.com), a odnośniki do kodu źródłowego również prowadzą do GitHub; tam Twoje dane przetwarza GitHub, Inc. według własnego oświadczenia o ochronie prywatności. Przycisk w sekcji „Wesprzyj projekt” prowadzi do serwisu suppi.pl, który obsługuje wpłaty według własnego regulaminu i polityki prywatności, a odnośnik w sekcji „O mnie” – do serwisu LinkedIn. Jeśli wesprzesz projekt, otrzymujemy tylko te dane, które suppi.pl przekazuje odbiorcy wsparcia, i wykorzystujemy je do odnotowania wpłaty oraz – w zakresie wymaganym przepisami – do celów rachunkowych i podatkowych (art. 6 ust. 1 lit. c RODO).</p>
<h2>6. E-maile i zgłoszenia błędów</h2>
<p>Gdy do nas piszesz albo wysyłasz zgłoszenie błędu z aplikacji, przetwarzamy Twój adres e-mail, imię i nazwisko (jeśli je podasz), treść wiadomości, dołączone zrzuty ekranu oraz – jeśli zostawisz tę opcję włączoną – informacje techniczne (wersja aplikacji, wersja Windows, model AI, tryb zatwierdzania, ostatni komunikat). Wykorzystujemy je, aby odpowiedzieć i poprawić aplikację (art. 6 ust. 1 lit. f RODO). Przechowujemy je do zamknięcia sprawy, a następnie do 2 lat. Nie dołączaj danych innych osób ani informacji poufnych.</p>
<h2>7. Informacje o nowych wersjach</h2>
<p>Jeśli zapiszesz się w aplikacji (Ustawienia → Nowe wydania), wykorzystamy Twój adres e-mail wyłącznie do wysyłania informacji o nowych wersjach, na podstawie Twojej zgody (art. 6 ust. 1 lit. a RODO), do czasu jej wycofania – odznaczeniem opcji w aplikacji albo e-mailem. Wycofanie zgody nie wpływa na wcześniejsze wysyłki. Wiadomości z zapisem i rezygnacją przechowujemy jako dowód do 3 lat (art. 6 ust. 1 lit. f RODO).</p>
<h2>8. Aplikacja</h2>
<p>Loonar Compass nie wysyła nam żadnych danych (brak telemetrii). Ustawienia, rozmowy, pamięć i szablony zostają na Twoim komputerze. Aby odpowiedzieć, aplikacja wysyła treść rozmowy, załączniki, zapamiętane ustalenia, używane szablony i potrzebne fragmenty modelu (w tym obrazki diagramów) bezpośrednio do Anthropic (Claude API) – na Twój klucz API i na podstawie Twojej umowy z Anthropic; w tym zakresie nie jesteśmy administratorem danych. Strony Confluence trafiają wyłącznie na serwer, który wskażesz.</p>
<h2>9. Odbiorcy danych</h2>
<p>Odbiorcami danych są nasi usługodawcy: GitHub, Inc. (hosting strony i pobieranie instalatora), GoatCounter (statystyki odwiedzin) i dostawca poczty e-mail. Jeśli dane są przetwarzane poza Europejskim Obszarem Gospodarczym (w przypadku GitHub – w USA), odbywa się to na podstawie decyzji Komisji Europejskiej stwierdzającej odpowiedni stopień ochrony (EU-U.S. Data Privacy Framework) albo standardowych klauzul umownych Komisji Europejskiej.</p>
<h2>10. Twoje prawa</h2>
<p>Masz prawo dostępu do danych, ich sprostowania i usunięcia, ograniczenia przetwarzania, przenoszenia danych, sprzeciwu oraz wycofania zgody w dowolnym momencie. Statystyki odwiedzin nie zawierają danych pozwalających Cię zidentyfikować, dlatego nie możemy ich przypisać do Ciebie (art. 11 RODO). Możesz złożyć skargę do Prezesa Urzędu Ochrony Danych Osobowych, ul. Stawki 2, 00-193 Warszawa. Podanie danych jest dobrowolne. Nie podejmujemy zautomatyzowanych decyzji i nie profilujemy.</p>
""",
    },
    "de": {
        "title": "Datenschutzerklärung",
        "desc": "Wie loonar.eu und Loonar Compass mit personenbezogenen Daten umgehen.",
        "body": """
<h2>1. Verantwortlicher</h2>
<p>{person}, Einzelunternehmer unter der Firma {firm}, {street}, {city}, Polen, NIP (Steuernummer) {nip}, REGON {regon}. Kontakt: <a href="mailto:{email}">{email}</a>.</p>
<h2>2. Hosting dieser Website (GitHub Pages)</h2>
<p>Die Website wird bei GitHub Pages gehostet, einem Dienst der GitHub, Inc., 88 Colin P. Kelly Jr. Street, San Francisco, CA 94107, USA. Wenn Sie eine Seite aufrufen, verbindet sich Ihr Browser mit den Servern von GitHub; diese erhalten Ihre IP-Adresse und die üblichen technischen Daten der Anfrage (aufgerufene Adresse, Zeitpunkt, Browser, verweisende Seite). GitHub verarbeitet Ihre IP-Adresse, um die Website auszuliefern, und protokolliert sie laut der <a href="{gh_pages}">Dokumentation von GitHub Pages</a> zu Sicherheitszwecken; wir haben keinen Zugriff auf diese Protokolle. Wir nutzen GitHub Pages auf Grundlage unseres berechtigten Interesses an einer sicheren und zuverlässigen Bereitstellung der Website (Art. 6 Abs. 1 lit. f DSGVO).</p>
<p>GitHub verarbeitet personenbezogene Daten unter anderem in den USA. Für Übermittlungen aus der Europäischen Union stützt sich GitHub auf das EU-U.S. Data Privacy Framework, an dem es teilnimmt, sowie auf die Standardvertragsklauseln der Europäischen Kommission. Einzelheiten: <a href="{gh_privacy}">Datenschutzerklärung von GitHub</a> (GitHub General Privacy Statement).</p>
<h2>3. Besuchsstatistik (GoatCounter)</h2>
<p>Um zu wissen, wie viele Menschen die Website besuchen und die Anwendung herunterladen, nutzen wir GoatCounter (goatcounter.com, betrieben von Martin Tournoij, Irland; Speicherung der Daten in der EU, auf Servern von Hetzner in Finnland und Deutschland). Der Zähler ist Teil des eigenen Codes dieser Website: Es wird kein externes Skript geladen, und es werden keine Cookies oder ähnlichen Kennungen verwendet.</p>
<p>Beim Aufruf einer Seite sendet Ihr Browser an GoatCounter Adresse und Titel der Seite, die verweisende Seite und die Bildschirmbreite; beim Klick auf die Download-Schaltfläche oder auf einen Link zu GitHub, suppi.pl, zur Datenschutzerklärung oder zum Impressum den Namen dieses Ereignisses und die Seite, auf der geklickt wurde. Aus der Anfrage selbst ermittelt GoatCounter Browser und Betriebssystem, Sprache und Land (anhand der IP-Adresse). Gespeichert werden nur zusammengefasste Zählungen dieser Angaben – weder Ihre IP-Adresse noch die vollständige Browserkennung (User-Agent) noch eine Kennung Ihrer Person. Damit ein wiederholter Besuch nicht doppelt gezählt wird, hält GoatCounter die Kombination aus IP-Adresse und Browserkennung nur im Arbeitsspeicher, höchstens 8 Stunden lang. Ohne JavaScript wird nichts gezählt. Rechtsgrundlage: unser berechtigtes Interesse an anonymen Statistiken zur Nutzung der Website (Art. 6 Abs. 1 lit. f DSGVO). Mehr: <a href="{gc_privacy}">Datenschutzerklärung von GoatCounter</a>.</p>
{optout}
<h2>4. Im Browser gespeicherte Daten</h2>
<p>Wenn Sie eine Sprache wählen (oder den Sprachhinweis schließen), speichert Ihr Browser diese Wahl in seinem lokalen Speicher (localStorage, Schlüssel <code>loonar-lang</code>); wenn Sie die Zählung abschalten, speichert er auch das (Schlüssel <code>loonar-nocount</code>). Diese Einträge werden nicht an uns übertragen und können in den Browsereinstellungen gelöscht werden.</p>
<h2>5. Downloads und Links zu anderen Diensten</h2>
<p>Das Installationsprogramm laden Sie aus GitHub Releases (github.com) herunter, und auch die Links zum Quellcode führen zu GitHub; dort verarbeitet GitHub, Inc. Ihre Daten nach seiner eigenen Datenschutzerklärung. Die Schaltfläche im Abschnitt „Projekt unterstützen“ führt zu suppi.pl, einem Dienst, der Zahlungen nach eigenen Bedingungen und eigener Datenschutzerklärung abwickelt, der Link im Abschnitt „Über mich“ zu LinkedIn. Wenn Sie das Projekt unterstützen, erhalten wir nur die Daten, die suppi.pl an den Empfänger der Unterstützung weitergibt, und verwenden sie, um die Zahlung zu erfassen und – soweit gesetzlich vorgeschrieben – für Buchhaltung und Steuern (Art. 6 Abs. 1 lit. c DSGVO).</p>
<h2>6. E-Mails und Fehlerberichte</h2>
<p>Wenn Sie uns schreiben oder aus der Anwendung einen Fehlerbericht senden, verarbeiten wir Ihre E-Mail-Adresse, Ihren Namen (falls angegeben), die Nachricht, beigefügte Bildschirmfotos und – sofern Sie die Option aktiviert lassen – technische Angaben (Version der Anwendung, Windows-Version, KI-Modell, Freigabemodus, letzte Meldung). Wir nutzen sie, um zu antworten und die Anwendung zu verbessern (Art. 6 Abs. 1 lit. f DSGVO). Wir speichern sie bis zum Abschluss der Angelegenheit und danach bis zu 2 Jahre. Bitte fügen Sie keine Daten anderer Personen und keine vertraulichen Informationen bei.</p>
<h2>7. Informationen zu neuen Versionen</h2>
<p>Wenn Sie sich in der Anwendung anmelden (Einstellungen → Neue Versionen), nutzen wir Ihre E-Mail-Adresse ausschließlich für Informationen zu neuen Versionen, auf Grundlage Ihrer Einwilligung (Art. 6 Abs. 1 lit. a DSGVO), bis Sie diese widerrufen – durch Abwählen der Option in der Anwendung oder per E-Mail. Der Widerruf berührt die Rechtmäßigkeit früherer Zusendungen nicht. Die An- und Abmeldenachrichten bewahren wir als Nachweis bis zu 3 Jahre auf (Art. 6 Abs. 1 lit. f DSGVO).</p>
<h2>8. Die Anwendung</h2>
<p>Loonar Compass sendet uns keine Daten (keine Telemetrie). Einstellungen, Unterhaltungen, Gedächtnis und Vorlagen bleiben auf Ihrem Computer. Um zu antworten, sendet die Anwendung die Unterhaltung, Anhänge, gemerkte Notizen, die verwendeten Vorlagen und die nötigen Modellausschnitte (einschließlich Diagrammbildern) direkt an Anthropic (Claude-API) – mit Ihrem API-Schlüssel und auf Grundlage Ihres Vertrags mit Anthropic; insoweit sind wir nicht Verantwortlicher. Confluence-Seiten gehen nur an den von Ihnen angegebenen Server.</p>
<h2>9. Empfänger</h2>
<p>Empfänger der Daten sind unsere Dienstleister: GitHub, Inc. (Hosting der Website und Download des Installationsprogramms), GoatCounter (Besuchsstatistik) und unser E-Mail-Anbieter. Werden Daten außerhalb des Europäischen Wirtschaftsraums verarbeitet (bei GitHub in den USA), geschieht dies auf Grundlage eines Angemessenheitsbeschlusses der Europäischen Kommission (EU-U.S. Data Privacy Framework) oder der Standardvertragsklauseln der Europäischen Kommission.</p>
<h2>10. Ihre Rechte</h2>
<p>Sie haben das Recht auf Auskunft, Berichtigung und Löschung, Einschränkung der Verarbeitung, Datenübertragbarkeit, Widerspruch sowie auf jederzeitigen Widerruf einer Einwilligung. Die Besuchsstatistik enthält keine Daten, mit denen wir Sie identifizieren könnten; wir können sie Ihnen daher nicht zuordnen (Art. 11 DSGVO). Sie können sich bei der polnischen Datenschutzbehörde (Prezes Urzędu Ochrony Danych Osobowych, ul. Stawki 2, 00-193 Warschau) oder bei der Aufsichtsbehörde Ihres Wohnorts beschweren. Die Angabe von Daten ist freiwillig. Wir nutzen keine automatisierte Entscheidungsfindung und kein Profiling.</p>
""",
    },
    "fr": {
        "title": "Politique de confidentialité",
        "desc": "Comment loonar.eu et Loonar Compass traitent les données personnelles.",
        "body": """
<h2>1. Responsable du traitement</h2>
<p>{person}, entrepreneur individuel exerçant sous la dénomination {firm}, {street}, {city}, Pologne, NIP (numéro fiscal) {nip}, REGON {regon}. Contact : <a href="mailto:{email}">{email}</a>.</p>
<h2>2. Hébergement de ce site (GitHub Pages)</h2>
<p>Le site est hébergé sur GitHub Pages, un service de GitHub, Inc., 88 Colin P. Kelly Jr. Street, San Francisco, CA 94107, États-Unis. Lorsque vous ouvrez une page, votre navigateur se connecte aux serveurs de GitHub, qui reçoivent votre adresse IP et les données techniques habituelles de la requête (adresse demandée, heure, navigateur, page de provenance). GitHub traite votre adresse IP pour fournir le site et, selon la <a href="{gh_pages}">documentation de GitHub Pages</a>, l’enregistre dans ses journaux à des fins de sécurité ; nous n’avons pas accès à ces journaux. Nous utilisons GitHub Pages sur la base de notre intérêt légitime à fournir le site de manière sûre et fiable (art. 6, par. 1, point f du RGPD).</p>
<p>GitHub traite des données personnelles notamment aux États-Unis. Pour les transferts depuis l’Union européenne, il s’appuie sur le cadre de protection des données UE–États-Unis (EU-U.S. Data Privacy Framework), auquel il participe, et sur les clauses contractuelles types de la Commission européenne. Détails : <a href="{gh_privacy}">déclaration de confidentialité de GitHub</a> (GitHub General Privacy Statement).</p>
<h2>3. Statistiques de visite (GoatCounter)</h2>
<p>Pour savoir combien de personnes visitent le site et téléchargent l’application, nous utilisons GoatCounter (goatcounter.com, exploité par Martin Tournoij, Irlande ; données stockées dans l’UE, sur des serveurs Hetzner en Finlande et en Allemagne). Le compteur fait partie du code de ce site : aucun script externe n’est chargé, et aucun cookie ni identifiant similaire n’est utilisé.</p>
<p>À l’ouverture d’une page, votre navigateur envoie à GoatCounter l’adresse et le titre de la page, la page de provenance et la largeur de l’écran ; lors d’un clic sur le bouton de téléchargement ou sur un lien vers GitHub, suppi.pl, la politique de confidentialité ou les mentions légales, il envoie le nom de cet événement et la page sur laquelle vous avez cliqué. À partir de la requête elle-même, GoatCounter détermine le navigateur et le système d’exploitation, la langue et le pays (d’après l’adresse IP). Il ne conserve que des décomptes agrégés de ces données – ni votre adresse IP, ni l’identification complète du navigateur (User-Agent), ni aucun identifiant vous concernant. Pour ne pas compter deux fois une même visite, il garde la combinaison de l’adresse IP et de l’identification du navigateur uniquement en mémoire, pendant 8 heures au plus. Sans JavaScript, rien n’est compté. Base juridique : notre intérêt légitime à disposer de statistiques anonymes sur l’utilisation du site (art. 6, par. 1, point f du RGPD). En savoir plus : <a href="{gc_privacy}">politique de confidentialité de GoatCounter</a>.</p>
{optout}
<h2>4. Données enregistrées dans votre navigateur</h2>
<p>Si vous choisissez une langue (ou fermez la suggestion de langue), votre navigateur mémorise ce choix dans son stockage local (localStorage, clé <code>loonar-lang</code>) ; si vous désactivez le comptage, il le mémorise aussi (clé <code>loonar-nocount</code>). Ces informations ne nous sont pas transmises ; vous pouvez les effacer dans les paramètres du navigateur.</p>
<h2>5. Téléchargements et liens vers d’autres services</h2>
<p>Le programme d’installation se télécharge depuis GitHub Releases (github.com), et les liens vers le code source mènent aussi à GitHub ; GitHub, Inc. y traite vos données selon sa propre déclaration de confidentialité. Le bouton de la section « Soutenir le projet » mène à suppi.pl, un service qui gère les paiements selon ses propres conditions et sa propre politique de confidentialité, et le lien de la section « À propos de moi » mène à LinkedIn. Si vous soutenez le projet, nous ne recevons que les données que suppi.pl transmet au bénéficiaire du soutien, et nous les utilisons pour enregistrer le paiement et, dans la mesure exigée par la loi, à des fins comptables et fiscales (art. 6, par. 1, point c du RGPD).</p>
<h2>6. E-mails et signalements de bugs</h2>
<p>Lorsque vous nous écrivez ou envoyez un signalement depuis l’application, nous traitons votre adresse e-mail, votre nom s’il est indiqué, le message, les captures d’écran jointes et, si vous laissez l’option activée, des informations techniques (version de l’application, version de Windows, modèle d’IA, mode de validation, dernier message). Nous les utilisons pour vous répondre et améliorer l’application (art. 6, par. 1, point f du RGPD). Nous les conservons jusqu’à la clôture du dossier, puis jusqu’à 2 ans. Merci de ne pas joindre de données d’autres personnes ni d’informations confidentielles.</p>
<h2>7. Informations sur les nouvelles versions</h2>
<p>Si vous vous inscrivez dans l’application (Paramètres → Nouvelles versions), nous utilisons votre adresse e-mail uniquement pour vous informer des nouvelles versions, sur la base de votre consentement (art. 6, par. 1, point a du RGPD), jusqu’à son retrait – en décochant l’option dans l’application ou par e-mail. Le retrait n’affecte pas les envois antérieurs. Nous conservons les messages d’inscription et de désinscription comme preuve pendant 3 ans au plus (art. 6, par. 1, point f du RGPD).</p>
<h2>8. L’application</h2>
<p>Loonar Compass ne nous envoie aucune donnée (aucune télémétrie). Les paramètres, conversations, la mémoire et les modèles restent sur votre ordinateur. Pour répondre, l’application envoie la conversation, les pièces jointes, les notes mémorisées, les modèles de document utilisés et les extraits du modèle nécessaires (y compris les images de diagrammes) directement à Anthropic (API Claude), avec votre clé API et en vertu de votre contrat avec Anthropic ; pour ce traitement, nous ne sommes pas responsable du traitement. Les pages Confluence sont envoyées uniquement au serveur que vous indiquez.</p>
<h2>9. Destinataires</h2>
<p>Les destinataires des données sont nos prestataires : GitHub, Inc. (hébergement du site et téléchargement du programme d’installation), GoatCounter (statistiques de visite) et notre prestataire de messagerie. Si des données sont traitées hors de l’Espace économique européen (pour GitHub, aux États-Unis), c’est sur la base d’une décision d’adéquation de la Commission européenne (EU-U.S. Data Privacy Framework) ou des clauses contractuelles types de la Commission européenne.</p>
<h2>10. Vos droits</h2>
<p>Vous disposez d’un droit d’accès, de rectification, d’effacement, de limitation du traitement, de portabilité, d’opposition et du droit de retirer votre consentement à tout moment. Les statistiques de visite ne contiennent aucune donnée permettant de vous identifier ; nous ne pouvons donc pas vous les attribuer (art. 11 du RGPD). Vous pouvez introduire une réclamation auprès de l’autorité polonaise de protection des données (Prezes Urzędu Ochrony Danych Osobowych, ul. Stawki 2, 00-193 Varsovie) ou de l’autorité de votre pays de résidence (en France : la CNIL). La fourniture des données est facultative. Nous ne procédons à aucune prise de décision automatisée ni à aucun profilage.</p>
""",
    },
}

LEGAL = {
    "en": {
        "title": "Legal notice",
        "desc": "Information about the publisher of loonar.eu and Loonar Compass.",
        "body": """
<p><strong>{firm}</strong><br>Sole proprietorship of {person}<br>{street}<br>{city}<br>Poland</p>
<p>E-mail: <a href="mailto:{email}">{email}</a><br>NIP (tax ID): {nip}<br>REGON: {regon}<br>Registered in the Central Register and Information on Economic Activity (CEIDG), kept by the Polish minister responsible for the economy (biznes.gov.pl).</p>
<p>Responsible for the content: {person}.</p>
""",
    },
    "pl": {
        "title": "Nota prawna",
        "desc": "Informacje o wydawcy loonar.eu i Loonar Compass.",
        "body": """
<p><strong>{firm}</strong><br>jednoosobowa działalność gospodarcza – {person}<br>{street}<br>{city}</p>
<p>E-mail: <a href="mailto:{email}">{email}</a><br>NIP: {nip}<br>REGON: {regon}<br>Wpis do Centralnej Ewidencji i Informacji o Działalności Gospodarczej (CEIDG), prowadzonej przez ministra właściwego do spraw gospodarki (biznes.gov.pl).</p>
<p>Odpowiedzialny za treść: {person}.</p>
""",
    },
    "de": {
        "title": "Impressum",
        "desc": "Angaben zum Anbieter von loonar.eu und Loonar Compass.",
        "body": """
<p><strong>{firm}</strong><br>Einzelunternehmen, Inhaber: {person}<br>{street}<br>{city}<br>Polen</p>
<p>E-Mail: <a href="mailto:{email}">{email}</a><br>NIP (Steuernummer): {nip}<br>REGON: {regon}<br>Eingetragen im polnischen Zentralregister und Informationssystem über die Gewerbetätigkeit (CEIDG), geführt vom zuständigen polnischen Wirtschaftsminister (biznes.gov.pl).</p>
<p>Inhaltlich verantwortlich: {person}.</p>
""",
    },
    "fr": {
        "title": "Mentions légales",
        "desc": "Informations sur l’éditeur de loonar.eu et de Loonar Compass.",
        "body": """
<p><strong>{firm}</strong><br>Entreprise individuelle, titulaire : {person}<br>{street}<br>{city}<br>Pologne</p>
<p>E-mail : <a href="mailto:{email}">{email}</a><br>NIP (numéro fiscal) : {nip}<br>REGON : {regon}<br>Inscrite au registre central polonais des activités économiques (CEIDG), tenu par le ministre polonais chargé de l’économie (biznes.gov.pl).</p>
<p>Directeur de la publication : {person}.</p>
<p>Hébergeur : GitHub, Inc. (GitHub Pages), 88 Colin P. Kelly Jr. Street, San Francisco, CA 94107, États-Unis.</p>
""",
    },
}


def page(kind, lang, **extra):
    """Returns (title, description, HTML body, date of the last update) of the privacy policy or the legal notice.
    `extra` fills the placeholders that build.py provides (links, the opt-out switch)."""
    src = (PRIVACY if kind == "privacy" else LEGAL)[lang]
    return src["title"], src["desc"], src["body"].format(**OWNER, **extra).strip(), UPDATED[lang]
