# SQL Scripter

Desktopowa aplikacja **SmCode** (Tauri 2 + Rust + React/Untitled UI), która wyszukuje zmienione w MSSQL procedury, funkcje, widoki i tabele i zapisuje je na dysk jako `schema.nazwa.sql`.

Kontakt / wsparcie: support@sm-code.com · https://sm-code.com

## Funkcje

- **Połączenia** zapisywane lokalnie w SQLite (`connections.db3`). Przy starcie aplikacja sprawdza, czy plik istnieje, i w razie braku tworzy go razem ze schematem.
  - macOS: `~/Library/Application Support/com.sm-code.sqlscripter/connections.db3`
  - Windows: `%APPDATA%\com.sm-code.sqlscripter\connections.db3`
- **Hasła są szyfrowane** (AES-256-GCM, losowy nonce dla każdego zapisu). Klucz 256-bit jest w pęku kluczy systemu: Keychain na macOS, Menedżer poświadczeń na Windows. W `.db3` nie ma jawnego tekstu. Hashowanie nie wchodzi w grę, bo aplikacja musi wysłać do SQL Servera oryginalne hasło. Starszy plik (v0.1, jawne connection stringi) jest przy starcie migrowany, a potem kompaktowany przez `VACUUM` + checkpoint WAL. Dodatkowo włączone jest `secure_delete`.
- **Formularz połączenia** ma osobne pola: serwer (także `HOST\INSTANCJA`), port, baza, uwierzytelnianie SQL lub Windows, użytkownik, hasło, Encrypt i TrustServerCertificate. Connection string jest tylko do odczytu, z zamaskowanym hasłem, i generuje go backend. Klient TDS jest konfigurowany bezpośrednio z pól, więc hasła ze znakami `;` czy `"` działają bez problemu.
- **Język PL/EN** zapisany w tabeli `settings` w `connections.db3`. Przy pierwszym uruchomieniu wybierany jest język systemu. Komunikaty błędów z backendu też są tłumaczone.
- **Branding SmCode**: logo, kolory z logo (czerń + limonka `#7DCF09`), okno „O programie” z kontaktem.
- **Lewy panel**: przewijana lista połączeń z wyszukiwarką, dodawaniem, edycją i usuwaniem. Połączenie otwierasz przyciskiem „Połącz” albo dwuklikiem.
- **Karty**: każde połączenie otwiera się w osobnej karcie, więc możesz pracować na wielu bazach jednocześnie. Ponowne „Połącz” przełącza na kartę, która jest już otwarta.
- **Filtry**: wybór typów obiektów (Wszystko / Procedury / Funkcje / Widoki / Tabele), zakres dat `modify_date` z szybkimi presetami oraz filtr po nazwie.
- **Wyniki**: tabela z zaznaczaniem pojedynczych wierszy lub wszystkich, sortowaniem i podglądem skryptu (ikona oka).
- **Eksport**: wybierasz folder i zapisujesz pliki `schema.nazwa.sql` w UTF-8 z BOM i z końcami linii CRLF, żeby SSMS poprawnie pokazywał polskie znaki. Aplikacja zapamiętuje ostatni folder dla każdego połączenia.

## Co jest w skryptach

| Obiekt | Skrypt |
|---|---|
| Procedura / funkcja / widok | `SET ANSI_NULLS` + `SET QUOTED_IDENTIFIER` (wartości z `sys.sql_modules`), potem definicja, w której wiodące `CREATE` zostaje zamienione na **`CREATE OR ALTER`**. Komentarze przed `CREATE` (też zagnieżdżone `/* */`) zostają zachowane. |
| Tabela | **Skrypt idempotentny** („create or update”, bo SQL Server nie ma `CREATE OR ALTER TABLE`): <br>1. `IF OBJECT_ID(...) IS NULL` → `CREATE TABLE` z kluczem głównym, <br>2. dla każdej kolumny `IF COL_LENGTH(...) IS NULL` → `ALTER TABLE ... ADD`, <br>3. PK, UNIQUE, indeksy (z `INCLUDE` i `WHERE`), CHECK i FK dodawane, jeśli nie ma ich po nazwie. |

Skrypt tabeli **niczego nie usuwa** i **nie zmienia typu istniejących kolumn**. To celowe, bo bez porównania z bazą docelową nie da się bezpiecznie wygenerować `ALTER COLUMN` / `DROP`. Kolumna `NOT NULL` bez `DEFAULT` jest oznaczona w skrypcie komentarzem `WARNING`.

## Wymagania

- Rust stable (edition 2024), Node 22+
- SQL Server **2017+**, bo skrypty tabel używają `STRING_AGG`. `CREATE OR ALTER` wymaga 2016 SP1+.
- macOS: Xcode Command Line Tools. Windows: Microsoft C++ Build Tools + WebView2 (jest w Windows 10/11).

## Uruchomienie

```bash
npm install
npm run app:dev          # aplikacja Tauri z hot-reloadem
npm run dev              # sam frontend w przeglądarce z mockiem backendu (src/lib/dev-mocks.ts)
cargo test -p scripter-core
```

## Instalatory (macOS i Windows)

Tauri buduje instalator dla systemu, na którym uruchamiasz build:

```bash
npm run app:build
```

- **macOS** → `target/release/bundle/dmg/SQL Scripter_0.2.0_aarch64.dmg` oraz `.app`. Na Intel/Universal: `npm run tauri build -- --target universal-apple-darwin` (wymaga `rustup target add x86_64-apple-darwin aarch64-apple-darwin`).
- **Windows** → `target\release\bundle\msi\*.msi` oraz `target\release\bundle\nsis\*-setup.exe`. Instalator NSIS instaluje dla bieżącego użytkownika (bez uprawnień admina) i jest po polsku.

Oba systemy naraz zbudujesz w GitHub Actions: `.github/workflows/release.yml` uruchamia się ręcznie albo po tagu `v*` i tworzy draft release z `.dmg` (ARM + Intel), `.msi` i `.exe`.

Bez podpisu cyfrowego macOS pokaże ostrzeżenie Gatekeepera (obejście: prawy klik → Otwórz albo `xattr -cr "/Applications/SQL Scripter.app"`), a Windows ostrzeżenie SmartScreen. Sekrety do podpisu i notaryzacji są opisane w komentarzu w workflow.

## Połączenie

Instancje nazwane (`HOST\SQLEXPRESS`) są rozwiązywane przez usługę SQL Browser (UDP 1434). Uwierzytelnianie Windows działa tylko w aplikacji uruchomionej na Windows.

## Architektura

```
crates/scripter-core/     # czysty Rust, bez Tauri – testowalny
  store.rs                # SQLite (.db3): tworzenie pliku, migracje v1→v2, CRUD, tabela settings
  crypto.rs               # AES-256-GCM dla haseł
  connection.rs           # pola połączenia → tiberius Config + connection string do podglądu
  i18n.rs                 # język komunikatów backendu
  mssql.rs                # tiberius: connect (timeout, SQL Browser, Azure redirect), zapytania do sys.*
  scripting.rs            # generowanie skryptów (CREATE OR ALTER, skrypt tabeli) – testy jednostkowe
  export.rs               # zapis plików schema.nazwa.sql
src-tauri/
  keychain.rs             # klucz szyfrowania w Keychain / Credential Manager
  state.rs                # mapa otwartych sesji: connection_id -> klient MSSQL (wiele baz naraz)
  commands.rs             # komendy Tauri; auto-reconnect po zerwanym połączeniu
src/
  i18n/                   # słowniki pl.ts / en.ts (en typowany kluczami pl – brak tłumaczenia = błąd kompilacji)
  features/app/           # ustawienia, O programie
  features/connections/   # lewy panel, dialog dodawania/edycji, potwierdzenie usunięcia
  features/workspace/     # karty, filtry, tabela wyników, podgląd skryptu
  components/             # komponenty Untitled UI React (MIT) skopiowane z github.com/untitleduico/react
```

## Znane ograniczenia / TODO

- **Build debug** (`cargo build`, `tauri dev`, debugger) nie używa Keychain: klucz jest w pliku `dev-encryption.key` w katalogu danych aplikacji, a połączenia w osobnym `connections-dev.db3`. Dzięki temu macOS nie pyta o dostęp przy każdej przebudowie. Build release używa Keychain / Menedżera poświadczeń.
- Klucz z Keychain jest wczytywany dopiero przy pierwszym użyciu (zapis hasła, połączenie). Jeśli odmówisz dostępu, aplikacja pokaże komunikat i spróbuje ponownie przy następnej akcji. Niepodpisana aplikacja może pytać po każdej aktualizacji – wybierz „Zawsze zezwalaj”.
- Jeśli klucz w pęku kluczy zostanie usunięty, zapisanych haseł nie da się odszyfrować. Aplikacja wtedy prosi o ponowne wpisanie hasła w edycji połączenia.
- `modify_date` tabeli zmienia się też po zmianie indeksu, constraintu czy triggera.
- Definicja w `sys.sql_modules` po `sp_rename` nadal zawiera **starą nazwę**.
- `CREATE OR ALTER` nie zmieni rodzaju funkcji (np. skalarnej na TVF). W takim przypadku trzeba zrobić `DROP` + `CREATE`.
- Brak w skryptach: triggery, sekwencje, typy, synonimy, uprawnienia, partycjonowanie i kompresja. Można je dodać jako kolejne `ObjectKind`.
