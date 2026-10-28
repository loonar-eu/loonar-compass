# Changelog

Wszystkie istotne zmiany w aplikacji Loonar Compass (do wersji 0.3 pod nazwą Loonar EA Assistant).

## Wersja 0.9.2 (w przygotowaniu)

- **Widoczne strzałki na diagramach:** połączenia, które nie są komunikatami diagramu sekwencji, nie są już przypinane do jednego diagramu (wcześniej np. zależność narysowana na diagramie „Systemy zależne” nie pokazywała się na żadnym innym). Pokazanie ukrytego połączenia usuwa jego zdublowane wpisy na diagramie, a nowa naprawa „połączenia niewidoczne na diagramie” odpina przypięte połączenia, usuwa duplikaty i – na życzenie – pokazuje ukryte. Asystent przy brakujących strzałkach sprawdza najpierw te przyczyny.
- **Załączniki w Jirze:** asystent dołącza do zgłoszeń obrazy diagramów prosto z modelu oraz obrazy i pliki z rozmowy – nowym narzędziem albo od razu przy zakładaniu i zmianie zgłoszenia. W Jira Server / Data Center obrazy pokazują się też w opisie zgłoszenia.
- **Ostre obrazy diagramów w Jirze i Confluence:** diagramy dołączane do zgłoszeń, stron i dokumentów są eksportowane w 2× większej rozdzielczości, rysowane z wektorów (metaplik EA), więc po powiększeniu nie są rozmyte. W opisie zgłoszenia Jira Server obraz ma pełny rozmiar zamiast miniatury, a szerokość można podać (np. 900 px).

## Wersja 0.9.1 (2026-10-28)

- **Jira i Confluence z certyfikatem klienta:** serwery firmowe, które oprócz tokenu wymagają certyfikatu klienta (mTLS), działają z Compassem. W Ustawieniach na kartach Jira i Confluence są pola „Certyfikat klienta” (ścieżka do pliku .p12/.pfx albo PEM) i „Hasło certyfikatu” (przechowywane w Menedżerze poświadczeń Windows). Bez certyfikatu taki serwer zamykał połączenie („Istniejące połączenie zostało gwałtownie zamknięte przez zdalnego hosta”).
- **Prostszy instalator:** bez ekranu „Konfiguracja” – klucz, model i konto EA podajesz w aplikacji, która przy pierwszym starcie przeprowadzi Cię przez ustawienia.
- **Ponowienie instalacji:** gdy instalacja nie może podmienić plików, bo Compass jest uruchomiony, ekran błędu ma przyciski „Zamknij Compass i spróbuj ponownie” oraz „Spróbuj ponownie” – bez zaczynania od nowa.
- **Adres Jira i Confluence wklejony z przeglądarki:** końcówka strony (np. „/spaces/…”, „/display/…”, „/browse/…”) jest obcinana, a gdy serwer zamiast danych odsyła stronę WWW, Compass mówi, że trzeba sprawdzić adres (wcześniej sprawdzenie pokazywało „Połączono”, a strony były puste).
- **Asystent czyta kod źródłowy:** nowe narzędzia tylko do odczytu – lista plików, wyszukiwanie w kodzie (plik:linia) i odczyt pliku – w lokalnych kopiach repozytoriów. Foldery wpisane w Ustawienia → „Kod źródłowy” (karta widoczna także w EA; przycisk „…” dodaje folder) czyta bez pytania, o inne najpierw pyta. Dzięki temu sam ustali, gdzie jest zaimplementowana klasa, operacja czy tabela i co od niej zależy.
- **OneDrive i Dysk Google:** asystent listuje foldery, czyta dokumenty (Word, Excel, PowerPoint, PDF, Visio, tekst; na Dysku Google także Dokumenty, Arkusze i Prezentacje Google) i – za każdym razem za Twoją zgodą – wysyła przygotowane przez siebie dokumenty. Logujesz się własnym kontem Microsoft albo Google w przeglądarce jeden raz (OAuth z PKCE), token odświeżania jest w Menedżerze poświadczeń Windows. Ustawienia: karty „OneDrive” (rejestracja aplikacji w Entra ID jako klient publiczny) i „Google Drive” (klient OAuth „Aplikacja komputerowa” w Google Cloud).
- **Nowa wersja i statystyki co godzinę:** Compass sprawdza nową wersję co godzinę (także gdy jest długo otwarty), a statystyki – jeśli są włączone – wysyła przy starcie (identyfikator instalacji, wersja, system, język) i co godzinę (podsumowanie dnia: rodzaje zadań i ich wynik, oceny odpowiedzi, czego asystent nie umiał). Pod odpowiedziami: 👍/👎 i – gdy asystent czegoś nie umie – „💡 Zgłoś zapotrzebowanie”.
- **Rozmowa w innym modelu, eksport i import:** po przeniesieniu projektu (np. z bazy do pliku) rozmowę da się kontynuować z nowym modelem. W zakładce EA: „⋯” → „Rozmowy z innych modeli…” (także link pod listą rozmów) → „Kontynuuj tutaj”. Powstaje kopia, oryginał zostaje przy swoim modelu, a notatki pamięci poprzedniego modelu można przypiąć do nowego. Asystent dostaje informację, że wcześniejsze wiadomości dotyczyły innego modelu, i najpierw sprawdza wspomniane elementy. W samodzielnym oknie model rozmowy zmienia się jak dotąd przyciskiem „Model: …”. Rozmowę z załącznikami można wyeksportować do pliku `.lcchat` (menu „⋯” w EA, prawy przycisk na rozmowie w oknie) i zaimportować na innym komputerze – trafia do modelu otwartego w danym oknie. Plik zawiera ścieżkę pliku albo nazwę bazy i serwera, bez kont i haseł.
- **Asystent pamięta, co zrobił:** kroki każdej odpowiedzi (odczyty, wyszukiwania, zapisy w EA, strony Confluence) zostają w rozmowie i asystent dostaje je w kolejnych turach. Wcześniej historia miała tylko tekst odpowiedzi, więc zapytany później potrafił zaprzeczyć, że np. czytał Confluence, choć to zrobił.
- **Asystent pracuje samodzielnie do końca:** robi całe zadanie w jednej odpowiedzi, zamiast co kilka kroków pokazywać stan i pytać „czy kontynuować?”. Rozsądne wybory (nazwy, pliki, format, kolejność) podejmuje sam i wymienia je w odpowiedzi; pyta tylko wtedy, gdy zła decyzja zmieniłaby dużo w modelu. Notatki „teraz czytam…” są w krokach pracy, nie jako osobne wiadomości w czacie. Limit to 40 kroków, a po jego osiągnięciu asystent mówi, co zrobił i co zostało, zamiast kończyć błędem. Gdy mimo to zatrzyma się przed końcem pracy (otwarte zadania w planie albo „napisz dalej”), Compass sam każe mu iść dalej, do 3 razy w jednej odpowiedzi.
- **Mniej pytań o skrypty EA:** po pierwszej zgodzie kolejne skrypty tej samej budowy – ten sam kod z innymi danymi, np. następna porcja aliasów – zapisują się i uruchamiają bez pytania (do zamknięcia Compassa); inny kod pyta jak dotąd. Alias i stereotyp operacji oraz atrybutu asystent ustawia teraz wprost przy zapisie, więc do aliasów (np. ścieżek REST) skrypt nie jest potrzebny. W karcie zgody na skrypt jest też przycisk „Zatwierdź wszystkie skrypty do zamknięcia Compassa”, a zmiany, które da się zrobić zwykłym zapisem, asystent robi bez skryptów.
- **Jedna zgoda na serię:** w karcie zgody na zgłoszenie Jira, publikację w Confluence, notatkę pamięci, wywołanie narzędzia MCP, wysyłkę pliku i skrypt EA jest przycisk „Zatwierdź wszystkie … do zamknięcia Compassa”. Dziesięć zgłoszeń Jira to jedno kliknięcie zamiast dziesięciu.
- **Story i UC pod epikiem w Jira Server / Data Center:** zgłoszenie z rodzicem-epikiem jest dowiązywane przez pole „Epic Link” (pole `parent` jest tam tylko dla pod-zadań). Wcześniej Jira odrzucała je błędem „Issue type … is not a sub-task but a parent is specified”. Jira Cloud działa jak dotąd.
- **Zmiana zgłoszeń Jira:** asystent poprawia istniejące zgłoszenia – tytuł, opis i etykiety (dodaje i usuwa), np. usuwa z kryteriów akceptacji niepotrzebne oznaczenie we wszystkich zgłoszeniach naraz. Zgoda jak przy zakładaniu, także jedną na wszystkie.
- **Niższe koszty modelu AI:** cała rozmowa w pętli asystenta – historia i wyniki narzędzi, np. przeczytane pliki kodu – jest teraz w cache Anthropic, więc każdy kolejny krok płaci za nią dziesiątą część ceny. Lista zmian nie jest już wklejana do każdego zapytania (ok. 18 tys. tokenów mniej); asystent czyta ją narzędziem, gdy pytasz o funkcje Compassa. W teście analizy kodu koszt zadania spadł o ok. 79%. Pod każdą odpowiedzią krok „💲 Koszt tej odpowiedzi” pokazuje liczbę wywołań, tokeny, udział cache i koszt w USD; każde wywołanie trafia też do lokalnego dziennika `usage` (same liczby, bez treści).
- **Bardzo długie rozmowy taniej:** zanim zapytanie zbliży się do 100 tys. tokenów (powyżej Claude Haiku 5.5 liczy całe zapytanie 5× drożej), starsza część rozmowy zamienia się w streszczenie zapisane w rozmowie – w czacie widać linię „Asystent widzi rozmowę powyżej jako streszczenie”, ostatnie 8 wiadomości zostaje słowo w słowo. W bardzo długiej pracy na narzędziach (np. dziesiątki przeczytanych plików) Anthropic sam usuwa z widoku modelu najstarsze wyniki narzędzi powyżej 85 tys. tokenów; 6 najnowszych zostaje, a asystent może przeczytać plik ponownie.
- **Stała część zapytania w cache przez godzinę:** narzędzia i instrukcje asystenta (~35 tys. tokenów) są trzymane w cache Anthropic 1 h zamiast 5 min, więc po przerwie między pytaniami nie są zapisywane od nowa (zapis kosztuje tyle, co kilkanaście odczytów). Rozmowa – zapisywana w każdym kroku – zostaje przy 5 min.

## Wersja 0.9 (2026-10-27)

Logowanie kontem firmowym do huba, modele w SQL Server, MySQL i Oracle, instrukcje modelowania zespołu w repozytorium, dokumenty PDF, PowerPoint, Visio i Excel, prezentacje, SharePoint, strażnicy zatrzymujący usuwanie oraz uważniejszy asystent: pytania, plan, podagenci i ochrona przed ukrytymi instrukcjami.

### Compass Hub

- **Logowanie kontem firmowym (OpenID Connect):** AD FS, Microsoft Entra ID, Keycloak, Okta i inni dostawcy, ustawiani w sekcji `"oidc"` pliku `hub.json`. Widok webowy pokazuje „Sign in with your company account”.
  - Authorization code z PKCE; hub sam sprawdza token ID (podpis RS256 kluczami dostawcy, wystawca, odbiorca, ważność, nonce).
  - Role z grup (`roles`, domyślnie Compass-Admins, Compass-Architects, Compass-Viewers): wygrywa najwyższa rola, brak grupy to brak dostępu. Lokalny użytkownik o tej samej nazwie jest odrzucany.
  - Klucz tajny klienta to sekret huba `OIDC_CLIENT_SECRET`. `"password_login": false` zostawia tylko logowanie firmowe; tokeny API działają dalej. Każde logowanie trafia do dziennika zmian.
- **Hub za IIS/ARR i audyt logowań:** `allatclaims` dodawany sam dla AD FS; blokada po błędnych hasłach według adresu klienta z `X-Forwarded-For` (tylko od proxy na tym samym komputerze); nieudane i odrzucone logowania w „Audit trail” z ✗; w „Users” lista kont firmowych z rolą i ostatnim logowaniem (tylko odczyt); usługa Windows zatrzymuje się i restartuje poprawnie. Instrukcja: ustawienie ARR `reverseRewriteHostInResponseHeaders="false"`.
- **Agenci zdarzeń dla zespołu:** klucz polityki `"event_agents"` (ten sam format co `event_agents.json`, także strażnicy). Agenci działają w Compassie w EA u każdego, z dopiskiem „(team)”.
- **Zakładka „Team policy” (admin):** edytor JSON polityki (min_approval, governance, event_agents, ask_min_role, ask_per_hour).
- **Raporty Word i Excel:** „Word report of the package” i „Excel report of the package” w zakładce Model check: opis, elementy, relacje (bez obrazków diagramów). Każde pobranie trafia do dziennika.
- **GraphQL i Power BI:** `POST /api/sources/{id}/graphql` tylko do odczytu (pakiety, elementy z atrybutami, operacjami, wartościami tagów i relacjami, diagramy; schemat pod `GET /api/graphql/schema`) oraz płaskie tabele `GET /api/sources/{id}/table/{elements|connectors|packages|diagrams|attributes|tagged_values}` dla Power BI (Pobierz dane → Sieć Web → Zaawansowane, nagłówek `Authorization: Bearer <token API>`). Każda rola.

### Modele w SQL Server, MySQL i Oracle

- **Nowe repozytoria:** oprócz PostgreSQL i plików .qea Compass czyta repozytoria EA w SQL Server, MySQL/MariaDB i Oracle: w osobnym oknie, w hubie i w trybie sprawdzania. Ustawienia → Model mają wybór SQL Server, MySQL i Oracle.
- **Sterownik ODBC:** 64-bitowy sterownik ODBC producenta jest wyszukiwany automatycznie (SQL Server: ODBC Driver 18 albo 17 albo własny sterownik Windows „SQL Server”). Pełny ciąg połączenia bez hasła można podać w `db.odbc` (na przykład dla Oracle).
- **Tylko odczyt:** każdy odczyt jest wycofywany.
- **Zapis przez EA** (edycja EA musi obsługiwać repozytoria DBMS):
  - SQL Server: Compass sam buduje połączenie EA (dostawca OLE DB SQLOLEDB), tak jak dla PostgreSQL.
  - MySQL i Oracle: połączenie EA pochodzi z nowego ustawienia `ea.conn` w app.properties, czyli tekstu skrótu połączenia EA przez źródło danych ODBC (np. `ea_lab --- DBType=0;Connect=Provider=MSDASQL.1;Data Source=<DSN ODBC>;…`). Bez niego zmiany idą przez Compassa w EA, który jak dotąd działa z każdym repozytorium otwartym w EA.
- **Docker:** obraz huba ma unixODBC; linuksowy sterownik ODBC producenta dodaj w obrazie zbudowanym na nim.

### Instrukcje modelowania w repozytorium

- **Zasady zespołu w EA:** Artifact «CompassInstructions» („Loonar Compass – modelling instructions”), którego notatki są zasadami repozytorium (nazewnictwo, typy i stereotypy, pakiety, diagramy, wartości tagów, kontrole). Asystent stosuje je w każdej odpowiedzi, u każdego, kto otwiera repozytorium.
- **Kreator na start:** asystent analizuje repozytorium i szkicuje zasady; zapis i dopisanie zasady to zmiany modelu pokazywane do zatwierdzenia.
  - Przykład: „Przygotuj instrukcje modelowania dla tego repozytorium”.
  - Przykład: „Dopisz do instrukcji: aktywności zaczynają się od czasownika”.

### Rozmowa

- **Pytania doprecyzowujące jako formularz:** gdy coś ważnego jest niejasne, asystent pokazuje „Pytania do Ciebie” z wyborem albo krótką odpowiedzią („Coś innego…”); „Wyślij odpowiedzi” wysyła je jako kolejną wiadomość („Moje odpowiedzi:”).
- **Plan większej pracy:** „📋 Plan: 1 z 4 zrobione” ze znakami ✓ ▶ ○ w krokach (i w System Output EA), aktualizowany w trakcie pracy.
- **Podagenci:** przy większych analizach asystent przekazuje zadanie podagentowi z rolą impact, review albo document, który czyta model we własnym kontekście i zwraca zwięzły wynik („Podagent (impact): …”).
- **Wybierz z modelu:** przycisk ⌖ obok Wklej wyszukuje elementy, pakiety i diagramy po nazwie (puste = główne pakiety) i dodaje je do wiadomości jako odwołania, na przykład «Element: Zamówienie (Class, id 26)».
- **Rozumowanie asystenta:** przycisk 💭 obok Wklej pokazuje streszczenie rozumowania jako krok „💭 Rozumowanie asystenta” (tylko Claude; poziom wysiłku zostaje w Ustawieniach).
- **Kod w piaskownicy Anthropic:** przycisk ∑ obok Wklej (domyślnie wyłączony; tylko Claude) pozwala asystentowi uruchamiać kod do obliczeń i analizy danych; wynik to krok „Kod w piaskownicy”. Przekazane tam dane trafiają do Anthropic, tak jak rozmowa.
- **Internet (🌐):** przycisk obok ∑ (domyślnie wyłączony). Z Claude wyszukiwanie i czytanie stron przez Anthropic; z OpenAI Compass czyta tylko strony z adresów podanych w rozmowie (bez adresów tego komputera i sieci firmowej). Dozwolone domeny: `claude.web_domains`. Polityka huba: `"web_access": false` wyłącza wszystkim, `"web_allowed_domains"` ogranicza domeny. Wyszukiwania i strony widoczne w krokach odpowiedzi.
- **Informacja o nowej wersji:** raz dziennie Compass sprawdza na GitHubie najnowsze wydanie i pokazuje pasek „Co nowego i pobieranie / Nie teraz” (w EA: wpis w System Output). Nic nie pobiera ani nie instaluje sam, bez identyfikatora instalacji. Wyłączanie: Ustawienia → Nowe wydania (newsletter) albo `app.update_check=false`; polityka huba `"update_check": false`.
- **Zasady komputera (GPO, Intune) i logowanie kontem firmowym:** klucz `HKLM/HKCU\SOFTWARE\Policies\Loonar\Compass` – `HubUrl` (adres huba zablokowany), `RequireHub` (asystent tylko z hubem i jego polityką; przy awarii huba ostatnia polityka do tygodnia), `WebAccess`, `UpdateCheck`, `MinApproval`, `AllowedProviders`. Szablony ADMX/ADML w `deploy\policies`. „Zaloguj się kontem firmowym” w Ustawienia → Zespół: logowanie huba w przeglądarce, sesja wraca na 127.0.0.1 i trafia do Menedżera poświadczeń.
- **Klucze AI, Jira i Confluence na hubie:** bramka `/ai/v1/messages` – klucze Claude trzyma hub, przypisane do osoby (nazwa albo UPN z AD), roli albo wszystkich, z limitami tokenów na dzień i miesiąc oraz listą modeli; zużycie per osoba w zakładce Users. Jira i Confluence przez hub kontem zespołu (viewer tylko czyta, zmiany w audycie). Polityka: `ai_via_hub`, `jira_via_hub`, `confluence_via_hub`.
- **Strażnik bez decyzji blokuje:** gdy strażnik nie zdecyduje w 40 s albo nie może (brak AI, hub niedostępny), usunięcie jest wstrzymywane z komunikatem (wcześniej przechodziło).
- **Cicha instalacja:** `LoonarCompass-Setup --quiet [--all-users] [--dir …] [--lang …] [--no-desktop] [--no-start-menu] [--no-ea-addin]` – wdrożenie przez Intune, Configuration Manager lub skrypt; dla wszystkich użytkowników dodatek w EA działa u każdego.
- **Strażnik usunięć w EA:** EA nie zamarza już na czas decyzji, a strażnik naprawdę decyduje (wcześniej po 40 s usunięcie przechodziło bez decyzji).
- **Czytanie stron Confluence:** asystent czyta stronę po tytule lub id, także przez hub.
- **Instrukcja instalacji huba dla działu IT** (PL i EN).
- **Compass w EA od startu:** okno Compass jest widoczne od uruchomienia EA; bez projektu pokazuje „Brak otwartego projektu…”, po otwarciu pliku lub repozytorium Compass włącza się sam, a po zamknięciu projektu wraca do tego komunikatu.
- **Hub po restarcie:** sesje zalogowanych osób przetrwają restart huba; ten sam nieważny token liczy się do blokady adresu raz; Compass przy wygasłym logowaniu do huba od razu prosi o ponowne zalogowanie, a błędy huba opisuje krótko (bez stron HTML z IIS).
- **Słownik projektu EA:** domyślny typ nowego pojęcia to `Glossary` (wcześniej `Slownik`).
- **Długie rozmowy:** powyżej około 100 000 tokenów wcześniejsza część jest na potrzeby kolejnych odpowiedzi zastępowana streszczeniem („Rozmowa jest długa: streszczam jej wcześniejszą część”); najnowsze wiadomości zostają.
- **Usuwanie rozmowy:** prawy przycisk myszy na rozmowie → „Usuń rozmowę” (w Compass w EA: przycisk 🗑 obok nazwy rozmowy). Rozmowa trafia do `history\bin` i można ją przywrócić; nie w trakcie pracy asystenta w niej.

### Dokumenty, prezentacje i SharePoint

- **Czytanie dokumentów:** narzędzie `read_document` i załączniki czytają PDF (każda strona; skan bez warstwy tekstu dołącz, a model przeczyta obrazy stron), PowerPoint .pptx (slajdy z notatkami), Visio .vsdx (strony, kształty z tekstem, połączenia A → B), Excel .xlsx (arkusze, komórki) i Word. Stare pliki .doc, .ppt, .xls i .vsd trzeba zapisać w nowym formacie.
  - Wymagania dla `trace_requirements` i `--requirements` mogą pochodzić z tych formatów.
- **Prezentacje PowerPoint:** `create_presentation` tworzy prezentację 16:9: slajd tytułowy, slajdy z punktami na dwóch poziomach i/lub diagramem albo obrazkiem, opcjonalnie także jako PDF (przez Microsoft PowerPoint).
- **Dokumenty PDF:** `create_document` z formatem pdf albo both (przez Microsoft Word). PDF wymaga Microsoft Office na komputerze.
- **SharePoint:** nowa karta Ustawienia → SharePoint (adres witryny, biblioteka dokumentów, identyfikator dzierżawy, identyfikator aplikacji (klienta), klucz tajny klienta; „Zapisz i sprawdź połączenie z SharePoint”). Wymaga rejestracji aplikacji w Entra ID z uprawnieniem aplikacji Microsoft Graph Sites.Selected (nadanym dla witryny) albo Sites.ReadWrite.All.
  - Asystent pokazuje zawartość folderu, czyta tekst dokumentu i wysyła przygotowany przez siebie plik; każde wysłanie wymaga zgody.

### Enterprise Architect

- **Agenci zdarzeń dla 18 zdarzeń:** utworzenie, zmiana i usuwanie elementów, połączeń, diagramów i pakietów; utworzenie i usuwanie atrybutów, metod i pojęć słownika.
- **Strażnicy:** `"mode": "guard"` przy zdarzeniu `*_deleting`. EA pyta Compass przed usunięciem; strażnik odpowiada ALLOW albo BLOCK z przyczyną, a EA pokazuje „Loonar Compass zatrzymał to usunięcie:” i anuluje usunięcie. Bez odpowiedzi w ciągu 40 s usunięcie zostaje wykonane.
  - Przykład: `{"name": "Zatwierdzone zostają", "event": "element_deleting", "mode": "guard", "applies_to": "Requirement", "prompt": "Zablokuj usunięcie wymagania, którego status to Approved."}`.
- **Technologie MDG:** narzędzie `mdg_technologies`: profile używane w modelu (z liczbą użyć), pliki technologii EA i Twoje, technologie zaimportowane do modelu, stereotypy z metatypem i tym, do czego się stosują, oraz ich wartości tagów z typem, wartościami i wartością domyślną.
- **API EA:** narzędzie `ea_api` czyta interfejs automatyzacji z EA.tlb zainstalowanego EA (klasy, właściwości do odczytu albo odczytu i zapisu, metody z parametrami, wyliczenia); asystent korzysta z niego przed pisaniem skryptów EA.
- **Kontrola wersji pakietów:** narzędzie `version_control_status` (pakiety pod kontrolą wersji, konfiguracja, plik, kto je ma pobrane do edycji) i operacja modelu `version_control` (check_out, check_in, get_latest, put_latest, resync, z komentarzem). Zawsze z pytaniem; pakiet musi już być pod kontrolą wersji w EA.

### Bezpieczeństwo i logowanie

- **Ochrona przed ukrytymi instrukcjami (prompt injection):** zasady bezpieczeństwa w każdym zapytaniu; wyniki z dokumentów, Jiry, Confluence, MCP, CMDB i kodu są oznaczone jako dane; odpowiedź, która powtarza słowo w słowo wewnętrzne instrukcje, jest wstrzymywana.
- **Serwery MCP przez HTTP:** `"auth": {"type": "basic", "user", "password"}` albo `{"type": "oauth2", "token_url", "client_id", "client_secret", "scope"}` w `mcp_servers.json` (sekrety jako `${secret:mcp_…}`).
- **Usługa AI z OAuth2:** nowe pola w karcie Model AI: „Adres tokenu OAuth2” (opcjonalnie, zamiast klucza API), a potem „Identyfikator klienta OAuth2”, „Zakres OAuth2 (scope)” i „Klucz tajny klienta OAuth2” (w Menedżerze poświadczeń). Na przykład Entra ID z zakresem `https://cognitiveservices.azure.com/.default`. Zapisywane jako `ai.oauth.*`.
- **Claude pod innym adresem:** pole „Adres Claude” (tylko Claude; puste = Anthropic), na przykład Azure AI Foundry `https://<zasób>.services.ai.azure.com/anthropic` albo firmowa bramka. Zapisywane jako `claude.url`.

## Wersja 0.8.2 (2026-10-23)

Compass Hub jako osobny program (Windows, Linux, Docker) z kontami użytkowników, kontrola modelu w potokach CI, reguły ArchiMate, pełny łańcuch śledzenia, porównanie z CMDB i gotowe serwery MCP.

### Compass Hub – osobny program

- **Osobny `compass-hub`:** mały program bez okna i bez EA, dla Windows i Linuksa. Uruchomienie: `compass-hub hub.json`. Dotychczasowe `LoonarCompass.exe --hub` też działa.
- **Usługa Windows:** `compass-hub --service-install C:\Hub\hub.json` (jako administrator) – hub startuje z systemem; `--service-uninstall` usuwa usługę.
- **Docker na Linuksie:** obraz `compass-hub` (plik `hub/Dockerfile`), dane i `hub.json` w wolumenie `/data`, działa jako zwykły użytkownik.
  - Przykład: `docker run -d -p 8730:8730 -v /srv/compass-hub:/data -e LOONAR_SECRET_CLAUDE_API_KEY=… compass-hub`.
- **Sekrety huba:** ze zmiennych środowiska `LOONAR_SECRET_<NAZWA>` albo plików `LOONAR_SECRET_<NAZWA>_FILE` (np. `CLAUDE_API_KEY`, `JIRA_TOKEN`, `CONFLUENCE_TOKEN`, `DB_PASSWORD`).

### Konta i logowanie w hubie

- **Logowanie:** nowy ekran logowania nazwą użytkownika i hasłem; hasła tylko jako skróty PBKDF2, sesja 12 godzin. Logowanie tokenem nadal jest możliwe.
- **Konto domyślne:** nowy hub bez użytkowników ma konto **admin / passw0rd** (z zerem). Przy pierwszym logowaniu trzeba zmienić hasło – wcześniej konto nie ma dostępu do niczego innego.
- **Zakładka „Users” (admin):**
  - dodanie użytkownika (z pierwszym hasłem do zmiany);
  - zmiana roli i reset hasła;
  - nowy token API dla Compassa i klientów MCP, pokazany raz;
  - usunięcie użytkownika (zawsze zostaje co najmniej jeden admin).
- **Blokada po błędach:** po 20 błędnych hasłach z jednego adresu hub odmawia na 10 minut.

### Jira i Confluence z huba

- **Konto techniczne:** administrator wpisuje w `hub.json` sekcję `"integrations"` (Jira: adres, projekt; Confluence: adres, przestrzeń), a tokeny podaje jako sekrety huba.
- **Przyciski dla architektów w widoku webowym:**
  - „Create Jira issue” przy elemencie i przy zestawie zmian;
  - „Publish this package to Confluence” przy kontroli pakietu – strona tekstowa z elementami, opisami i relacjami, bez obrazków diagramów, bo hub nie ma EA.
- **Dziennik i Ask:** każda akcja trafia do dziennika zmian. „Ask” nadal tylko czyta model.

### Kontrola modelu w potokach CI

- **Tryb CI:** `LoonarCompass.exe --check --model model.qea [--code src] [--requirements BRD.docx] [--cmdb cmdb.csv] [--report raport.md] [--junit junit.xml] [--fail-on error|warning]`.
- **Wynik:** raport Markdown, plik JUnit dla GitLaba, GitHub Actions i Azure DevOps, kod wyjścia 0 (OK), 1 (ustalenia) albo 2 (błąd). Bez AI i bez EA.

### Model, wymagania, kod i CMDB

- **Reguły ArchiMate 3.2:** kontrola modelu sprawdza relacje elementów ArchiMate (np. access tylko do struktury pasywnej, influence tylko do motywacji). Wyłączenie: `"archimate": false` w governance.json.
- **Łańcuch śledzenia:** `trace_requirements` z folderem kodu sprawdza drogę wymaganie → model → kod → test → zgłoszenie i pokazuje, gdzie się urywa.
  - Przykład: „Porównaj wymagania z C:\Projekty\BRD.docx z modelem i kodem w C:\repo\sklep”.
- **CMDB a model:** eksport CSV/JSON (np. z ServiceNow) albo dane pobrane przez serwer MCP porównane z komponentami modelu: czego brakuje w modelu, czego nie ma w CMDB, inny właściciel lub status.
  - Przykład: „Porównaj eksport CMDB C:\Eksport\cmdb_ci_appl.csv z komponentami modelu”.

### Serwery MCP

- **Gotowe szablony:** Azure DevOps, GitHub, ServiceNow / CMDB, pliki lokalne i Compass Hub w karcie „Serwery MCP” – wybierasz z listy i uzupełniasz organizację lub adres.
- **Token serwera MCP:** zapisywany z karty w Menedżerze poświadczeń (nazwy zaczynają się od `mcp_`).
- **Programy Node.js:** `npx` w konfiguracji działa na Windows (uruchamiane jako `npx.cmd`).

## Wersja 0.8.1 (2026-10-23)

Wydanie poprawek bezpieczeństwa po audycie całej aplikacji. Zalecane dla wszystkich, a szczególnie dla zespołów z Compass Hub. Kilka czynności pyta teraz o zgodę częściej niż wcześniej – to celowe.

### Compass Hub

- **Odporność:** źle zbudowane żądanie nie zatrzymuje już huba; żądania mają limity rozmiaru, liczby nagłówków i czasu, a token jest sprawdzany przed odczytem treści.
- **Tokeny:** w hub.json zamiast "token" można podać "token_sha256" (skrót tokenu); `LoonarCompass.exe --hub-token` tworzy nowy losowy token i jego skrót. Nazwy i tokeny muszą być unikalne. Po 20 błędnych tokenach adres jest blokowany na 10 minut (poprawny token działa dalej).
- **Zestawy zmian:** opis i podgląd, które widzi recenzent, hub liczy sam z operacji, a widok webowy pokazuje operacje. Nikt, także admin, nie zatwierdza własnego zestawu. Wprowadzenie ma dwa kroki (przejęcie przez jeden Compass, potem wynik), więc zestawu nie da się wprowadzić dwa razy. Admin może zwolnić zestaw, który utknął.
- **Ask:** asystent w widoku webowym tylko czyta model (bez plików, Jira, Confluence i serwerów MCP), odpowiada na jedno pytanie naraz, z limitem czasu. Nowe ustawienia polityki: "ask_min_role" (kto może pytać) i "ask_per_hour" (domyślnie 20 pytań na osobę na godzinę).
- **Widok webowy:** nagłówki Content-Security-Policy i ochrona przed osadzaniem w ramce. Token jest pamiętany tylko na czas sesji przeglądarki; jest przycisk „Sign out”. Błędy wewnętrzne są pokazywane jako identyfikator, a szczegóły trafiają do hub.log.
- **Dziennik zmian:** przyjmuje tylko znane pola z czasem huba. Dane stanowisk widzi tylko admin.

### Zgody i zapisy w modelu

- **Pliki:** eksport XMI, dokumenty z szablonów EA i import XMI zawsze pytają o zgodę (także w trybie auto) i pokazują pełną ścieżkę. Dozwolone są tylko pełne ścieżki na dysku lokalnym, bez udziałów sieciowych i folderów systemowych.
- **Usuwanie:** id podane jako tekst albo ułamek nie omija już pytania przy usuwaniu elementu z relacjami. Usunięcie pakietu zawsze pyta.
- **Pamięć, szablony i skrypty:** zapis notatki w pamięci, zapis szablonu dokumentu oraz zapis i uruchomienie skryptu EA zawsze pytają, z całym tekstem lub kodem. Skrypt uruchamia się dokładnie w pokazanej postaci, razem z dołączeniami.
- **Odczyt plików:** zanim asystent przeczyta plik spoza załączników rozmowy (dokument wymagań, folder z kodem), pyta o zgodę.
- **Zapytania SQL:** asystent może tylko czytać – zapytania ze słowami zapisu (INSERT, UPDATE, DELETE, INTO…) i nietypowym cytowaniem są odrzucane, także przy odczycie przez okno EA.
- **Agenci zdarzeń:** tylko czytają model. Nazwy obiektów trafiają do polecenia jako dane, nie jako instrukcje.
- **Cofanie:** dziennik cofania jest sprawdzany przed wykonaniem, a zgoda pokazuje, co zostanie usunięte.

### Serwery MCP i połączenia

- **Zgoda MCP:** każde wywołanie narzędzia MCP pyta o zgodę, chyba że serwer ma "trusted": true. Sama deklaracja serwera „tylko odczyt” już nie wystarcza.
- **Sekrety:** klucze i hasła samego Compassa nie mogą trafić do serwera MCP przez ${secret:…}. Wyjątek: ${secret:hub_token} dla punktu /mcp huba z ustawień. Program lokalny MCP dostaje tylko podstawowe zmienne środowiska oraz te z "env".
- **Przekierowania i adresy:** przekierowania HTTP tylko w obrębie tego samego serwera (klucze API nie trafiają gdzie indziej). Adres http:// jest dozwolony tylko dla tego komputera, bez nazwy użytkownika i hasła w adresie.

### Pozostałe

- **Edytor diagramów:** nazwy z modelu są wyświetlane jako tekst (bez HTML), strona ma politykę CSP, a usuwać i zmieniać można tylko obiekty otwartego diagramu.
- **Dodatek EA:** log nie zawiera connection stringu. Compass startuje we własnym folderze. Bibliotek DLL nie szuka się w bieżącym folderze.
- **Okna błędów EA:** zamykane są tylko te, które pojawiły się w trakcie zapytania Compassa.
- **SQLite 3.50.2:** poprawki znanych podatności; pliki modelu są otwierane w trybie defensywnym.
- **Rekomendacja dla firm:** osobna rola PostgreSQL z minimalnymi uprawnieniami (SELECT, a dla zapisów przez EA także INSERT, UPDATE i DELETE na tabelach t_*; nigdy superuser) oraz szyfrowanie połączenia z weryfikacją certyfikatu.

## Wersja 0.8 (2026-10-23)

Największe wydanie od początku. Compass pilnuje jakości modelu, sprawdza każdy zapis i pozwala go cofnąć, porównuje model z wymaganiami i z kodem, wycenia zmiany i działa w zespole. Przy każdej funkcji jest przykładowe polecenie: wpisz je w rozmowie tak, jak jest, albo własnymi słowami.

### Model AI do wyboru

- **Dostawcy i modele.** Anthropic (Claude) albo OpenAI, dowolny model (z listy albo własny identyfikator) i poziom wysiłku, od „domyślny dla modelu” do max. Gdzie: Ustawienia → karta „Model AI”.
  - Domyślnie bez zmian: Claude Haiku 5.5.
  - Każdy dostawca ma własny klucz w Menedżerze poświadczeń Windows; dane trafiają tylko do wybranego.
- **Dowolny endpoint zgodny z API OpenAI.** Azure OpenAI, Ollama, LM Studio albo firmowa bramka. Z modelem lokalnym nic nie opuszcza Twojego komputera.

### Jakość modelu i reguły zespołu

- **Kontrola modelu.** Według reguł, bez zgadywania przez AI. Każde ustalenie ma wagę: błąd, ostrzeżenie albo informacja. Znajduje:
  - domyślne i puste nazwy, elementy bez opisu, sieroty;
  - powtórzone nazwy i relacje, relacje do nieistniejących elementów;
  - puste pakiety i diagramy, przepływy niezgodne z regułami UML lub BPMN.
  - Przykład: „Sprawdź jakość pakietu Domena i wypisz najważniejsze problemy.”
- **Reguły organizacji.** W pliku governance.json obok ustawień albo z huba zespołu: konwencje nazw, wymagane tagged values, dozwolone relacje między typami (metamodel / MDG).
  - Przykład: „Sprawdź, czy wszystkie komponenty w pakiecie Systemy mają tagged value Owner.”
- **Analiza luk as-is / to-be.** Co nowe, co do wycofania, co zmienione (stereotyp, opis, relacje); na życzenie do Excela.
  - Przykład: „Porównaj pakiet As-is z pakietem To-be i zapisz luki do Excela.”
- **Pisownia w języku Compassa.** Notatki elementów i diagramów, kroki scenariuszy i słownik projektu są sprawdzane słownikiem pisowni Windows. Nazwy elementów i pojęcia słownika to znane słowa; poprawki dopiero po Twojej zgodzie.
  - Zapis języka w ustawieniach przełącza też korektę EA na ten język, jeśli EA ma dla niego słownik.
  - Brak języka: Ustawienia Windows → Czas i język → Język → dodaj język.
  - Przykład: „Sprawdź pisownię notatek w całym modelu i popraw błędy.”

### Zapisy, którym można ufać

- **Podgląd „przed → po”.** Okno zatwierdzania pokazuje, co powstaje, co zmienia się z jakiej wartości na jaką i co jest usuwane.
- **Weryfikacja po każdym zapisie.** Compass czyta model ponownie i porównuje go z poleceniem. Gdy coś się różni, asystent mówi o tym zamiast pisać „gotowe”. Sprawdza, czy:
  - utworzone obiekty istnieją, relacje mają właściwe końce, elementy są we wskazanym pakiecie;
  - nazwy, opisy, stereotypy i tagged values mają nowe wartości;
  - usunięte obiekty zniknęły, pliki XMI i raporty zostały zapisane.
- **Zapis „wszystko albo nic”.** Gdy zapis nie uda się w połowie, obiekty, które zdążył utworzyć, są usuwane.
- **Bezpieczne usuwanie.** Usunięcie elementu, który ma relacje, zawsze wymaga zgody (także w trybie auto), a okno wymienia te relacje.
- **Wehikuł czasu.** Każdy zapis trafia do dziennika cofania z operacjami odwrotnymi; cofnąć można to, co powstało lub się zmieniło. Usunięcia Compass nie cofa – przed nim robi baseline EA pakietu, który przywracasz w EA. Przed cofnięciem zawsze pyta o zgodę.
  - Przykłady: „Cofnij ostatnią zmianę.”, „Pokaż historię zmian, które zrobiłeś w tym modelu.”, „Cofnij dwie ostatnie zmiany.”

### Wymagania, kod i wycena

- **Wymagania a model.** Dokument Word, tekst lub Markdown, strona Confluence albo tekst wklejony do rozmowy zamienia się w macierz śledzenia: pokryte, prawdopodobnie pokryte, niepokryte, „ten sam identyfikator, inna treść” oraz elementy modelu, których nie wymaga żadne wymaganie. Relacje śledzenia (trace) powstają dopiero po Twojej zgodzie.
  - Przykład: „Porównaj wymagania z dokumentu C:\Projekty\BRD.docx z modelem: co jest pokryte, a czego brakuje?”, potem „Dodaj relacje trace dla pokrytych wymagań.”
- **Model a kod.** Folder z kodem, tylko do odczytu: klasy, interfejsy, enumy i rekordy z polami (Java, Kotlin, C#, TypeScript, Rust, Python), tabele SQL z kolumnami, operacje API (OpenAPI operationId). Raport pokazuje, czego brakuje w modelu, czego nie ma już w kodzie i czym różnią się pola.
  - Przykład: „Porównaj pakiet Domena z kodem w folderze C:\repo\sklep – czego brakuje w modelu, a czego w kodzie?”
- **Wycena zmiany.** Wpływ zmiany zamienia się w listę zadań w prostym języku (zmienić, dostosować, sprawdzić) z godzinami według typu elementu i poziomu powiązania, aktualizacją diagramów, testami, analizą, dokumentacją i buforem ryzyka; na życzenie do Excela.
  - Normy są w pliku estimate.json obok ustawień. To jeden ze sposobów wyceny, nie jedyny: dopasuj normy do swojego zespołu.
  - Przykład: „Wyceń zmianę klasy Zamówienie: co trzeba zmienić i ile to zajmie?”

### Nowe operacje na modelu

- **Automatyczny układ diagramu.** W dół, w prawo, w górę albo w lewo, z wybranym odstępem.
  - Przykład: „Ułóż diagram Model domeny od lewej do prawej.”
- **Eksport i import pakietu XMI.** Import nadaje nowe GUID-y, chyba że poprosisz o ich zachowanie.
  - Przykład: „Wyeksportuj pakiet Domena do pliku C:\Eksport\domena.xml.”
- **Dokumenty z szablonów dokumentów EA.** DOCX, PDF albo RTF, jak Publish → Report Builder.
  - Przykład: „Wygeneruj raport z szablonu Model Report dla pakietu Domena do pliku C:\Raporty\domena.docx.”
- **Więcej edycji.** Pasy (swimlanes) diagramu, dokumenty powiązane elementów, odwrócenie relacji, dane pakietu (nazwa, opis, wersja, status, stereotyp). Agregacja i kompozycja powstają jako właściwe relacje UML.
  - Przykłady: „Dodaj na diagramie Proces zamówienia tory Sprzedaż i Magazyn.”, „Odwróć relację między Klientem a Zamówieniem.”
- **Scenariusze elementów (structured scenarios).** Scenariusze z krokami aktora i systemu.
  - Przykład: „Dodaj do przypadku użycia Złóż zamówienie scenariusz główny w pięciu krokach.”

### Skrypty EA

- **Czytanie, zapis i uruchamianie skryptów.** Asystent pokazuje grupy i skrypty i czyta skrypt. Zapisuje JScript, VBScript i JavaScript (brakująca grupa powstaje sama; zatwierdzane jak każdy zapis) i uruchamia skrypt za każdym razem po Twojej zgodzie, w otwartym oknie EA albo w tle.
  - Repository i Session.Output działają jak w EA; dołączenia „!INC Grupa.Skrypt” są rozwiązywane z modelu.
  - Przykład: „Utwórz i uruchom skrypt EA w JScript, który wypisze nazwy wszystkich pakietów modelu.”

### Praca zespołowa: Compass Hub

- **Opcjonalna usługa zespołu.** `LoonarCompass.exe --hub hub.json` na komputerze lub serwerze zespołu. W hub.json: adres nasłuchu, modele zespołu (bazy i pliki EA), użytkownicy z tokenami i rolami (viewer, architect, admin) oraz polityka.
  - W Compassie: Ustawienia → karta „Zespół (Compass Hub)”, z adresem i Twoim tokenem od administratora huba.
- **Wspólna polityka.** Reguły kontroli modelu i minimalny tryb zatwierdzania obowiązują na wszystkich stanowiskach; każdy zapis trafia do centralnego dziennika zmian; wspólne notatki zespołu.
- **Zestawy zmian do przeglądu.** Zamiast zapisywać, asystent zgłasza zmianę do huba. Drugi architekt komentuje ją i zatwierdza w widoku webowym (zasada czterech oczu), a autor wprowadza zatwierdzony zestaw z weryfikacją.
  - Przykłady: „Zaproponuj zespołowi zmianę do przeglądu: dodaj klasę Magazyn do pakietu Domena.”, „Pokaż zatwierdzone zestawy zmian i wprowadź zestaw 3.”
- **Widok webowy dla osób bez EA.** Adres huba w przeglądarce, z zakładkami Ask, Search, Model check, Gaps, Audit trail, Team notes i Change sets. W zakładce „Ask” interesariusz zadaje pytanie o model, a asystent odpowiada tylko z odczytu.
- **Serwer MCP huba.** Tylko do odczytu, dla innych narzędzi AI: źródła, drzewo pakietów, wyszukiwanie i szczegóły elementów, analiza wpływu, kontrola modelu i analiza luk.

### Jira i własne serwery MCP

- **Jira (Cloud i Data Center).** Zgłoszenia po kluczu albo wyszukiwaniem JQL jako kontekst do modelu; za Twoją zgodą nowe zgłoszenia, komentarze i linki do elementów. Gdzie: Ustawienia → karta „Jira”.
  - Przykłady: „Przeczytaj zgłoszenie SHOP-123 i sprawdź, których elementów modelu dotyczy.”, „Utwórz w Jira zgłoszenie na brakujący opis klasy Faktura.”
- **Własne serwery MCP.** Karta „Serwery MCP”, zapis w mcp_servers.json: program lokalny (stdio) albo HTTP; tokeny z ${secret:nazwa} (Menedżer poświadczeń) lub ${env:NAZWA}.
  - Narzędzie, które nie jest tylko do odczytu, pyta o zgodę przed każdym wywołaniem, chyba że serwer ma "trusted": true.
  - „Zapisz i sprawdź serwery MCP” pokazuje liczbę narzędzi każdego serwera.
  - Przykład: „Przez serwer MCP hub wypisz źródła modeli zespołu.”

### Compass w Enterprise Architect

- **Kontrola na bieżąco.** Nowo narysowana relacja i zmieniony element są sprawdzane od razu (reguły i pisownia notatek); uwagi pojawiają się w zakładce „Loonar Compass” okna System Output.
- **Agenci zdarzeń.** Karta „Agenci zdarzeń (Compass w EA)”, zapis w event_agents.json: gdy powstanie lub zmieni się element albo powstanie relacja lub diagram, asystent robi to, co mówi reguła.
  - Czyta model i odpowiada w System Output (linie od 🤖); nigdy sam nie zmienia modelu; ten sam agent i obiekt najwyżej raz na minutę.
  - Przykładowa reguła dla nowego komponentu: „sprawdź, czy ma opis i właściciela, i zaproponuj brakujące relacje”.
- **Zmiana nazwy rozmowy.** Ołówek obok listy rozmów.
- **Odczyt pliku modelu otwartego w EA.** Odczyty idą przez okno EA, więc widzą ostatnie zmiany, zanim EA zapisze je na dysk.

### Inne zmiany i poprawki

- Ustawienia bez numeracji kart; pierwsza karta to „Model AI”.
- Komunikaty pracy mówią „Asystent …” zamiast „Claude …”.
- Model w pliku: po każdym zapisie zmiany są od razu na dysku, więc kolejne odczyty widzą nowy stan.
- Okna błędów EA (SQL, XML) nie blokują już pracy w tle; zapytania z wyrażeniami i powtórzonymi nazwami kolumn zwracają poprawne wyniki.
- Instrukcja użytkownika w 4 językach: nowy rozdział z przypadkami użycia i gotowymi poleceniami, rozdział „Compass w Enterprise Architect” oraz karty „Model AI”, „Jira”, „Zespół”, „Serwery MCP” i „Agenci zdarzeń”.

## Wersja 0.7.5 (2026-10-22)

- Poprawka: Compass w EA wypełnia cały panel i dopasowuje się przy każdej zmianie jego rozmiaru, także w oknie pływającym i w grupie zakładek.
- Okno „Informacje” i stopka pokazują pełny numer wersji (np. 0.7.5).

## Wersja 0.7.4 (2026-10-21)

- Poprawka: Compass w EA znów dopasowuje się do rozmiaru panelu przy każdej jego zmianie (w 0.7.3 zostawał w rozmiarze z otwarcia).

## Wersja 0.7.3 (2026-10-21)

- Compass w EA: pisanie działa po „Dodaj do rozmowy w Compass”, po kliknięciu w drzewo lub diagram i po przełączeniu paneli (klawiatura trafia do okna Compassa).
- Okno Compassa mieści się w widocznej części panelu EA, więc dolny pasek ikon zostaje na ekranie.
- Błędne zapytanie asystenta do bazy otwartej w EA nie zatrzymuje już EA okienkiem błędu: Compass zamyka je sam i przekazuje treść błędu asystentowi, który poprawia zapytanie.

## Wersja 0.7.2 (2026-10-21)

- Compass w EA pracuje z każdą bazą otwartą w EA, także spoza ustawień: czyta model przez połączenie samego EA, bez haseł i konfiguracji.
- Okno Compassa zawsze dokuje się w panelu EA (wcześniej czasem zostawało obok jako osobne okno).

## Wersja 0.7.1 (2026-10-21)

- Słownik projektu EA (Glossary): asystent dodaje pojęcie z definicją i typem (domyślnie „Slownik”); polskie pojęcie trafia we wszystkich formach (liczba pojedyncza i mnoga, od mianownika do wołacza), więc EA podświetla w notatkach każdą odmianę. W polu wiadomości: prawy przycisk na słowie → „Dodaj do słownika projektu EA”, albo prośba w rozmowie.
- Compass w EA dopasowuje się do szerokości panelu: tekst zawija się zamiast znikać z prawej strony.
- Menu podręczne elementu, pakietu i diagramu: „Dodaj do rozmowy w Compass” znów bezpośrednio w Specialize.
- Klawiatura działa po przełączeniu paneli EA (np. Notes → Compass); Compass zamyka się razem z EA.

## Wersja 0.7 (2026-10-21)

Compass w Enterprise Architect:

- Add-in do EA: przycisk „Loonar Compass” na wstążce Start (grupa Add-Ins) otwiera Compass jako okno dokowane w EA, w tym samym miejscu co Properties (EA zapamiętuje, gdzie je położysz). Instalator ma pole „Dodaj Loonar Compass do Enterprise Architect”; działa z każdą instalacją EA, także wersją próbną.
- System Output ma zakładkę „Loonar Compass”: Twoje wiadomości i kolejne kroki asystenta (odczyty modelu, zapisy do EA, wyniki) z godziną. W czacie zostaje tylko jedna linia z ostatnim krokiem.
- W EA Compass pracuje zawsze na projekcie otwartym w tym oknie EA (pliku lub bazie): bez wybierania modelu i bez zatwierdzania. Lista pokazuje tylko rozmowy tego projektu, a zmiana projektu w EA przełącza Compass. Bez otwartego projektu Compass nie działa.
- Zapisy idą przez to samo okno EA, więc zmiany widać od razu w drzewie i na diagramach.
- „Pokaż diagram” i „Edytuj diagram” otwierają diagram w EA, bez obrazków w czacie.
- Menu kontekstowe elementu, pakietu lub diagramu: „Dodaj do rozmowy w Compass” wstawia do wiadomości wskazanie obiektu, np. «Pakiet: Sklep (id 5)».
- Układ okna dokowanego: mniejsza czcionka, rozmowy projektu na liście rozwijanej z przyciskiem „+”, ikony widoków na dole, przewijanie stron w poziomie i pionie; Ctrl+V wkleja zdjęcia, pliki i tekst.
- W ustawieniach w EA zostają tylko Claude, tryb zatwierdzania, Confluence i Miro; model i EA wynikają z otwartego projektu.

Wszędzie:

- Słownik: prawy przycisk na podkreślonym słowie w polu wiadomości → „Dodaj do słownika”. Polskie wyrażenie trafia do słownika Windows we wszystkich formach (liczba pojedyncza i mnoga, od mianownika do wołacza).

## Wersja 0.6.5 (2026-10-20)

- Konto EA należy do modelu, nie do aplikacji: konto i hasło EA ustawia się w karcie „2. Model”, przy pliku albo bazie. Karta „3. Enterprise Architect” ma tylko ścieżkę do EA.exe i przycisk „Zapisz i sprawdź dostęp do EA”.
- Plik i baza: pole wyboru „Model ma włączone zabezpieczenia (konta użytkowników EA)”. Bez niego pola konta i hasła są nieaktywne, a EA otwiera model bez logowania.
- Nowa opcja „Model wymaga blokady do edycji (Require User Lock to Edit)”: z nią asystent blokuje zmieniane pakiety i elementy, a potem zwalnia blokady; bez niej nie blokuje.
- Test ustawień modelu i modelu rozmowy zapisuje do modelu pakiet próbny, sprawdza go i od razu usuwa.
- Nowa rozmowa zaczyna się od zatwierdzenia modelu („Zatwierdź model i rozpocznij”). Potem model rozmowy jest ustalony i widoczny tylko do podglądu; zmiana modelu domyślnego w ustawieniach nie zmienia trwających rozmów.
- Poprawka: rozmowa trafia na listę od razu po pierwszej wiadomości. Wcześniej rozmowa, w której asystent jeszcze pracował, nie była widoczna na liście.
- Poprawka: partycje (tory) diagramu aktywności wyższe niż szersze rysują się w EA pionowo, z nazwą u góry.
- Poprawka: podgląd diagramu otwartego w oknie EA pokazuje stan po ostatnim zapisie (EA przeładowuje diagram przed eksportem obrazka).

## Wersja 0.6.4 (2026-10-19)

- Rysowanie z obrazka: asystent odwzorowuje układ załącznika (rzędy, kolejność, położenie względem sąsiadów, przebieg linii) zamiast układać diagram od nowa; mierzy obrazek przed rysowaniem, zostawia co najmniej 40 px odstępu między elementami, a po narysowaniu sprawdza najpierw każdą relację, potem układ (najwyżej dwie rundy poprawek).
- Styl linii całego diagramu stosuje się po utworzeniu relacji i nie kasuje linii poprowadzonych ręcznie.
- Węzeł obiektu (ObjectNode) w diagramie aktywności zapisuje się jako obiekt EA, zamiast przerywać zapis.
- Krótkie komentarze asystenta między krokami są w języku rozmowy.

## Wersja 0.6.3 (2026-10-19)

- Edytor diagramów: zapis do EA przenosi linie dokładnie tak, jak je widać w edytorze (poziome i pionowe odcinki, miejsca styku z elementami), także dla nowych relacji. Wcześniej EA dostawało tylko ręcznie przesunięte punkty i rysowało linie na skos.

## Wersja 0.6.2 (2026-10-19)

- Poprawka: rozwidlenie / scalenie (fork/join) na diagramie aktywności EA rysowało poziomo, choć miało być pionowe. Orientację ustawia teraz aplikacja z kształtu elementu (wyższy niż szerszy = pionowy).
- Edytor diagramów: „Rozwidlenie / scalenie” w przyborniku diagramu aktywności.

## Wersja 0.6.1 (2026-10-19)

- Poprawka: edytor diagramów nie otwierał diagramów modelu w bazie PostgreSQL (błąd „column \"default\" does not exist”). Modele w plikach .qea/.qeax działały.

## Wersja 0.6 (2026-10-18)

Edytor diagramów:

- Pod każdym diagramem z EA w rozmowie jest przycisk „Edytuj diagram”. Otwiera diagram w oknie edytora (WebView2): przesuwanie, zmiana rozmiaru, wyrównywanie, zmiana nazw, cofanie (Ctrl+Z / Ctrl+Y). Edytor zmienia model, nie obrazek: zapis idzie przez EA, z panelem zatwierdzania i dziennikiem zmian.
- Nowe elementy z przybornika dopasowanego do rodzaju diagramu: klas, komponentów, przypadków użycia, aktywności, BPMN, maszyny stanów (stany, początek, koniec, wybór), wdrożenia (węzły, urządzenia, środowiska wykonawcze, artefakty), pakietów (prawdziwe pakiety) i sekwencji. Nowy element trafia w wolne miejsce; łączenie przeciągnięciem od elementu do elementu.
- Łączenie: po najechaniu na element pojawia się niebieska strzałka; przeciągnij ją na drugi element. Rodzaj relacji wybierasz z listy „Połączenie”.
- BPMN: typy zadań (użytkownika, usługi, ręczne, skryptu, wysłania, odbioru, reguły biznesowej), zdarzeń (wiadomość, czas, sygnał, błąd…) i bramek (rozłączna, włączna, równoległa, złożona, oparta na zdarzeniach) jako ikony i znaczniki, wybór typu w zakładce „Ogólne”, zdarzenie pośrednie w przyborniku. Klasy powiększają się same, gdy dochodzą atrybuty i operacje.
- „Z modelu…”: wstawianie na diagram elementów, które już są w modelu, z wyszukiwarką.
- Usuwanie pyta: z tego diagramu czy z modelu. Przy usuwaniu z modelu pokazuje, na ilu i na jakich innych diagramach jest element i ile ma relacji.
- Panel „Właściwości”: pola jak na karcie ogólnej EA (nazwa, alias, stereotyp, słowa kluczowe, status, wersja, autor, faza, złożoność, abstrakcyjny, notatki; pakiet, GUID i daty do odczytu), atrybuty i operacje klas, tagi (z listami wartości z profilu, np. BPMN), wygląd na diagramie (kolor, styl linii). Każde pole ma znaczek „model” albo „diagram”.
- Relacje: nazwa, stereotyp, kierunek, krotności i role obu końców, notatki, tagi.
- Diagramy sekwencji: linie życia, komunikaty w kolejności z EA (zmiana kolejności ▲ ▼ albo Alt+↑ / Alt+↓), komunikaty synchroniczne, asynchroniczne i odpowiedzi, nazwa z operacji odbiorcy.
- „Uporządkuj z AI”: zapisuje zmiany i prosi asystenta w rozmowie o czytelne rozmieszczenie diagramu.
- Nowe operacje zapisu dla asystenta: zmiana relacji, zmiana i usuwanie atrybutów i operacji, podtyp elementu (pseudostany), komunikaty sekwencji z numerem, rodzajem i odpowiedzią; usuwanie tagu.

Miro (z EA do Miro):

- Karta ustawień „7. Miro”: token dostępu (w Menedżerze poświadczeń Windows) i test połączenia.
- „Wyślij do Miro” pod diagramem: wybór tablicy, miniatura tablicy z miejscem na diagram (aplikacja sama znajduje wolne miejsce, ramkę można przeciągnąć), wysyłka jako edytowalne kształty i połączenia w ramce z nazwą diagramu. Diagramy sekwencji: linie życia i ponumerowane komunikaty w kolejności z EA.

Dokumentacja:

- Nowe rozdziały instrukcji: „9. Edytor diagramów” i „10. Wysyłanie diagramów do Miro” (kolejne rozdziały przesunęły się o dwa); na stronie na telefonie ikona dokumentacji w nagłówku.

## Wersja 0.5 (2026-10-17)

Model dla każdej rozmowy:

- Każda rozmowa ma własny model. Pokazuje go nagłówek rozmowy („Model: …”); kliknięcie otwiera model tej rozmowy, w którym możesz go sprawdzić albo zmienić.
- Nowa rozmowa mówi, którego modelu użyje: domyślnego z ustawień. „Wybierz inny model…” pozwala wskazać inną bazę PostgreSQL (serwer, port, baza, konto, hasło, szyfrowanie) albo plik EA (.qea, .qeax), z kontem EA, i sprawdzić połączenie.
- Hasła połączenia innego niż w ustawieniach trafiają do Menedżera poświadczeń Windows; odinstalowanie usuwa je razem z pozostałymi sekretami, jeśli to wybierzesz.
- Upuszczenie pliku .qea albo .qeax na okno ustawia go jako model rozmowy (w rozmowie z wiadomościami zaczyna się nowa rozmowa).
- Istniejące rozmowy dostają kopię modelu z ustawień, więc pracują z nim dalej także po późniejszej zmianie ustawień.
- Pamięć asystenta pozostaje wspólna dla wszystkich modeli. Notatka dotyczy tylko jednego modelu wyłącznie wtedy, gdy o to poprosisz („zapamiętaj to tylko dla tego modelu”).

## Wersja 0.4 (2026-10-17)

Modele w plikach:

- Model może być teraz plikiem projektu Enterprise Architect .qea albo .qeax: Ustawienia → „2. Model” → „Plik EA (.qea, .qeax)” i ścieżka do pliku. Nie trzeba serwera bazy, konta ani hasła.
- Asystent czyta plik bezpośrednio i tylko do odczytu. Zmiany zapisuje EA: przez Twoje okno EA, gdy masz w nim otwarty ten plik (drugie EA nigdy nie otwiera tego samego pliku), a w przeciwnym razie przez EA w tle.
- Zabezpieczenia użytkowników w pliku są opcjonalne: bez nich konto i hasło EA zostają puste. Plik z włączonymi zabezpieczeniami działa jak model w bazie, z kontem EA.
- Instalator ma ten sam wybór na stronie konfiguracji.
- Asystent pisze zapytania w dialekcie SQL miejsca, w którym jest model (SQLite dla plików, PostgreSQL dla baz).

## Wersja 0.3 (2026-10-17)

Nowa nazwa i publikacja:

- Nowa nazwa: Loonar Compass (wcześniej Loonar EA Assistant). Ustawienia, rozmowy, pamięć, szablony i hasła z poprzedniej wersji przechodzą automatycznie przy pierwszym uruchomieniu; pliki z polskimi nazwami w folderze danych dostały nazwy angielskie (`audit_log.jsonl`, `confluence_pages.json`).
- Kod źródłowy jest publiczny na GitHubie (github.com/loonar-eu/loonar-compass), a strona loonar.eu opisuje aplikację w czterech językach, z pytaniami i odpowiedziami (FAQ).
- Nowa licencja freeware (wiążąca wersja polska, tłumaczenia EN/DE/FR): korzystanie także komercyjne, bez ograniczeń czasu i terytorium, z zasadami dotyczącymi wyników AI, danych i komponentów open source. Lista licencji komponentów open source w pliku THIRD-PARTY-NOTICES.txt i w widoku „Informacje”.
- Widok „Wesprzyj projekt” (ikona serca w pasku nawigacji): dobrowolne wsparcie przez suppi.pl/loonar.
- Polityka prywatności i nota prawna na loonar.eu; w aplikacji krótkie informacje przy kluczu API (co trafia do Anthropic), przy zgłoszeniu błędu i przy newsletterze, a pod polem wiadomości przypomnienie, że odpowiedzi przygotowuje model AI.
- Nowe tło ekranu startowego i banera instalatora (mapa nieba z siatką globusa i różą kompasu).

Bezpieczeństwo i prywatność:

- Hasło do bazy jest w Menedżerze poświadczeń Windows, tak jak klucz API, hasło konta EA i token Confluence; hasło zapisane wcześniej w pliku ustawień jest przenoszone automatycznie.
- Szyfrowanie połączenia z bazą modelu (TLS), karta „2. Baza modelu”: preferowane (domyślnie), wymagane albo wymagane z weryfikacją certyfikatu w magazynie certyfikatów Windows. Test połączenia mówi, czy połączenie jest szyfrowane, i ostrzega przed kontem superużytkownika.
- Confluence tylko przez HTTPS (http:// wyłącznie dla serwera na tym samym komputerze). Przed publikacją asystent sprawdza, czy strona o tym tytule już istnieje, i mówi wprost, że jej treść zostanie zastąpiona.
- Usuwanie, przenoszenie i kopiowanie pakietów wymaga osobnej zgody także w trybie plan, a panel zgody podaje nazwy obiektów. Zapis do pamięci asystenta i nadpisanie szablonu przechodzą przez zatwierdzanie jak zmiany w modelu.
- Treść załączników, dokumentów i modelu asystent traktuje jako dane, a nie polecenia. Obrazek rysowany przez asystenta nie może wczytać pliku z dysku ani z sieci, a jego rozmiar jest ograniczony.
- Odczyt z bazy: limit czasu zapytania i blokada funkcji serwera, które czytają pliki albo wpływają na inne sesje. Do modelu nie trafia tekst połączenia z otwartego okna EA z danymi logowania.
- Programy systemowe i biblioteki są uruchamiane wyłącznie z folderu Windows, nigdy z folderu pobierania.

Instalacja:

- Instalacja bez uprawnień administratora: domyślnie tylko dla bieżącego użytkownika (`%LOCALAPPDATA%\Programs\Loonar Compass`); instalacja dla wszystkich użytkowników prosi o uprawnienia administratora. Ustawienia i hasła trafiają zawsze do profilu zalogowanego użytkownika.
- Wcześniejsza wersja (Loonar EA Assistant) zainstalowana tylko dla Ciebie jest usuwana podczas instalacji; tę dla wszystkich użytkowników można usunąć przyciskiem w instalatorze.
- Odinstalowanie usuwa na życzenie cały folder danych oraz wszystkie sekrety z Menedżera poświadczeń (także token Confluence i hasło do bazy).
- Okno instalatora mieści się na małych ekranach i przy dużym skalowaniu; usunięto skrót „Szybkie uruchamianie”, pole „Model” i pole Workspace ID.

Stabilność:

- Zapytania do Claude API są ponawiane przy przeciążeniu, limicie zapytań i chwilowych błędach serwera; błędny klucz API i blokada przez proxy mają czytelne komunikaty.
- Proxy z ustawień internetowych Windows (oprócz zmiennej HTTPS_PROXY).
- Przy nieoczekiwanym błędzie aplikacja pokazuje komunikat i zapisuje szczegóły do pliku zamiast znikać; błąd w pliku ustawień jest opisany zamiast „brak ustawień”.
- Poprawione zamykanie aplikacji przy długich podpisach obrazków z literami spoza ASCII na stronach Confluence.
- Aplikacja i instalator uruchamiają się także na starszych wydaniach Windows 10 i bez pakietu Visual C++ Redistributable.

Języki:

- Cztery języki: polski, angielski, niemiecki i francuski. Instalator zaczyna od wyboru języka (domyślnie angielski), który ustawia język aplikacji, odpowiedzi asystenta i domyślnych szablonów dokumentów. Język można zmienić w Ustawieniach (karta „4. Język i zatwierdzanie zmian”).
- Wszystko, co jest dostępne bez AI, jest w czterech językach: okno aplikacji i instalatora, szablony dokumentów, licencja, instrukcja użytkownika i lista zmian.
- Asystent odpowiada w języku aplikacji, chyba że piszesz do niego w innym języku.
- Po zmianie języka niezmodyfikowane szablony domyślne są od razu zastępowane wersjami w nowym języku, a pamięć asystenta oraz własne i zmodyfikowane szablony tłumaczy Claude. Bez połączenia z Claude na ikonach Ustawień, Pamięci i Szablonów pojawia się pomarańczowy wykrzyknik z wyjaśnieniem w podpowiedzi, a tłumaczenie kończy się automatycznie, gdy połączenie zacznie działać (albo po kliknięciu „Przetłumacz teraz”). Historia rozmów nigdy nie jest tłumaczona.
- Nowe, prostsze logo (płaskie, dwukolorowe, czytelne także w małym rozmiarze i w druku) w ikonie aplikacji, instalatora i w widoku „Informacje”.

Dokumentacja do Confluence:

- Strony Confluence przygotowywane przez asystenta z modelu i rozmowy: kod strony w formacie edytora kodu źródłowego (storage XHTML) z nagłówkami, tabelami, listami, spisem treści, panelami (informacja, uwaga, ostrzeżenie, wskazówka, sekcja rozwijana), statusami, zgłoszeniami Jira, wzmiankami o osobach i blokami kodu; diagramy z EA i obrazki jako załączniki. Na czacie karta strony z przyciskami „Kopiuj kod strony” i „Otwórz folder z obrazkami”.
- Publikacja w Confluence przez REST API (na prośbę, zawsze po zatwierdzeniu, także w trybie auto): nowa strona pod stroną nadrzędną albo aktualizacja istniejącej (po ID albo tytule), obrazki jako załączniki (istniejące dostają nową wersję). Diagramy są eksportowane na nowo przy każdej publikacji, więc ponowna publikacja odświeża stronę po zmianach w modelu; asystent pamięta opublikowane strony.
- Ustawienia, karta „6. Confluence”: adres, token (Data Center: token osobisty; Cloud: e-mail i token API), klucz przestrzeni i strona nadrzędna, z przyciskiem „Zapisz i sprawdź połączenie z Confluence”.

Szablony dokumentów:

- Szablony dokumentów, które asystent wypełnia treścią z modelu (strona Confluence albo dokument Word), w folderze `templates` obok historii rozmów: Wstępna analiza wykonalności, BRD - Wymagania biznesowe, Opis zmian w systemie, Opis procesu. Szablony to pliki Markdown ze wskazówkami dla asystenta; można je zmieniać, dodawać własne albo poprosić asystenta, żeby zrobił szablon z przykładowego dokumentu.
- Widok „Szablony dokumentów” w pasku nawigacji: lista z opisem, „Użyj” (wstawia do czatu polecenie przygotowania dokumentu), „Edytuj” i „Otwórz folder szablonów”.

## Wersja 0.2 (2026-10-15)

Pamięć:

- Pamięć między rozmowami: asystent zapisuje ustalenia (konwencje modelowania, preferencje, poprawki, fakty o projektach, wszystko po „zapamiętaj…”) jako lokalne pliki Markdown i zna je w każdej nowej rozmowie. Ustalenie na ten sam temat aktualizuje, zamiast dublować, a nieaktualne usuwa. Zapis ustalenia w trakcie odpowiedzi nie przerywa dalszej pracy (wcześniej API zwracało błąd 400 o podpisie bloku „thinking”).
- Widok „Pamięć asystenta” w pasku nawigacji: lista ustaleń z datą, przyciski „Usuń” i „Otwórz folder pamięci” (folder `memory` obok historii rozmów, ze spisem `MEMORY.md`).

Enterprise Architect:

- Prowadzenie połączenia po wskazanej trasie, np. według szkicu na zrzucie: punkty załamania i miejsca styku z bokami elementów (linie tylko poziome i pionowe albo dowolne).
- Zmiana przebiegu linii połączeń na diagramie (jak Line Style w EA): prosta, automatyczna, łamana, prostokątna, prostokątna zaokrąglona, drzewo i boczna; dla wybranych połączeń albo dla wszystkich na diagramie.
- Analiza wpływu zmiany: od wskazanego elementu po relacjach w głąb (do 6 poziomów, w jedną albo obie strony, opcjonalnie tylko wybrane typy relacji), z relacją, przez którą dochodzi wpływ, ścieżką i diagramami, na których leżą dotknięte elementy; wynik do opisu, obrazka, dokumentu albo Excela.
- Gdzie użyte są operacje (usługi) klasy, komponentu albo interfejsu: diagramy sekwencji (komunikaty do elementu albo jego linii życia, powiązane z operacją albo o zgodnej nazwie) i aktywności (akcje wywołujące operację), z pakietem i wywołującym, a także lista operacji nieużywanych na żadnym diagramie.
- Atrybuty i operacje klas (z parametrami), kopia pakietu z całą zawartością oraz baseline (zapisana wersja) pakietu. Przed usuwaniem albo większą przebudową istniejącego pakietu asystent ma robić baseline (to zalecenie dla asystenta, nie blokada: przy ważnych pakietach poproś o baseline wprost); przywraca się go w EA (Manage Baselines → Compare/Restore), bo przywracanie przez automatyzację zawiesza EA.
- Współpraca z otwartym oknem EA (ten sam model): asystent wie, jaki diagram masz otwarty i co jest na nim zaznaczone (także w przeglądarce projektu), otwiera diagramy i wskazuje elementy w przeglądarce, a po zapisie sam odświeża zmienione diagramy i pakiety w Twoim oknie EA, bez F5.
- Dziennik zmian: każdy zapis asystenta do modelu (także nieudany) w pliku `dziennik_zmian.jsonl` obok historii rozmów; widok „Dziennik zmian” w pasku nawigacji z wynikiem, czasem, kontem i bazą oraz eksportem do Excela.
- Słowa kluczowe elementów (pole Keywords w EA) przy tworzeniu i edycji.
- Legenda diagramu z automatycznym kolorowaniem, jak w EA: filtr według właściwości elementu (domyślnie słowo kluczowe, także stereotyp, status, typ) lub połączenia (np. stereotyp), pozycje z wartością, kolorem i podpisem.
- Zmiana nazwy, opisu i stereotypu istniejącego diagramu.
- Ukrywanie wybranych połączeń na jednym diagramie (i ponowne pokazywanie): relacje zostają w modelu i na innych diagramach, więc gęsty diagram da się odciążyć bez usuwania relacji.

- Zmiana położenia i rozmiaru elementu, który już jest na diagramie (np. powiększenie ramki, puli albo podprocesu), bez zdejmowania go z diagramu; połączenia zostają.
- Powiązanie elementu (np. podprocesu BPMN) z istniejącym diagramem jako diagramem złożonym, tak że dwukrotne kliknięcie otwiera diagram. Operacja była w aplikacji, ale asystent o niej nie wiedział i odsyłał do ręcznej zmiany w EA.
- Podprocesy BPMN osadzone (embedded): kroki jako elementy podrzędne podprocesu, narysowane w jego prostokącie na tym samym diagramie (tak jak zapisuje je import BPMN z Camundy); obok podprocesów zwiniętych, z krokami na osobnym diagramie.
- Dodawanie kroków do istniejącego podprocesu: element podrzędny powstaje w pakiecie podprocesu i jest z nim wiązany (jak w bazie EA). Wcześniej EA odrzucał to błędem „Element locked”, choć blokadę trzymał sam asystent.

Uruchamianie:

- Zamknięcie aplikacji zamyka też jej sesję EA. Wcześniej proces EA zostawał w tle i zajmował licencję, przez co kolejne sesje EA mogły czekać bez końca.
- Okno otwiera się także na kartach graficznych o mniejszych możliwościach (np. w Wine przez Vulkan): aplikacja wymaga od karty tylko tego, co karta obsługuje.
- Komunikacja z EA przez COM bez systemowej biblioteki propsys, więc działa także w Wine.

Dokumenty i obrazki:

- Dokumenty Word (DOCX) tworzone przez asystenta: nagłówki, akapity, listy, tabele, diagramy z EA i obrazki narysowane wcześniej; na czacie karta pliku z przyciskami „Otwórz” i „Zapisz jako…”.
- Arkusze Excel (XLSX) tworzone przez asystenta: kilka arkuszy, nagłówki z filtrem i zamrożonym pierwszym wierszem, liczby jako liczby (np. lista elementów, macierz, wynik analizy wpływu).
- Obrazki rysowane przez asystenta (np. macierz przypadków użycia i wymagań, tabela, wykres, schemat) jako PNG na czacie, z przyciskiem „Zapisz jako…”; asystent widzi wynik i może go poprawić.

Kontakt:

- Widok „Zgłoś błąd” w pasku nawigacji: temat, opis, zdjęcia (przeciągnięte do okna, wklejone ze schowka albo wybrane z dysku, z miniaturami), opcjonalny e-mail do odpowiedzi i informacje techniczne (bez haseł i kluczy). „Wyślij zgłoszenie” otwiera w programie pocztowym Windows gotową wiadomość do mariusz@loonar.eu ze zdjęciami jako załącznikami (Simple MAPI); gdy program pocztowy tego nie obsługuje, otwiera wiadomość bez zdjęć i folder ze zdjęciami do przeciągnięcia.
- Ustawienia, karta „5. Nowe wydania (newsletter)”: adres e-mail i zgoda na informacje o nowych wydaniach; wybór jest zapamiętywany w ustawieniach, a zapis i rezygnacja trafiają do autora wiadomością z programu pocztowego.

Wygląd:

- Nowy wygląd okna, funkcje bez zmian: granatowy pasek nawigacji po lewej z ikonami (Czat, Ustawienia, Changelog, Informacje) zamiast zakładek u góry, ciepłe, jasne tło jak w Claude, czcionka Segoe UI z półgrubymi nagłówkami.
- Lista rozmów pogrupowana na „Dzisiaj”, „Wczoraj”, „Ostatnie 7 dni” i „Wcześniej”; nowa rozmowa przyciskiem „+”, wybrana rozmowa podświetlona, kropki stanu bez zmian.
- Nagłówek rozmowy z tytułem i znacznikiem „Claude pracuje” w trakcie pracy asystenta.
- Twoje wiadomości w beżowym dymku jak w Claude; odpowiedzi asystenta z ikoną aplikacji, w szerokości wygodnej do czytania.
- Pole wiadomości jako jedna karta: tekst, przycisk wklejania, model i tryb zatwierdzania oraz okrągłe przyciski wysyłania i przerwania.
- Lista kroków jako karta: rozwinięta w trakcie pracy, po zakończeniu zwinięta do jednej linii (kliknięcie nagłówka rozwija).
- Ekran nowej rozmowy z logo i podpowiedzią, co można zrobić.
- Ustawienia jako białe karty z cieniem; Changelog i Informacje z marginesami strony.

Ustawienia:

- Neutralne wartości domyślne, używane tylko wtedy, gdy nie ma jeszcze ustawień (istniejących instalator ani aplikacja nie zmieniają): serwer bazy, nazwa bazy, konto EA i hasła puste (przy serwerze szara podpowiedź „np. 127.0.0.1”), port 5432, model EA „Model”, Max tokenów 10000.
- Ścieżkę do EA.exe można wskazać w oknie wyboru pliku (przycisk „…”), także w instalatorze.
- Blokowanie pakietów przy zapisie jest zawsze włączone; opcja zniknęła z ustawień.
- Gdy EA odmówi zapisu z powodu blokady, komunikat błędu podaje, kto blokuje element, jego pakiet i diagram (konto lub grupa, od kiedy), albo że w modelu nie ma żadnej blokady.

Instalacja:

- Strona „Co nowego” w instalatorze, zaraz po powitaniu: przy aktualizacji zmiany od zainstalowanej wersji, przy nowej instalacji zmiany w bieżącej wersji.
- Pasek postępu instalacji i odinstalowania (procent i bieżący krok) zamiast kręcącego się kółka.

## Wersja 0.1 (2026-10-13)

Pierwsza wersja, kompletna: wszystkie funkcje z listy poniżej są sprawdzone. Samodzielna aplikacja okienkowa do pracy z modelem Enterprise Architect za pomocą Claude Haiku 5.5.

### Działa (sprawdzone)

Model i rozmowa:

- Rozmowa z modelem Claude Haiku 5.5 przez API Anthropic, z obsługą Workspace ID.
- Dopisywanie wiadomości w trakcie pracy asystenta: wiadomość trafia do kolejki i asystent uwzględnia ją przy najbliższym kroku, bez przerywania.
- Lista kroków na czacie w trakcie pracy asystenta: co robi (zapytanie do bazy, zakres zapisu do EA, pobranie diagramu), ile to trwa i jaki był wynik każdego kroku. Lista nie jest zapisywana w historii rozmowy.
- Praca w kilku rozmowach naraz: w trakcie pracy asystenta można zacząć nową rozmowę albo wrócić do innej, a rozpoczęta praca toczy się w tle (pomarańczowa kropka przy nazwie rozmowy w trakcie pracy, zielona po jej zakończeniu, dopóki nie zajrzysz do tej rozmowy). Zapisy do EA z różnych rozmów wykonują się po kolei.
- Przycisk „Przerwij”: zatrzymuje pracę po bieżącym kroku; zaplanowane kolejne kroki nie są wykonywane.
- Tryby zatwierdzania zapisu: ask (panel z opisem zmian i przyciskami „Zatwierdź” / „Odrzuć” przed każdym zapisem; odrzucony zapis nie zmienia modelu), plan (jedno pytanie przed pierwszym zapisem w odpowiedzi, kolejne zapisy tej odpowiedzi już bez pytania) i auto (bez pytania).
- Asystent zna aplikację, w której działa: o swoich funkcjach i ograniczeniach odpowiada na podstawie tego changelogu.
- Asystent wskazuje obiekty modelu nazwą, pakietem i GUID, bez wewnętrznych numerów z bazy.

Enterprise Architect:

- Odczyt modelu EA z bazy PostgreSQL (pakiety, elementy, diagramy, połączenia). Sesja bazy jest tylko do odczytu. Port bazy inny niż 5432 działa także przy zapisie przez EA.
- Zapis do modelu EA przez COM: asystent tworzy pakiety, diagramy, elementy i połączenia oraz rozmieszcza elementy na diagramie. Loguje się na konto EA z ustawień i po zapisie zdejmuje blokady, żeby wynik dało się od razu edytować.
- Stała sesja EA dla całej aplikacji: pierwsze wywołanie EA trwa kilka do kilkunastu sekund (start EA i otwarcie modelu), kolejne poniżej sekundy. Aplikacja komunikuje się z EA bezpośrednio przez COM, bez PowerShella. Sesja zamyka się po 10 minutach bezczynności i przy zamknięciu aplikacji.
- Rysowanie przez asystenta na bieżąco, bez osobnych importerów: z pliku BPMN (elementy na pozycjach z pliku), ze zdjęcia, ze szkicu i z samego opisu.
- Poprawne typy BPMN 2.0 w EA: zadania, podprocesy, zdarzenia, bramki i przepływy, ze stereotypami i tagami.
- Przenoszenie istniejących elementów, diagramów i pakietów do innego pakietu; połączenia zostają.
- Usuwanie z modelu: elementów, połączeń, diagramów i pakietów oraz zdejmowanie elementu z diagramu.
- Kolory wypełnienia: domyślny kolor elementu oraz kolor elementu na wybranym diagramie.
- Komórki nawigacyjne (navigation cells): kafelki-linki do diagramów na diagramie startowym, z podpisem (nazwa diagramu docelowego) i ikoną dobraną do jego typu.
- Pobranie diagramu z EA jako obrazka PNG (tak, jak rysuje go EA) i pokazanie go na czacie, z przyciskami „Zapisz jako…” i „Otwórz w pełnym rozmiarze”.
- Optimistic locking zapisu: asystent może wskazać obiekty, które nie powinny się zmienić od chwili jego odczytu; jeśli się zmieniły, zapis jest odrzucany.

Załączniki:

- Przeciąganie plików do okna, z Eksploratora i z przeglądarki (pobrane pliki): do 5 plików, do 10 MB każdy.
- Wklejanie ze schowka jako załącznik: obraz (np. zrzut ekranu) albo pliki skopiowane w Eksploratorze, skrótem Ctrl+V lub przyciskiem przy polu wiadomości.
- PDF i obrazy trafiają do modelu Claude bezpośrednio, DOCX jako tekst, pliki tekstowe (BPMN, XML, CSV, kod…) jako tekst.
- ZIP jest rozpakowywany: obrazy, PDF i pliki tekstowe z jego wnętrza trafiają do modelu Claude tak, jakby były załączone osobno (do 20 plików).
- Załącznik, którego pliku nie ma już na dysku, zostawia w rozmowie wzmiankę i nie blokuje dalszej pracy w tej rozmowie.

Okno i czat:

- Wielowierszowe pole wiadomości: długi tekst się zawija, Enter wysyła, Shift+Enter wstawia nową linię.
- Strzałka w górę w pustym polu wiadomości przywołuje ostatnią własną wiadomość z rozmowy; kolejne naciśnięcia cofają się do wcześniejszych, strzałka w dół wraca.
- Sprawdzanie pisowni w polu wiadomości słownikiem systemu Windows, w języku z ustawień systemu: błędne słowa są podkreślane na czerwono.
- Formatowanie odpowiedzi asystenta na czacie: pogrubienia, listy punktowane i numerowane, tabele, nagłówki, kod w linii i bloki kodu.
- Godzina przy każdej wiadomości na czacie (dla starszych wiadomości także data).
- Otwarta rozmowa pokazuje się od końca, czyli od ostatnich wiadomości.
- Kółko ze strzałką nad polem wiadomości, gdy czat jest przewinięty wyżej: przenosi na koniec rozmowy.
- Wyszukiwanie w rozmowach: po tytule i po treści wiadomości.
- Zmiana nazwy rozmowy: ołówek przy wybranej rozmowie, Enter zapisuje, Esc anuluje.
- Historia rozmów zapisywana w plikach i powrót do wcześniejszych rozmów.
- Okno pozostaje aktywne w trakcie pracy asystenta (praca idzie w tle).
- Stały rozmiar okna przy przenoszeniu między monitorami o różnym skalowaniu.
- Jasna kolorystyka okna, zakładki „Changelog” i „Informacje”, duże logo aplikacji w zakładce „Informacje”.
- Ekran powitalny z grafiką z wizytówki Loonar (globus, klawiatura, gwiazdki), nazwą i wersją aplikacji oraz danymi właściciela.
- Uruchamianie bez okna terminala, pasek z danymi właściciela.
- Ikona aplikacji w pliku exe: widoczna w Eksploratorze i po przypięciu do paska zadań.

Ustawienia:

- Ustawienia w oknie w czterech obramowanych grupach: klucz API, baza modelu, konto EA, tryb zatwierdzania. Klucz API i hasło konta EA są w Menedżerze poświadczeń Windows. Model Claude jest pokazany tylko do odczytu.
- Zapis każdej grupy osobno (przyciski „Zapisz i sprawdź…”), z wynikiem testu pod grupą: klucz API (zapytanie do modelu), baza (połączenie i odczyt), EA (logowanie przez COM na podane konto).

Instalacja:

- Instalator w postaci kreatora: licencja, folder docelowy (domyślnie Program Files), skróty (menu Start, pulpit, szybkie uruchamianie), opcjonalna konfiguracja (klucz API, baza, konto EA, tryb zatwierdzania) i podsumowanie.
- Aktualizacja przez ponowne uruchomienie instalatora: ustawienia, historia rozmów i załączniki zostają.
- Odinstalowanie z menu Start albo z ustawień Windows; dane użytkownika i sekrety są usuwane tylko na życzenie.
- Ustawienia, historia rozmów i załączniki są w profilu użytkownika (`%APPDATA%\Loonar EA Assistant`), osobno dla każdego użytkownika komputera.
- Instrukcja użytkownika (PDF) w folderze instalacji i w menu Start.

### Warunki konieczne

- Model EA jest plikiem projektu .qea albo .qeax albo jest w bazie PostgreSQL. Starsze formaty plików (.eap, .eapx, .feap) i inne bazy nie są obsługiwane; plik .eap/.eapx EA przenosi do .qea transferem projektu.
- Model w bazie musi mieć włączone zabezpieczenia użytkowników (Enable Security, tryb „Require User Lock to Edit”) i konto użytkownika dla asystenta (dowolna nazwa), z hasłem i uprawnieniami do edycji.
- W pliku modelu zabezpieczenia użytkowników są opcjonalne; gdy są włączone, plik też potrzebuje takiego konta.

### Znane ograniczenia

- Pierwsze wywołanie EA po uruchomieniu aplikacji (albo po 10 minutach przerwy) trwa kilka do kilkunastu sekund. W tym czasie w tle działa proces EA, który zajmuje jedno miejsce w licencji.
- Duże diagramy asystent zapisuje partiami; przy bardzo długiej odpowiedzi może zabraknąć limitu tokenów (ustawienie „Max tokenów”).
- Gdy jedna z operacji zapisu się nie uda, wcześniejsze operacje z tego samego zapisu zostają w modelu; komunikat podaje, co zostało zapisane.
- Kursywa w odpowiedziach asystenta nie jest formatowana.
- Na monitorze o innym skalowaniu niż główny, obraz okna jest nieco mniej ostry.
- Gdy okno EA nie jest otwarte albo pokazuje inny model, zmiany zapisane przez asystenta widać po odświeżeniu pakietu (F5).
- Proxy ze skryptem PAC oraz uwierzytelnianie NTLM/Kerberos na proxy nie są obsługiwane.
- Szyfrowanie TLS w ustawieniach dotyczy odczytu modelu przez asystenta; połączenie samego EA z bazą zależy od ustawień EA.
- Instalator nie jest jeszcze podpisany cyfrowo, więc Windows SmartScreen może ostrzegać przy pierwszym uruchomieniu.
