# Strona loonar.eu – Loonar Compass

Statyczna strona w 4 językach (EN, PL, DE, FR), hostowana bezpłatnie na **GitHub Pages** (HTTPS z certyfikatem
GitHub) pod domeną **loonar.eu**, publikowana z publicznego repozytorium **github.com/loonar-eu/loonar-compass**
przez GitHub Actions. Bez PHP, bez cookies, bez zewnętrznych skryptów i czcionek. Statystyki: GoatCounter
(bez cookies, bez danych osobowych). Instalator jest pobierany z **GitHub Releases**.

**Wszystkie pliki HTML, `sitemap.xml`, `robots.txt`, `CNAME`, `.nojekyll` i kopie licencji są generowane**
skryptem `website_src\build.py`. Nie poprawiaj ich ręcznie – zmiana zniknie przy następnym generowaniu.
Ręcznie zmieniasz tylko `assets/style.css` i `assets/site.js` (potem podnieś `ASSET_V` w `build.py`).

## Zawartość folderu

```
website/
├── index.html, pl/, de/, fr/        strona główna (EN pod https://loonar.eu/, PL pod /pl/ …)   – generowane
├── privacy/, pl/privacy/ …          polityka prywatności w 4 językach                          – generowane
├── legal/, pl/legal/ …              nota prawna / Impressum / mentions légales                 – generowane
├── 404.html                         strona „nie znaleziono” (4 języki; GitHub Pages pokazuje ją sam) – generowana
├── sitemap.xml, robots.txt          mapa strony z wersjami językowymi (hreflang)               – generowane
├── CNAME, .nojekyll                 domena loonar.eu; wyłączenie Jekylla                       – generowane
├── favicon.svg, favicon-32.png, favicon.ico, apple-touch-icon.png
├── assets/
│   ├── style.css                    wygląd strony (ręcznie)
│   ├── site.js                      animacje, język, statystyki GoatCounter, przełącznik „nie licz mnie” (ręcznie)
│   ├── license-en|pl|de|fr.txt      kopie LICENSE*.txt z katalogu projektu                      – generowane
│   └── img/                         logo (SVG), og-image.png (obrazek do udostępniania), author.jpg (zdjęcie w „O mnie”)
└── README.md                        ten plik (workflow nie publikuje go na stronie)
```

## Generowanie strony

Źródła w `website_src\` (publikowany jest tylko folder `website`):

- `content.py` – wszystkie teksty strony głównej i 404 w 4 językach (z FAQ, sekcjami „Wesprzyj” i „O mnie”),
- `legal.py` – polityka prywatności i nota prawna (polski tekst jest wiążący co do treści),
- `build.py` – szablon HTML, dane wydania, adresy usług, generowanie i **kontrola** wszystkich stron,
- `assets.py` – obrazek do udostępniania `og-image.png`; z parametrem `icons` także logo i favikony (wymaga Pillow).

```
python -I -B website_src\build.py
python -I -B website_src\assets.py     (tylko po zmianie nazwy/hasła na obrazku)
```

`-I` – tryb izolowany (skrypt sam dodaje swój folder do ścieżki importu), `-B` – bez `__pycache__`.
Po wygenerowaniu `build.py` sprawdza każdą stronę: domknięcie znaczników, unikalne `id`, czy istnieją kotwice
i pliki, do których prowadzą linki (także `../#faq` między stronami), i czy bloki JSON-LD się parsują.
Błąd = kod wyjścia 1 i lista problemów. Francuskie wąskie spacje przed `: ; ? !` i w « » skrypt wstawia sam.

Teksty muszą używać dokładnych nazw z interfejsu aplikacji (`i18n\strings.src`, format `angielski ‖ polski ‖
niemiecki ‖ francuski`). Przy zmianach w aplikacji sprawdź FAQ ponownie – zwłaszcza: instalacja (dla mnie / dla
wszystkich), szyfrowanie bazy, proxy, Menedżer poświadczeń, opcje deinstalatora, podpis instalatora (SmartScreen).

## Publikacja na GitHub Pages

Workflow `.github/workflows/pages.yml` (w katalogu repozytorium) publikuje folder `website` przy każdym
pushu na `main`, który zmienia `website/**` (albo sam workflow), oraz ręcznie (Actions → „Website (GitHub Pages)”
→ „Run workflow”). Uprawnienia: `contents: read` (checkout), `pages: write`, `id-token: write`.
Przed wysłaniem usuwa z paczki `README.md`; plik `.nojekyll` nie trafia do paczki (pliki z kropką są pomijane),
co niczego nie zmienia – przy publikacji przez Actions Jekyll i tak nie działa.

### Jednorazowa konfiguracja (w tej kolejności)

1. **Weryfikacja domeny w ustawieniach konta/organizacji `loonar-eu`** (zanim dodasz domenę w repozytorium –
   chroni przed przejęciem domeny): GitHub → zdjęcie profilu → Settings (dla organizacji: organizacja →
   Settings) → **Pages** → „Add a domain” → `loonar.eu`. GitHub poda rekord TXT, np.
   `_github-pages-challenge-loonar-eu.loonar.eu` z kodem. Dodaj go w panelu DNS home.pl (nazwa hosta:
   `_github-pages-challenge-loonar-eu`), poczekaj (do 24 h) i kliknij „Verify”. Rekordu TXT nie usuwaj.
2. **Repozytorium → Settings → Pages → Build and deployment → Source: „GitHub Actions”.**
3. **Repozytorium → Settings → Pages → Custom domain:** wpisz `loonar.eu` i zapisz. (Przy publikacji przez Actions
   liczy się to ustawienie; plik `CNAME` w `website` jest ignorowany – zostaje jako informacja.)
4. **Rekordy DNS w panelu home.pl** (Domeny → loonar.eu → Strefa DNS). Usuń dotychczasowe rekordy A/AAAA
   domeny `loonar.eu` i rekord dla `www` (wskazywały hosting home.pl), potem dodaj:

   | Nazwa         | Typ   | Wartość                 |
   |---------------|-------|-------------------------|
   | `loonar.eu`   | A     | `185.199.108.153`       |
   | `loonar.eu`   | A     | `185.199.109.153`       |
   | `loonar.eu`   | A     | `185.199.110.153`       |
   | `loonar.eu`   | A     | `185.199.111.153`       |
   | `loonar.eu`   | AAAA  | `2606:50c0:8000::153`   |
   | `loonar.eu`   | AAAA  | `2606:50c0:8001::153`   |
   | `loonar.eu`   | AAAA  | `2606:50c0:8002::153`   |
   | `loonar.eu`   | AAAA  | `2606:50c0:8003::153`   |
   | `www`         | CNAME | `loonar-eu.github.io.`  |

   **Nie ruszaj rekordów MX, SPF/DKIM (TXT) i autodiscover** – poczta `mariusz@loonar.eu` zostaje w home.pl.
   Nie dodawaj rekordów wildcard (`*.loonar.eu`). Jeśli strefa ma rekord CAA, musi dopuszczać `letsencrypt.org`.
   GitHub sam przekierowuje `www.loonar.eu` → `loonar.eu`.
5. Gdy DNS zadziała (Settings → Pages pokaże „DNS check successful”, certyfikat może potrwać do kilkudziesięciu
   minut), zaznacz **„Enforce HTTPS”**. Jeśli certyfikat się nie pojawia: usuń domenę, zapisz, wpisz ponownie.
6. Sprawdź: `https://loonar.eu/`, `/pl/`, `/de/`, `/fr/`, `/privacy/`, `/legal/`, nieistniejący adres (404),
   przycisk pobierania, `https://www.loonar.eu/` (przekierowanie).

Wartości DNS są z dokumentacji GitHub („Managing a custom domain for your GitHub Pages site”, stan 2026-10-17).

### Bezpieczeństwo nagłówków

GitHub Pages nie pozwala ustawiać własnych nagłówków HTTP (dawny `.htaccess` usunięto). Content-Security-Policy
jest więc w `<meta http-equiv>` na każdej stronie (generuje ją `csp()` w `build.py`): tylko własne pliki, obrazy
`data:` (ikony w CSS) i adres licznika GoatCounter. Meta-CSP nie obsługuje `frame-ancestors` – strony nie mają
formularzy ani akcji, więc osadzenie w ramce nic nie daje. Pliki dostają cache ok. 10 minut (ustawienie GitHub);
odwołania do CSS/JS mają `?v=ASSET_V`.

## Statystyki – GoatCounter

1. Załóż konto na https://www.goatcounter.com/ z kodem strony **`loonar`** (panel: https://loonar.goatcounter.com).
   Kod jest w jednej stałej `GOATCOUNTER` w `build.py` (`""` wyłącza statystyki i usuwa adres z CSP).
2. W ustawieniach GoatCounter warto dopisać własne wizyty do ignorowanych albo użyć na stronie polityki
   prywatności przycisku „Nie licz moich odwiedzin w tej przeglądarce” (zapis `loonar-nocount` w localStorage).

Jak to działa: `site.js` nie ładuje `count.js`, tylko wysyła żądanie do udokumentowanego punktu `/count`
(`navigator.sendBeacon`, a gdy go brak – obrazek), więc kliknięcia nie opóźniają przejścia na inną stronę.
Nic nie jest liczone bez JavaScriptu, na `localhost`/adresach prywatnych/`file://`, w ramce ani po wyłączeniu
przyciskiem. Do Not Track/GPC nie jest uwzględniane (dokumentacja GoatCounter tego nie zaleca); można to
włączyć stałą `HONOR_DNT` w `site.js` – wtedy dopisz to do polityki prywatności.

Co widać w panelu:

| Pozycja        | Znaczenie                                                                            |
|----------------|--------------------------------------------------------------------------------------|
| `/`, `/privacy/`, `/legal/` | odsłony stron angielskich (bez prefiksu = EN)                           |
| `/pl/…`, `/de/…`, `/fr/…`   | odsłony wersji językowych                                               |
| `/404`         | wejście na nieistniejący adres; tytuł = brakujący adres                              |
| `download`     | kliknięcie „Pobierz instalator” (tytuł = strona, z której kliknięto, np. `/pl/`)      |
| `github`       | kliknięcie dowolnego linku do github.com (ikona w nagłówku, kod źródłowy, wydania)   |
| `support`      | kliknięcie przycisku „Wesprzyj na suppi.pl” (lub innego linku do suppi.pl)           |
| `privacy`, `legal` | kliknięcie linków w stopce                                                       |

**Liczba pobrań:** zdarzenie `download` liczy kliknięcia na stronie; faktyczne pobrania pliku pokazuje GitHub
(licznik `download_count` każdego pliku wydania, np. `gh release view v0.3.0 --repo loonar-eu/loonar-compass
--json assets` albo API `GET /repos/loonar-eu/loonar-compass/releases`). Obejmuje też pobrania spoza strony.

## Nowa wersja

Wszystko, co dotyczy wydania, jest na początku `website_src\build.py`:

| Stała       | Co to jest                                                         | Teraz |
|-------------|--------------------------------------------------------------------|-------|
| `VER`       | numer wersji (karta pobierania, hero, dane strukturalne)           | `0.3` |
| `SETUP_URL` | **jedyny link do instalatora** – plik w GitHub Release; z niego brana jest nazwa pliku i adres repozytorium | `…/releases/download/v0.3.0/LoonarCompass-Setup-0.3.exe` |
| `RELEASE`, `DATE` | data wydania (ISO i w 4 językach)                            | `2026-10-17` |
| `SHA256`    | suma kontrolna instalatora pokazywana pod przyciskiem              | patrz TODO na górze |
| `SIZE_MB`   | rozmiar w MB – używany tylko, gdy instalatora nie ma w `dist\`     | `28` |

Kroki:

1. Zbuduj instalator (`build_installer.ps1`), policz sumę: `Get-FileHash dist\LoonarCompass-Setup-X.Y.exe`.
2. Na GitHubie utwórz wydanie z tagiem `vX.Y.0` i dołącz plik instalatora (nazwa jak w `SETUP_URL`).
3. W `build.py` zmień `VER`, `SETUP_URL`, `RELEASE`, `DATE`, `SHA256`. Rozmiar skrypt odczyta z `dist\`
   i sprawdzi sumę z plikiem (przy niezgodności przerwie pracę i pokaże właściwą wartość).
4. Sprawdź w `content.py` teksty zależne od wersji (np. FAQ o SmartScreen – po podpisaniu instalatora zmień je).
5. Wygeneruj stronę, zrób commit i push na `main` – workflow opublikuje stronę.

Po zmianie `style.css` lub `site.js` podnieś `ASSET_V` w `build.py` (teraz `3`).

## Język strony

Wersja domyślna to angielska (`/`); przełącznik EN/PL/DE/FR jest w nagłówku i stopce każdej strony
(na stronach prawnych prowadzi do tej samej strony w innym języku). Przy pierwszej wizycie na `/` `site.js`
proponuje (nie wymusza) język przeglądarki; wybór zapamiętuje w localStorage (`loonar-lang`).

## Sekcja „O mnie” i licencja

Zdjęcie: `assets/img/author.jpg` (400×400, pokazywane w kole). Bez pliku strona pokazuje neutralną sylwetkę.
Teksty: klucze `ab_*` w `content.py`. Licencję `build.py` kopiuje z `LICENSE*.txt` katalogu projektu
i ostrzega, jeśli zawiera starą nazwę produktu.
