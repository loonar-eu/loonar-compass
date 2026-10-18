# Changelog

Wszystkie istotne zmiany w aplikacji Loonar Compass (do wersji 0.3 pod nazwą Loonar EA Assistant).

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
