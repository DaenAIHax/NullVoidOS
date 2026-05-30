# Bootstrap Changelog

All notable changes to the NullVoidOS `lfs-bootstrap` direction.

Format adapted from [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Dates are ISO 8601 (YYYY-MM-DD). Entries reference commit hashes when
applicable.

## [Unreleased]

### Nullang self-host: codegen Wave 4 — scope di variabili (2026-05-30)

Chiuso il limite di Wave 3: gli `id`-variabile ora si risolvono al loro tipo
reale invece del default `Int`. Aggiunto un **ambiente di tipi** (`type
Binding = { nametk, tc }`, lista piatta per funzione): `emit_fn` lo pre-carica
coi parametri, `emit_block` lo estende a ogni `let` (sfruttato a riferimento,
`env_push` muta in modo condiviso). `type_of_expr` ora risolve un `id` via
`env_lookup` (confronto per LESSEMA — token diversi, stesso nome). `env`
filato attraverso tutte le `emit_*`.

Conseguenze: `let y = x` dove `x` è String emette `const char* nlu_y = nlu_x;`
(prima sarebbe stato `long`, errore C); e `a == b` fra DUE variabili String dà
`strcmp` (prima serviva un literal da un lato). Prova: `fn same(a: String, b:
String) { … if a == b … }` + `let x = greet("X"); let y = x;` → codegen-Nullang
→ gcc -Wall (zero warning) → stampa `ciao X` / `1` / `0`. W1–W3 invariati,
self-ingest 120/120 item zero spuri, effect-check pulito.

Restano le espressioni che devono emettere statement prima di sé — struct
literal (alloca+assegna), list literal, e **if-come-espressione**: stesso
problema (statement-hoisting), prossima onda — più field/index access.

### Nullang self-host: codegen Wave 3 — type-inference + strcmp (2026-05-30)

Il codegen-in-Nullang ora tipa quel che emette, non tutto `long`. Tre pezzi:

- **Return-type nelle firme.** Il parser classifica anche `-> T`
  (`return_type_code`, salvato in `nd_fn.c`); `classify_type_lex` è ora
  condiviso fra annotazione di parametro e tipo di ritorno. `c_sig` emette
  `const char* nlu_greet(...)` invece di `long`.
- **`type_of_expr`.** Inferenza sufficiente a dichiarare un `let` e scegliere
  `==`/strcmp: literal sul loro tipo, call sul ritorno del builtin
  (`builtin_ret_tc`) o della fn utente (`find_fn_ret`, lookup fra gli item),
  binop → long. `emit_stmt` per `let` usa `c_type(type_of_expr(init))`.
- **`==`/`!=` su String → `strcmp`.** In `emit_expr`, se un operando è di tipo
  String l'uguaglianza diventa `(strcmp(a, b) == 0)` invece del confronto di
  puntatori — e il literal da un lato del confronto basta a farlo scattare.

Limite dichiarato (Wave 3): un `id`-variabile in `type_of_expr` torna `Int`
(niente scope di variabili ancora) → `let y = x_string` sbaglierebbe; non
capita nei target di prova. `root` filato attraverso tutte le `emit_*` per il
lookup del ritorno delle fn utente.

Prova: `fn greet(name: String) -> String { concat("ciao, ", name) }` +
`let msg = greet("Nullang"); … let first = substr(msg,0,1); if first == "c" {…}`
→ codegen-Nullang emette C tipato → gcc -Wall (zero warning) → stampa
`ciao, Nullang` / `lunghezza = 13` / `inizia per c`. W1/W2 invariati, self-ingest
117/117 item zero spuri, effect-check pulito.

### Nullang self-host: codegen Wave 2 — String + builtin (2026-05-30)

Il codegen-in-Nullang ora emette programmi che STAMPANO. Aggiunto al preludio
(embeddato come string literal multi-riga, verbatim dal codegen Rust) gli
helper `nullang_print`/`concat`/`str_of_int`/`str_len`/`substr`/`char_at`, e in
`emit_expr` il **dispatch dei builtin**: un nome in `is_builtin` diventa
`nullang_<name>` invece di `nlu_<name>`, e per i builtin World-gated
(`world_gated`: print/read_file/write_file) il **primo argomento `World` si
droppa** dalla call (a runtime World è cancellato, vive nella clausola `uses`).

Prova end-to-end: `fn sq(n) { n*n } fn main { print(world, "…"); print(world,
concat("sq(7) = ", str_of_int(sq(7)))); 0 }` → codegen-Nullang emette C → gcc →
ELF che stampa `codegen Nullang: vivo` / `sq(7) = 49`. Verificato anche il
literal multi-riga del preludio: il lexer-in-Nullang lo ri-lessa senza desync,
self-ingest invariato (112/112 item, zero spuri), effect-check pulito.

Restano: type-inference per `let`-String e return-type String nelle firme
(oggi i `let` sono sempre `long`), `==` su String → `strcmp`, poi List+struct.

### Nullang self-host: il codegen — Wave 1, AST→C→ELF da Nullang (2026-05-30)

Il terzo e ultimo stadio del compilatore comincia a esistere in Nullang.
`selfhost-parser.null` (ormai il compilatore-in-Nullang: lexer+parser+codegen)
cammina l'arena del parser ed emette C. **Wave 1 = sottoinsieme Int-only**:
aritmetica, `let`/`while`/`assign`, chiamate utente, `main` con `World`
cancellato. Mangling come il codegen Rust (`nlu_` su fn/parametri/locali,
`main` resta `main`), forward-declaration prima dei corpi, ogni binop parato
da parentesi (le precedenze sono già nell'albero), l'espressione di coda di un
corpo-blocco diventa `return`.

Prerequisito caduto: **il parser non scarta più i tipi**. `parse_param` ora
classifica l'annotazione (`type_code_after_colon`) in un codice — Int/Bool/
String/World/List/altro — abbastanza perché il codegen mappi su C e cancelli
`World`. (AST di tipi completo ancora differito.)

Prova end-to-end: dato `fn fact(n) { …while… } fn main { fact(5) }`, il
codegen-Nullang emette C valido, `write_file` lo scrive, **gcc lo compila e
l'ELF esce con codice 120 = fact(5)**. Il giro AST→C→ELF è chiuso e ogni
stadio sopra gcc è autorato in Nullang. Self-ingest invariato (110/110 item,
zero spuri), effect-check pulito.

Restano le onde: String+builtin (`print`/`concat`/`str_*`), List+struct
(box/unbox, `nlf_` field mangling, typedef), if-come-espressione (lowering a
temp+if-statement), preludio completo — fino a quando il codegen compila il
compilatore stesso (fixpoint stage0→stage2, allora Rust è togliibile).

### Nullang self-host: il parser mangia la propria coda (2026-05-30)

`selfhost-parser.null` ora parsa l'INTERO proprio sorgente. Prima gestiva
solo espressioni e statement; mancava la grammatica di cui è fatto il file
stesso. Aggiunto:

- **Dichiarazioni top-level.** `parse_fn` (`fn nome(params) -> T uses !cap
  { corpo }`), `parse_type_decl` (`type Nome = { campi };`), con
  `parse_params`/`parse_param` e `parse_type_fields`/`parse_type_field`.
  Le annotazioni di tipo (`: T`, `-> T`, anche generiche `List<Token>`) e
  le clausole d'effetto `uses` si saltano in modo robusto (`skip_type` con
  profondità `<...>`, `skip_anno`, `skip_return_type`, `skip_uses`): sono
  affare di un'altra fase, il nome e la forma restano. `parse_program` ora
  dispatcha su `parse_item` (fn / type / statement).
- **`if` come espressione.** Il muro che bloccava il self-ingest: in Nullang
  `let x = if c { a } else { b }` è legale, ma `parse_primary` trattava `if`
  come un identificatore → desync. Estratto `parse_if`, condiviso fra
  statement-context e primary-expression context. (Trovato bisezionando il
  desync su `scan_string`, isolato a `let endpos = if i < n {…} else {…}`.)
- **Driver `ingest`.** Legge il proprio file via `read_file` (`uses
  !fs.read`), lo parsa, stampa l'indice degli item recuperati.

Prova: sul proprio sorgente (34709 byte, 8001 token) costruisce 4462 nodi
AST e recupera **91 item top-level = 91 dichiarazioni reali, zero spuri,
zero nodi `?`**. Effect-check pulito. Stesso passo del lexer, un livello
più su: il cuore ricorsivo del compilatore ora si dà una forma ad albero
da solo. Resta il codegen (Wave self-host successiva).

### `bootstrap/LANDSCAPE.md` — analisi del paesaggio competitivo (2026-05-30)

Materiale per blog/paper. Smonta NullVoidOS in 4 layer e per ognuno mappa
chi lo persegue (Zero/Vercel, Pel, SICA, Darwin Gödel Machine, Voyager,
Sandlock, AIOS), con fonti citate. Verdetto: ogni mattone esiste già; il
contributo proprio è l'integrazione verticale dei 4 layer come una cosa
sola, non l'invenzione dei singoli pezzi.

### DESIGN: interfaccia agentica a voce (fast/slow) — visione, non task corrente (2026-05-30)

Nuova sezione `DESIGN.md` "Layer 4 vision — voice-orchestrated agentic
interface (fast/slow)", dopo "Deferred: graphical UI". Provocata dalla
domanda sul robot Reachy Mini (cervello da spostare in locale): una volta
che il cervello è locale (`llama.cpp` già nel substrate), l'interfaccia
human-facing di un OS agent-primario non deve essere né desktop né TTY — può
essere **voce**. Riformula la questione GUI come "un backend agentico, più
frontend come trasporti" (stessa forma del daemon Reachy e del nostro
`agent_backend.send()`). Fissa:

- **Split fast/slow (System-1/System-2):** router LLM piccolo locale sempre
  vivo (tiene il dialogo, decide *quando* scalare) ⇄ Opus worker cloud che
  ragiona. Lo split è strutturale, non un'ottimizzazione: i due profili di
  latenza sono incompatibili in un solo modello. Non un primitivo nuovo —
  due backend pluggable già previsti cablati in topologia fast/slow.
- **Il router è backend, non frontend:** nel momento in cui *decide* è
  backend; metterlo nel frontend spingerebbe la decisione fuori dal confine
  di fiducia. STT/TTS/mic = trasporti dumb non fidati.
- **Tre conseguenze:** terminale = trasporto che entra più in profondità
  (escape hatch ad alta fiducia, bypassa il router); robot = frontend-voce +
  capability fisiche (`!motor`/`!camera`), non un terzo sistema; il confine
  di escalation = confine di supervisione (provenance + grant capability +
  conferma a voce). È lì che smette di essere un assistente vocale e diventa
  feature di OS.
- **Crown jewel rimandato:** il protocollo router↔Opus (cosa si passano,
  stato condiviso, interruzione mid-task) è il contributo vero; vincoli
  precoci: route-not-rewrite e il duplex vivo. Design-in-system, non ora.
- **Compatibilità substrate:** `whisper.cpp` (stesso autore di `llama.cpp`),
  Piper (TTS C++ piccolo), Silero VAD — nessuna dipendenza irriducibile
  nuova. **North star onesta:** cosa aggiunge l'OS vs un'app vocale su Linux
  = solo capability+provenance+supervisione-a-voce sul boundary + coerenza
  "agent-primary senza desktop".

Solo documentazione, zero codice. Layer 4 / Phase 4+, *davanti* al bootstrap
di Phase 0/1 — si costruisce quando il bootstrap respira. Cullis resta la
scommessa commerciale. Scelta utente: "Visione scritta (hobby)".

### Nullang self-host: il parser — discesa ricorsiva in Nullang + due muri caduti (2026-05-30)

`examples/selfhost-parser.null`: il cuore RICORSIVO di un compilatore scritto
in Nullang. Lexer embedded (token) → discesa ricorsiva con precedence-climbing
(Pratt) → AST, provato da un tree-printer su 10 frammenti Nullang reali. La
precedenza esce giusta (`1 + 2 * 3 - 4` → `((1+(2*3))-4)`), il meno unario lega
più stretto del binario, chiamate annidate, letterali-struct, accesso campo
incatenato, indicizzazione, `let mut`, `while`, `if/else` — tutti corretti.

Due muri sono caduti per renderlo possibile, entrambi in questa sessione:

- **Mutua ricorsione (cresce-il-compilatore).** Un parser a discesa ricorsiva
  è un ciclo intrinseco (`parse_expr` ⇄ `parse_unary` ⇄ `parse_postfix` ⇄
  `parse_primary` ⇄ `parse_args` → `parse_expr`): nessun ordine di emissione
  lo soddisfa, e il C rifiuta una chiamata a funzione non ancora dichiarata.
  Il codegen ora **emette le forward-declaration C** di ogni funzione non-`main`
  prima di tutti i corpi. Modifica minima al compilatore Rust, guidata
  strettamente da ciò che il self-host richiede (SPEC §12 tempo 2). Sblocca
  ogni codice Nullang ricorsivo, non solo questo parser. Coperta da test
  (`mutual_recursion_emits_forward_declarations`); suite 72→73 verde.
- **Stato mutabile condiviso senza parametri `mut`.** Il cursore sui token e
  l'arena dei nodi devono mutare attraverso la ricorsione, ma i parametri non
  sono `let mut`. Si sfrutta la **semantica a riferimento**: dentro ogni
  funzione si ri-lega il parametro a un `let mut` locale — alias dello stesso
  handle heap — che rende `push`/field-write legali E condivisi col chiamante.
  Verificato in `examples/probe-mutshare.null` (cursore + List condivisi a 5
  livelli di ricorsione).

Tecniche di design notevoli: **AST ad arena piatta** — niente handle null per i
figli mancanti (enum/Option differiti, §11), i nodi vivono in una `List<Node>`
e si riferiscono per INDICE Int (`-1` = assente); le liste a lunghezza variabile
(argomenti, campi, statement) si incatenano con un campo `next`. **Disciplina
`else` obbligatorio**: in Nullang ogni `if` è un'espressione e pretende `else`,
quindi i "consuma-se-c'è" diventano funzioni `eat_*` con else chiamate come
statement, e il dispatch del printer è una catena if/else annidata. **Flag
`nostruct`**: dentro la condizione di `if`/`while` una `Ident {` è il BLOCCO, non
un letterale-struct — disambiguazione identica a Rust, verificata (`while i < n {`
non cattura `n { … }`). Loop host-dry-run verde (build→cc→run senza VM).
Prossima onda: statement/funzioni completi fino a far mangiare al parser la
propria coda, come il lexer.

### Nullang self-host: il lexer mangia la propria coda (2026-05-30)

`examples/selfhost-lexer.null` da nucleo-su-input-giocattolo a lexer che legge
un sorgente Nullang vero via `read_file` (`!fs.read`) + `argv` e lo tokenizza.
Muri caduti rispetto alla prima sonda:

- **Stringhe** con escape (`\"`, `\\`, `\n`, `\t`) — prima un muro; ora
  `scan_string` salta gli escape e chiude sulla virgoletta giusta. Verificato:
  `"ha#sh and \"quote\" inside"` esce come UN token (il `#` interno non apre
  commento, le virgolette escaped non chiudono).
- **Commenti** `#...` saltati come il whitespace; una virgoletta dentro un
  commento non apre una stringa.
- **Operatori multi-char** con maximal munch (`->`, `==`, `!=`, `||`, `&&`,
  `::`, `<=` …): corsa contigua di char-operatore = un solo token. Prima
  finivano scambiati per identificatori — muro silenzioso, ora chiuso.
- Classi token distinte: number / ident / keyword / punct-strutturale / op /
  string, con istogramma nel resoconto.

Prova self-host: il lexer letto su se stesso emette 1400 token classificati
(48 number, 430 ident, 126 keyword, 455 punct, 271 op, 70 string). Loop
host-dry-run verde (build→cc→run senza VM). Il confine `fs` resta capability:
Landlock confinerebbe la lettura a runtime senza che il lexer ne sappia nulla.
Prossima onda self-host: il parser (recursive-descent in Nullang sul flusso di
token) — nuova caccia ai muri, sessione propria.

### DESIGN: orizzonte "tutto in VM" — visione, non task corrente (2026-05-30)

Nuova sezione `DESIGN.md` "Horizon — everything in the VM", subito dopo il
Trust model. Fissa la direzione (l'end-state di un OS agent-primario: l'agente
fa tutto in VM, host fuori dal loop) **senza costruirla** — coerente con alfa
di ricerca e con la cornice visioni-da-scrivere-non-da-costruire. Registra:
cosa è GIÀ in VM (l'evoluzione del compilatore via rito self-improve), cosa
resta host e perché ognuno è load-bearing (git=nessun accesso GitHub by design
+ contraddirebbe il threat-model dare un credential di push a un agente non
sandboxato; chirurgia parser/typer=blast-radius totale non smoke-probe-able;
floor `/bin/nullang` cotto=rete di rollback), le precondizioni gating (chiudere
le sharp-edge del perimetro, una rete di verifica = self-host, un canale
mediato per gli artefatti) e l'ordine se mai perseguito (self-host → confinare
l'agente → egress mediato; git-in-VM è l'ULTIMO passo, forse mai giusto per
un'alfa mono-utente). Solo documentazione, zero codice.

### Nullang: P1 stdlib — `index_of` + `split` (autorati dall'agente in-VM) (2026-05-30)

Secondo grappolo stdlib, **autorato dall'agente DENTRO la VM** col rito
generation-managed (generation-7, `nv-toolchain-0.1.2`) e mergeato host verbatim
— come `char_at`, ma stavolta due builtin in un colpo. Scelta dell'utente:
questi NON fatti host, lasciati all'agente (il dominio del loop di
auto-miglioramento, `BUILTINS_CONTRACT.md`).

- `index_of(s, sub) -> Int` — byte offset della prima occorrenza di `sub`, `-1`
  se assente, `0` per `sub` vuoto (match-at-start). La "find" di search/replace,
  che il config-parser rifaceva a mano. Scan O(n·m), totale.
- `split(s, sep) -> List<String>` — **primo builtin che PRODUCE una `List<T>`**:
  costruisce la lista col runtime `nl_list` esistente (push di puntatori String
  via `intptr_t`, stesso boxing del codice utente). Chiude il cerchio aperto da
  `List<T>` — il linguaggio ora sa *creare* collezioni, non solo riceverle.
  `sep` vuoto → `[s]` (niente trappola degli infiniti vuoti); separatori
  consecutivi → segmenti vuoti (`split("a,,b", ",")` = `["a","","b"]`). Segmenti
  freschi via `malloc`, come `substr`.

Abilitazione host: l'agente è partito dall'immagine ricostruita all'HEAD
`4fc225f` (List+struct+P0 già baked). Integrazione host: solo le 2 regioni del
contratto (`check.rs::builtins()` + impl C nel PRELUDE), corretti 3 typo nei
*commenti* del diff (codice invariato). Suite **72/72** (3 nuovi). Esempio
`examples/split-index.null` ELF verde (config `k=v` parsato con split+int_of_str,
CSV con campo vuoto, sep vuoto → `[s]`). SPEC §4.7 aggiornato. `Cargo.lock`
invariato. Resta P2: `str_of_bool`/`else if` (ergonomia).

### Nullang: P0 stdlib — `char_code` + `int_of_str` (confine String↔Int) (2026-05-30)

Due builtin puri e totali che chiudono il gap stdlib **confermato 3/3 da due
sonde indipendenti** (il lexer self-host e il config-parser dell'altro agente):
entrambe rifacevano a mano `parse_int` (catena ~22 LOC) ed enumeravano le classi
di carattere perché mancava `char→Int`.

- `char_code(s, i) -> Int` — il byte a indice `i` (0..255), `-1` fuori range. È
  il `char→Int` mancante: le classi di carattere diventano range aritmetici
  (`code >= 48 && code <= 57`) invece di catene `==` a 10 rami. Sentinella `-1`
  (un byte reale è 0..255), non `""` come `char_at` (che collide con NUL).
- `int_of_str(s) -> Int` — parse decimale totale: `-` opzionale, cifre, si ferma
  al primo non-cifra; `""`/spazzatura → `0`. È il gap headline deterministico.
  Variante `Result` (distinguere `"0"` da errore) è il follow-up §10, come
  `read_file`.

Builtin = la fascia normalmente auto-servita dall'agente in-VM; qui fatti host
per scelta esplicita dell'utente (P0 urgenti, nessun boot VM, sbloccano subito le
due sonde). Suite **69/69** (4 nuovi). Esempio `examples/string-int-seam.null`
ELF verde (`8081, -42, 12, 0, 55, digit, upper, -1`). SPEC §4.7 aggiornato.
`Cargo.lock` invariato. Restano: P1 `split`/`index_of` (cercati da entrambe le
sonde), P2 `str_of_bool`/`else if`.

### Nullang: `List<struct>` — la lista accetta elementi struct (2026-05-30)

Chiuso il gap che la sonda `selfhost-lexer.null` aveva fatto emergere dall'uso:
la tabella token naturale è `List<Token>`, ma `List` ammetteva solo scalari.
Ora **gli struct sono elementi di lista validi** (`List<Point>`). Quasi gratis
come previsto: uno struct è già un handle a 64 bit, quindi entra nello stesso
slot uniforme che `List` usa per boxare un puntatore String — nessun nuovo
meccanismo runtime, solo l'estensione del boxing.

- `ElemTy` guadagna la variante `Struct(u32)`; `box_slot`/`unbox_slot` trattano
  l'handle come il puntatore String (`(long)(intptr_t)` in avanti,
  `(nlstruct<id>)(intptr_t)` al ritorno). `Ty` resta `Copy`.
- `TypeRef` guadagna il campo `elem`: il parser non può risolvere l'elemento di
  `List<Point>` (non ha la tabella struct), quindi stasha il `TypeRef`
  dell'elemento e il **checker** lo risolve (nuovo `resolve_typeref`, speculare
  in checker e codegen per l'annotazione di `[]` vuota).
- **Semantica a riferimento attraverso la lista**: `let e = xs[i]; e.f = v;`
  muta il record dentro la lista (l'elemento è un handle). Verificato.
- Liste annidate (`List<List<T>>`) e liste di enum **restano deferite**
  (`Typ003` per l'elemento illegale).

Suite **65/65** (5 nuovi: push+read per handle, `[]`-con-annotazione, literal di
struct, elemento di tipo errato, lista-di-liste rifiutata). Nuovo esempio
`examples/list-of-structs.null` (ELF verde, mutazione via handle in lista).
`examples/selfhost-lexer.null` **riscritto alla forma naturale `List<Token>`**
(via il workaround struct-of-arrays): stesso output (14 token), codice più
liscio — la prova sul campo che il muro è caduto. SPEC §4.2/§11 aggiornato.
`Cargo.lock` invariato.

### Nullang: frammento di lexer in Nullang — prima sonda di self-host (2026-05-30)

Primo passo misurabile verso il self-host (SPEC §12): `examples/selfhost-lexer.null`
è un frammento del lexer di Nullang **scritto in Nullang**, che tokenizza un
sottoinsieme del linguaggio (whitespace, numeri, identificatori/keyword,
punteggiatura). Compila a ELF ed esegue verde via `null run`: su input
`fn add(a, b) { let x = 12; }` produce 14 token classificati
(keyword/ident/punct/number).

**È un banco di prova di `List`+`struct`, non solo codice** — e ha fatto emergere
il prossimo gap host *dall'uso, non dall'intuito*:

- **Muro trovato: `List<struct>`.** La forma naturale della tabella token è
  `List<Token>`, ma `List` ammette solo elementi scalari (Int/Bool/String). Il
  file usa il workaround **struct-of-arrays** (tre `List<Int>` parallele:
  `kinds`/`starts`/`lens`); `struct Token` resta usato come valore di ritorno di
  `scan_*` ma non può ancora vivere in una lista. Il fix è **quasi gratis**: uno
  struct è già un handle a 64 bit → entra nello slot uniforme che `List` usa per
  boxare. Conferma sul campo che "struct a riferimento" era la scelta giusta.
  → prossimo pezzo host.
- Attrito minore: niente `char→Int` né ordinamento su String, quindi le classi
  di carattere si esprimono per enumerazione (`is_digit` a 10 rami) o negazione
  (`is_ident_char`). `||`/`&&` invece **funzionano** (una predizione opposta è
  stata smentita dalla sonda — il metodo "misura, non indovinare" ha retto).

Solo `examples/` — zero impatto sul compilatore.

### Nullang: `struct` — record nominali a riferimento (Direzione B, 2026-05-30)

Aggiunto `struct` al compilatore Nullang: con `List<T>` (v0.3) completa il
nucleo di data-modelling necessario a esprimere le tabelle del compilatore
stesso — **la precondizione per il self-host** (SPEC §12). Tocca tutte e
cinque le fasi. Decisioni di design (concordate con l'utente):

- **Semantica a riferimento**, come List: uno struct è un handle a un header
  su heap (`nlstruct<id>`, un puntatore in C), quindi le scritture di campo
  propagano attraverso gli alias e uno struct entra GIÀ nello slot uniforme
  64-bit di List (List<struct> diventa quasi gratis in futuro).
- **Dichiarazione** `type Name = { field: Type, ... };` (nuova keyword
  `type`); **costruzione a campi nominati** `Point { x: 1, y: 2 }` (tutti i
  campi obbligatori, una volta ciascuno, ordine libero); **lettura** `p.x`;
  **scrittura** lvalue `p.x = v`, con catene `p.a.b = v`. La scrittura
  richiede che la radice della catena sia `let mut` (stessa disciplina di
  superficie di `push`/`set` su List). Niente field-update expression, niente
  costruzione posizionale, niente `xs[i] = v` (§10, una via per concetto).
- **Campi v0.4**: `Int`/`Bool`/`String` o un altro struct (handle, con
  self-/mutua-referenza grazie alla forward-declaration dei typedef in C);
  campi enum-typed e List-typed deferiti (`Sch010`).
- Disambiguazione parser: `Name { ... }` è uno struct literal solo se `Name`
  è PascalCase (convenzione §4.1) — non collide con `if cond { ... }` /
  `while cond { ... }` le cui condizioni sono identificatori snake_case.
  Field access `.field` è postfisso (dopo un'espressione), distinto dal
  simbolo enum `.red` (a inizio primary). Nomi campo manglati `nlf_` per
  schivare le keyword C.

Suite **58/58** (11 nuovi test: costruzione/lettura, write-richiede-mut,
write-con-mut, campo mancante / sconosciuto / tipo errato, lettura su
non-struct, struct annidato + catena `p.a.b = v`, campo-List-deferito,
nome-tipo duplicato, ritorno-per-handle). Esempio `examples/structs.null`
(Point/Line, scrittura mutabile, semantica a riferimento verificata via
alias, struct-in-struct, catena) compila a ELF ed esegue verde via
`null run`. SPEC §4.2/§4.7/§11 aggiornato. `Cargo.lock` invariato.
### Nullang fix: `if`/`match` in posizione di statement senza `;` (PAR010) (2026-05-30)

Chiude l'unico attrito sistematico emerso dal **benchmark agentico** (Nullang
vs Python vs C, stesso `wc`-lite, 3 agenti per arm): tutti e 3 gli agenti
Nullang sbattevano sullo stesso identico `PAR010` ("expected `}` to close a
block, found `if`") e lo risolvevano a mano. Causa: in `parse_block` un'`if`
parsata come espressione, se non seguita da `=` o `;`, veniva sempre trattata
come **valore di coda** del blocco con `break` → poi `expect(RBrace)` falliva
sul secondo statement. Quindi due `if` consecutivi come statement erano
illegali senza `;` esplicito.

Fix (semantica identica a Rust): un'espressione block-like (`if`/`match`) in
posizione **non-finale** (token successivo ≠ `}`) è uno statement a sé, niente
`;`. In coda resta valore del blocco — comportamento precedente **preservato**
(retro-compatibile: i `;` espliciti continuano a funzionare). Cambiata solo una
branch in `parser.rs::parse_block`; `;` resta obbligatorio per chiudere `let`,
assegnamento e call-statement.

Suite `nullang` 60/60 (i 2 nuovi sopra struct+List: bare-if-statement compila,
tail-if resta valore). Verificato fuori-suite sul tree mergeato: la forma
"naturale" di `wc`-lite (i due `if` senza `;` che gli agenti volevano scrivere)
compila a ELF e passa 6/6 le fixture del benchmark; `examples/structs.null` e
gli altri esempi intatti. SPEC §4.5 aggiornata. Primo data point: una feature di
linguaggio **misurata** invece che ipotizzata. Sviluppato in un worktree isolato
(`nullang-par010`) per non collidere con `struct` in corso, poi cherry-pickato
sopra `struct` (`1a4da6a`) — unico conflitto questo CHANGELOG, `parse_block`
auto-mergiato pulito.

### Nullang: `List<T>` — collezione built-in mutabile (Direzione B 2/2, 2026-05-30)

Aggiunto `List<T>` al compilatore Nullang (`bootstrap/system/nullang/`): il
muro #2 della mappa dei gap e l'ultimo pezzo pesante lato host prima di poter
puntare al self-host. Tocca tutte e cinque le fasi
(lexer/AST/parser/checker/codegen). Decisioni di design (concordate con
l'utente):

- **Container built-in monomorfico**, non generics utente: il compilatore
  conosce `List<T>` come caso speciale; nessun `fn f<T>`. Element type scalare
  (`Int`/`Bool`/`String`); liste annidate e liste di enum deferite. `Ty` resta
  `Copy` (nuovo `ElemTy` piccolo e `Copy`) → le fasi esistenti non richiedono
  rework.
- **Semantica a riferimento**: una lista è un handle a un header su heap
  (`struct { long len, cap; long* data; }* nl_list`), perciò `push`/`set`
  mutano in place. Per onorare la mutabilità di superficie, il checker
  **richiede che `push`/`set` operino su un binding `let mut`**.
- **API**: literal `[a, b, c]`, lettura `xs[i]`, scrittura `set(xs, i, v)`,
  aggiunta `push(xs, v)`, lunghezza `list_len(xs)`. Niente `xs[i] = v` lvalue
  (§10). Slot uniforme a 64 bit (VM LP64), `String` boxata via `intptr_t`.
  Indici totali: read fuori range → default, write → no-op (come `substr`).
  `[]` vuota prende il tipo da `: List<T>`.
- `push`/`set`/`list_len` sono intrinseci polimorfici (l'unica polimorfia del
  linguaggio) — nomi riservati.

Suite **47/47** (9 nuovi test: literal/index, runtime calls, boxing `String`,
e i casi negativi push-non-mut / tipo-elemento / `[]`-senza-annotazione /
index-non-lista / index-non-Int). Esempio `examples/lists.null` (Int list +
buffer di righe `List<String>` + scan con indice mutabile) compila a ELF ed
esegue verde via `null run`. SPEC §4.2/§4.7/§11 aggiornato. `Cargo.lock`
invariato (nessuna nuova dipendenza → `cargoHash` di `nullang.nix` resta
valido).

### Threat-model — agent-primary ≠ agent-trusted: il confine è l'hypervisor (2026-05-30)

Discussione con l'utente che chiude un buco concettuale: *se l'agente non è
fidato, come lo sandboxiamo?* Conclusioni messe nero su bianco in `DESIGN.md`
(nuova sezione "Trust model & sandboxing"). Punti chiave:

- **La capability nel linguaggio è audit, non sicurezza.** `requires cap[...]`
  vincola solo il codice che *passa* da Nullang; un agente malevolo o *sviato*
  (confused deputy via prompt injection — quindi "untrusted" è il **default**,
  non un caso raro) ha la shell e bypassa il boundary. È l'analogo Nix di
  scrivere la sandbox *dentro* l'espressione.
- **Due soggetti distinti.** Le *app generate* sono già sandboxate dal kernel
  (Traccia A: netns/Landlock/seccomp, ALIVE). L'*agente stesso* no: in un OS
  agent-primary tiene per costruzione le capability di attivazione. "agent-
  primary" e "untrusted agent" al 100% sono in tensione genuina.
- **L'unico confine reale è kernel/hypervisor.** Modello scelto per l'alpha:
  *perimeter-as-jail* (la VM è la prigione, l'agente è dio dentro, TCB = QEMU/
  KVM). Regge solo a 3 condizioni: (1) il cervello è la rete → non "niente
  internet" ma *egress unico sorvegliato* verso il modello (o modello locale);
  (2) perimetro **pulito** — niente mount RW di segreti host; (3) provenienza
  sugli output, perché escono e li fidi dopo.

**Sharp edge corrente documentato** (non ancora chiuso, accettato per alpha
mono-utente): il `boot-vm` viola (1) — NAT user-mode dà egress generale — e (2)
— `~/.claude` montato **RW** via 9P `claudefs`, canale di scrittura verso
l'host (un agente dentro può iniettare MCP/hook che poi girano *sull'host*).
Annotato con nota `THREAT-MODEL:` in `flake.nix` accanto al lancio qemu;
corretto anche il commento stale che chiamava il mount "read-only". Nessuna
modifica funzionale: il mount resta RW perché l'agente ci scrive davvero
(`.claude.json`, backup). Fix futuro: share RO delle sole credenziali +
scratch scrivibile separato, e proxy di egress al posto del NAT.

### Nullang — `let mut` + `while`: stato mutabile e iterazione (Direzione B, 1/2) (2026-05-29)

La chirurgia parser/typer che l'agente NON può fare (fuori dal confine builtin),
quindi lavoro host. **Uccide il muro #3** della mappa gap: la ricorsione
esauriva lo stack sui file >~qualche kB; ora si itera. Da sola **sblocca la
Wave 2** (sed/grep batch: scandisci una String con un indice mutabile in un
`while`, accumula l'output — niente `List` necessaria).

- `let mut name = expr;` — binding riassegnabile; `name = expr;` lo riassegna.
  Assegnare un binding non-`mut` → `TYP001`; il tipo del valore deve combaciare.
- `while cond { ... }` — loop mentre `cond` (Bool) regge; è uno statement.

Toccate tutte le fasi: lexer (keyword `mut`/`while`), AST (`Stmt::Assign`,
`Stmt::While`, flag `mutable` su `Let`), parser (assegnamento = ident seguito da
`=`; solo una variabile è lvalue), checker (mappa locali ora `(Ty, bool)`:
traccia la mutabilità; valida mut+tipo sull'assegnamento, Bool sulla cond),
codegen (`Assign` → `x = v;`; `While` → `for(;;){ <cond>; if(!c) break; <body> }`
così la cond si rivaluta a ogni giro anche se è un if/match abbassato). I locali
C erano già mutabili (non-const), quindi `mut` è puramente disciplina di
typecheck.

Suite `nullang` 38/38 (3 nuovi: loop→C, assign-a-non-mut→TYP001, cond-non-Bool→
TYP001). Smoke host: somma iterativa 0..9 = 45; scan iterativo di stringa (conta
'a' con indice mutabile, **niente ricorsione**) = 6. Niente nuove dipendenze.
Prossimo in B: `List<T>` (buffer a righe indicizzato).

### Decisione — il compilatore è generation-managed: aggiornamenti a caldo, niente reboot (2026-05-29)

Problema sollevato dall'utente: se ogni miglioria al linguaggio costasse un
reboot, l'agente in-VM **perderebbe il contesto di sessione ogni volta** che
sbatte contro un muro del linguaggio. Il loop di auto-miglioramento dev'essere
a caldo e persistente.

Insight: `nullang` è solo un binario userspace, e l'init mette `/run/current/bin`
davanti a `/bin` nel PATH. Quindi il compilatore si aggiorna **come ogni altro
pacchetto** — `nv-pkg install` del nuovo `nullang` (pacchetto `nv-toolchain`) →
dichiaralo in `system.null` → `nv-rebuild switch`. `/run/current/bin/nullang`
ombreggia il `/bin/nullang` cotto, **persiste in `/var` (sopravvive al reboot)**,
e il rollback è `nv-rebuild rollback` (generation reale). È lo stesso modello
`configuration.nix`+`nixos-rebuild switch` di NixOS, esteso al compilatore.
Prima: l'agente faceva `cp` su `/bin/nullang` (effimero, solo-RAM, perso al
reboot). Ora: package+switch (persistente, no reboot, no perdita di contesto).

**Zero codice cambiato** — il meccanismo c'era già: `PATH=/run/current/bin:/bin`,
nessun hardcode di `/bin/nullang`, `nv-rebuild` usa `null` non `nullang` (niente
chicken-and-egg). Cambia solo il **rito** in `BUILTINS_CONTRACT.md`
(build→package→declare→switch→smoke→rollback). Il `/bin/nullang` cotto resta il
pavimento (primo boot + floor di rollback).

### Milestone — primo builtin auto-servito dall'agente: `char_at` (loop di auto-miglioramento chiuso) (2026-05-29)

**L'agente in-VM ha esteso il proprio compilatore, da solo, e ha provato di non
averlo rotto.** Dentro la VM (modello intermedio, `BUILTINS_CONTRACT.md`): ha
copiato `/usr/src/nullang` in `/var`, aggiunto `char_at(s: String, i: Int) ->
String` come builtin puro (Sig in `check.rs::builtins()` + funzione C nel
`PRELUDE`, **solo le due regioni del contratto**), `cargo build`, backup+swap di
`/bin/nullang`, lanciato la sua smoke-probe (5 casi char_at: 0/middle/last/
past-end/negative) → exit 0, niente rollback. Demo: conta le 'a' in "banana
republic of bananas" → 6 (scan ricorsivo L→R via `char_at`).

Autocritica notevole dell'agente: aveva scritto "O(1)", l'ha **ritrattata da
solo** nel corretto "O(i), evita l'O(n²) di `substr`+`strlen` su uno scan L→R",
ricompilato e ri-verificato. Esattamente il rigore che rende l'esperimento
credibile.

Merge host-side: applicate verbatim le due regioni, `cargo build` + suite 35/35
(nuovo test `char_at`) + e2e (demo conta-'a' → 6 riprodotta). `Cargo.lock`
invariato → nessun ricalcolo `cargoHash`. È il primo passo concreto verso il
self-host: la forgia che migliora sé stessa, con la rete di sicurezza che regge.

### Nullang — fix: mangling degli identificatori utente (collisione con keyword C) (2026-05-29)

Bug trovato dalla smoke-probe dell'agente in-VM: `fn double(...)` (o un `let`/
param chiamato `int`, `static`, `return`, …) finiva verbatim nel C emesso e
collideva con la keyword C → `cc` falliva (`expected identifier before 'long'`).
Fuori dal confine builtin dell'agente (è codegen), quindi fix dell'autore host.

`codegen::mangle(name)`: ogni identificatore utente (nomi di funzione, parametri,
`let`, binder di `match`) viene prefissato `nlu_` — schema iniettivo che schiva
tutte le keyword C e i simboli runtime (`nullang_*`, `nl_argc/argv`, `nlenum*`,
`_t*`). `main` resta verbatim (entry point C, mai referenziato come valore).
`check.rs` setta il `c_name` dei func utente a `mangle(name)`, così definizioni e
chiamate combaciano; i builtin tengono il loro `nullang_*`. Suite 34/34 (nuovo
test keyword + aggiornate 2 asserzioni che leggevano il C non-manglato). e2e
host: il repro esatto dell'agente (`fn double`) compila e stampa 42.

### Build — compressione initramfs parallela (pigz) + niente rebuild sui doc (2026-05-29)

Due fix d'igiene al ciclo di rebuild dell'initramfs (1.16 GB, ricostruito a ogni
modifica di `nullang`):
- **`pigz` al posto di `gzip`** (`pkgs/initramfs.nix`): `gzip` è single-thread e
  pinnava 1 core su 16 per minuti. `pigz -9 -p $NIX_BUILD_CORES` comprime su
  tutti i core; output gzip-compatibile, il kernel lo decomprime identico
  (`RD_GZIP`). ~10-15× sulla fase di compressione.
- **`*.md` esclusi dal `src` di `nullang`** (`pkgs/nullang.nix`): editare
  `SPEC.md`/`BUILTINS_CONTRACT.md` cambiava l'hash del sorgente e forzava un
  rebuild completo di `nullang` + initramfs (è successo). I doc non sono input
  del compilatore: ora non triggerano più nulla.

### Decisione — loop di auto-miglioramento dell'agente: "intermedio, solo builtin" (2026-05-29)

L'agente in-VM ha proposto di chiudere il loop sul linguaggio stesso (provo →
muro → aggiungo la feature → ricostruisco → ritento), notando che la fucina è
già nella VM (rustc/cargo/cc) e manca solo il sorgente + l'autorità di
sostituire `/bin/nullang`. Insight suo: il modello generation+rollback di
NullVoidOS vale anche per il compiler.

Deciso (utente): **posizione intermedia — l'agente può aggiungere solo
builtin**, non toccare parser/typer/Ty. Razionale: i builtin sono ~70% delle
richieste e hanno blast radius minimo (un bug rompe un builtin); la sintassi/
tipi (`List`/`mut`/`while`/`struct`) ha blast radius totale (rompe la
compilazione di tutto) e resta all'autore host. Sicuro perché `nullang`
(construction) non è nel critical path di `nv-rebuild` (che usa `null`,
declaration) e `cargo` lo ricostruisce senza dipendere da `nullang` → swap
sempre reversibile.

Contratto in `system/nullang/BUILTINS_CONTRACT.md`: confine esatto (cosa può/
non può editare), pattern del builtin, e il rito build→swap→smoke-probe→
rollback. Stato: (1) ✅ smoke-probe costruita dall'agente (`nv-smoke-probe`,
generation-6, 13 check + fault-injection validata); (2) ✅ **consegna sorgente
= Via B**: `nullang` source infilato nell'immagine a `/usr/src/nullang`
(read-only; `initramfs.nix` let `nullangSrc`, esclude `target/`) — la VM NON
tocca GitHub (repo privato + chiave SSH host non autorizzata + niente PAT nella
VM); l'agente copia in `/var`, edita, `cargo build`, swappa, push-back via diff;
(3) prossimo: primo builtin auto-servito (preferenza agente: `char_at`).
Orizzonte: self-host (Nullang in Nullang) = Wave 6+, dopo `List`/`struct`/
`while`/`mut`.

### Nullang — `argv`/`argc` (gate Wave 2: tool CLI parametrici) (2026-05-29)

Dopo che l'agente in-VM ha chiuso Wave 1 (`nv-edit 0.1.0` = `cat -n` reale,
generation-5), la sua richiesta #1 era `argv` — senza, un `sed`-like opera solo
su parametri cablati a build-time, non è un tool. Aggiunti due builtin **puri**
(niente World, niente effetto): `argc() -> Int` e `argv(i: Int) -> String`
(convenzione C: `argv(0)` = nome programma, fuori range → `""`).

Scelta: **puri**, non gated. argv è dato di startup (non un effetto continuo);
gateggiarlo con `!proc.argv` aggiungerebbe cerimonia e un mismatch col
vocabolario di `null` (che non ha `proc.argv`). La forma ergonomica
`fn main(world, args: List<String>)` aspetta `List<T>` (§11).

Codegen: il `main` C passa da `int main(void)` a `int main(int argc, char**
argv)` e fa lo stash in due globali (`nl_argc`/`nl_argv`) che i builtin leggono
(le globali sono sempre assegnate — innocue se inutilizzate). Suite `nullang`
33/33 (1 nuovo test argv; aggiornata l'asserzione sulla firma di `main`). Smoke
e2e host: `build` + esecuzione del binario con argomenti reali → `argc=3`,
`argv(1)=alpha`, `argv(2)=beta`, `argv(99)=""`. Niente nuove dipendenze.

Sblocca Wave 2 (sed-like batch parametrico). Ancora pesanti per le ondate dopo:
`List<T>` + `mut`/`while` (buffer + scansione senza esaurire lo stack), poi
`read_line`/raw-TTY per l'interattività (Wave 4-5).

### Nullang Tier 0 — decomposizione stringhe, `==` su String, file I/O (2026-05-29)

Risposta diretta alla mappa dei gap dell'esperimento "costruisci un editor"
(l'agente in-VM aveva sbattuto sul fatto che Nullang sa *comporre* stringhe ma
non *scomporle*, non legge input/file, e `==` non vale su String). Tier 0 sblocca
gli strumenti **batch** (`cat -n`, `wc`, un `ed` non interattivo).

Cinque builtin + un operatore, tutti senza chirurgia su parser/tipi (modello
`concat`):
- `str_len(String) -> Int` e `substr(String, Int, Int) -> String` — puri; gli
  indici di `substr` clampano, quindi è totale (niente panic, niente tipo
  errore). `char_at(s,i)` = `substr(s,i,1)`.
- `read_file(World, String) -> String uses !fs.read` e `write_file(World,
  String, String) uses !fs.write` — effectful (World-gated come `print`).
- `==`/`!=` su `String` (typechecker esteso; codegen → `strcmp(a,b) {==,!=} 0`,
  non confronto di puntatori).

**La cucitura linguaggio↔capability si chiude qui.** L'effetto del linguaggio è
*path-less* (`uses !fs.read`, senza path — il path è un `String` a runtime); il
grant in `system.null` (`!fs.read."/dir"`) lo scopa, e **Landlock lo impone** a
`nv-rebuild run` (il lavoro `!fs` di prima). Verificato: `nullang package`
deriva `capabilities: ["fs:read","fs:write"]` dal `uses` di `main`, e
`read_file` senza `uses !fs.read` dà `EFF001 + repair add-uses-clause`.
Estendere il set di effetti di Nullang **accende** l'enforcement senza altro
codice.

Implementazione in `check.rs` (4 builtin + `==` su String in `check_binary`) e
`codegen.rs` (impl C in PRELUDE + `strcmp` per l'uguaglianza di stringhe).
`read_file` ritorna `""` su errore per ora (un `Result` enum, §10, è il
follow-up). Suite `nullang` 32/32 (6 nuovi test Tier 0). Smoke end-to-end
host (codegen→cc→run): `str_len`/`substr`/`==`/round-trip `write_file`→
`read_file` tutti corretti, file scritto davvero su disco. Nessuna nuova
dipendenza, build offline.

Prossimo (dalla mappa gap dell'editor): Tier 1 (`read_line`/stdin → editor di linea),
Tier 2 (`List`+`struct`, chirurgia su parser/tipi → buffer vero), Tier 3 (`mut`
+ raw tty → TUI).

### Traccia A — capability enforcement a runtime: slice `!proc.spawn` + `!rand` via seccomp (2026-05-29)

Terza e quarta capability applicate a runtime, dopo `!net` (netns) e `!fs`
(Landlock). Usano **seccomp-bpf**: un filtro cBPF che restituisce `EPERM` per i
syscall che una capability mancante deve vietare. **PASS in VM** (verifica host
verde su tutti e 4 i casi prima del boot).

**Kernel:** nessun rebuild — `SECCOMP`/`SECCOMP_FILTER` erano già abilitati col
blocco `!net`.

**Supervisore** (`system/nv-rebuild`): `nv-rebuild run` costruisce un programma
cBPF (allow-all, `EPERM` sui syscall vietati) e lo installa nel figlio via
`pre_exec` — `prctl(PR_SET_NO_NEW_PRIVS)` poi `prctl(PR_SET_SECCOMP,
SECCOMP_MODE_FILTER)`. Installato post-fork/pre-execve, quindi l'execve di
lancio non è mai bloccato e `unshare(2)` (syscall distinto dalla famiglia clone)
continua a funzionare per il trampolino `!net`. Mancante `!proc.spawn` → deny
`fork`/`vfork`/`clone`/`clone3`; mancante `!rand` → deny `getrandom`.

**Niente nuova dipendenza.** Il filtro è cBPF a mano via `libc`, raggiunto come
`nix::libc` (ri-esportato da `nix`, già dipendenza) — quindi `Cargo.toml`/
`Cargo.lock`/`cargoHash` invariati. Scelta forzata e fortunata: crates.io era
intermittente, `seccompiler`/`libc`-diretto non scaricabili; il vendor FOD col
lock cambiato voleva ri-fetchare tutto e falliva offline. `nix::libc` evita
del tutto il problema.

**Demo** `system/demos/procrand-enforce/`: un pacchetto, due probe C compilati
in-VM (`getrandom`, `fork`), quattro servizi che differiscono in una sola
capability. Host (seccomp è unprivileged con `NO_NEW_PRIVS`) + VM:
`rand-granted`/`spawn-granted` exit 0, `rand-denied`/`spawn-denied` exit 7.

**`!proc.exec` NON enforced (onesto):** cBPF stateless non può permettere solo
l'execve di lancio e bloccare i successivi — serve seccomp `USER_NOTIF` o un
supervisore ptrace. Negare `!proc.spawn` già blocca il pattern fork+exec di
helper (la minaccia pratica); l'audit line stampa `exec=denied` ma il filtro non
agisce. Le quattro capability del vocabolario ancora recorded-only restano
`!net.localhost` (raffinamento di `!net`) e `!activate.system` (privilegio già
implicito).

### Traccia A — capability enforcement a runtime: slice `!fs` via Landlock (2026-05-29)

La seconda capability applicata a runtime, dopo `!net`. Dove `!net` usa un
network namespace, `!fs` usa **Landlock** — l'access control path-scoped e
deny-by-default del kernel, mappato 1:1 sul vocabolario (`!fs.read."P"`,
`!fs.write."P"`). **PASS in VM** con kernel Landlock.

**Kernel** (`pkgs/kernel.nix`): `CONFIG_SECURITY` + `CONFIG_SECURITY_LANDLOCK`
+ `CONFIG_LSM="landlock"`. tinyconfig spedisce `SECURITY` off, quindi Landlock
va acceso e aggiunto alla lista LSM attiva o i syscall danno ENOSYS.

**Supervisore** (`system/nv-rebuild`): `nv-rebuild run` estrae le capability
`fs.read`/`fs.write` dal descrittore, costruisce un ruleset Landlock e fa
`restrict_self` **prima** dello spawn. Il ruleset è ereditato attraverso
fork+execve (e attraverso il trampolino `unshare` di `!net`), quindi i due
confinamenti compongono sul binario finale. Nuova dipendenza Rust `landlock`
0.4 (la rete era tornata → `cargo generate-lockfile` + `cargoHash` ricalcolato).

**Baseline.** Deny-by-default bloccherebbe un servizio dal leggere il *proprio*
binario (l'execve lo risolve sotto il ruleset), quindi il supervisore concede
sempre un baseline di runtime: read+execute sui tree di codice (`/bin`,
`/nix/store`, `/run`, `/var/lib/nv-store`, `/lib`, `/usr`, `/proc`) e read+write
su `/dev` e `/tmp`. Lo smoke host ha colto che un `/dev` read-only rompe perfino
un `2>/dev/null`. Il vocabolario protegge i path *dati* fuori dal baseline.

**Demo** `system/demos/fs-enforce/`: un binario, due servizi che differiscono
solo in `!fs.read."/srv"` (entrambi con `!net` per isolare la variabile fs). Una
canary in `/srv`. Verifica host (Landlock è unprivileged → prova piena del
meccanismo) + **PASS in VM headless**: `fs-granted` legge la canary (exit 0),
`fs-denied` bloccato da Landlock (exit 7). Stesso binario, esito opposto deciso
solo dalla capability dichiarata.

**Limiti onesti:** subtree granularity (`PathBeneath`); il baseline è grossolano
(tightening al solo store path del binario è follow-up); `!proc.*`/`!rand`
(seccomp) restano il prossimo incremento (`SECCOMP` già compilato).

### Traccia A — capability enforcement a runtime: slice `!net` (kernel + supervisore) (2026-05-29)

La prima capability che NullVoidOS **applica** a runtime invece di limitarsi a
registrarla. Finora il vocabolario (`!net`, `!fs.*`, `!tty`, …) era
*recorded-only*: `system.null` concedeva, i pacchetti dichiaravano, `null`
type-checkava `requires ⊆ caps` — ma nulla impediva a un processo di fare ciò
che non aveva dichiarato. Questo slice chiude il buco per `!net`. **Tutti e tre
gli stadi fatti; Stadio 3 (prova falsificabile) PASS in VM headless con kernel
+ `nv-rebuild` nuovi.**

**Stadio 1 — kernel** (`pkgs/kernel.nix`). Abilitati `NET_NS` + l'intera
famiglia namespace (`UTS/IPC/PID/USER_NS`, `NAMESPACES`) + `SECCOMP`/
`SECCOMP_FILTER`. Root cause scovato: `tinyconfig` setta `EXPERT=y` e disabilita
`MULTIUSER`+`SYSVIPC`; `CONFIG_NAMESPACES depends on MULTIUSER`, quindi senza
abilitare prima le dipendenze `olddefconfig` scartava silenziosamente l'intero
blocco namespace (incluso `NET_NS`). Aggiunti `MULTIUSER`+`SYSVIPC`; ribuild
verde, tutti i simboli `=y`.

**Stadio 2 — supervisore** (`system/nv-rebuild`). Tre cuciture:
- `manifest.rs`: nuovo tipo `Capability { path, arg }` + campo `requires` su
  `Service` (deserializzato da `null eval`).
- `generation.rs`: `requires` persistito nel descrittore `etc/services/<name>`
  come riga `requires=<token>` (token compatti: `net`, `net.localhost`,
  `fs.read:/etc`, `tty`).
- `cli.rs` + `main.rs`: nuovo comando **`nv-rebuild run <service>`** — legge il
  descrittore della generation attiva, stampa una audit line, e lancia il
  processo confinato: senza capability `net` → `unshare -n` (applet busybox,
  netns fresco, solo loopback down) → niente rete; con `!net` → resta nel netns
  host. Nessuna nuova dipendenza Rust (vincolo offline: il vendor fetch vuole
  rete); `cargoHash` invariato.

**Demo riproducibile** (`system/demos/net-enforce/`): un solo binario probe,
pacchettizzato una volta, dichiarato come **due servizi che differiscono solo
nelle capability concesse** (`net-granted` = `[!net !tty]`, `net-denied` =
`[!tty]`). Probe via `/proc/net/dev` (namespaced — riflette il netns, a
differenza di `/sys/class/net` che ha ingannato una prima versione). Test
falsificabile: stesso codice, esito opposto deciso solo dalla capability
dichiarata.

**Verifica host** (nessun boot, store offline): kernel ribuiltato coi simboli
namespace `=y`; `nv-rebuild` ricompilato offline col comando `run`; host-dry-run
completo `nv-pkg install → nv-rebuild switch → descrittore → run` — confermato
che `requires` fluisce fino al descrittore (`requires=net tty` / `requires=tty`)
e che `run net-granted` esegue confinato nel netns host (exit 0). Il meccanismo
netns dimostrato con `unshare -rn` + `/proc/net/dev`.

**Stadio 3 — PASS in VM** (headless, QEMU separato senza mount di `~/.ssh`/
`~/.claude`, solo il repo in 9P read-only, `poweroff` automatico). `net-granted`
→ default route via `eth0` → exit 0; `net-denied` → `unshare -n` → nessuna
default route → exit 7. Stesso binario, stesso pacchetto: l'esito opposto è
deciso solo dalla capability dichiarata. Il run in-VM ha catturato due bug del
*probe* che l'host non poteva vedere: `/sys/class/net` non riflette il netns del
lettore, e un netns fresco auto-crea `sit0` (tunnel IPv6-in-IPv4) oltre a `lo` —
quindi "qualsiasi interfaccia non-`lo`" dava falso positivo. Probe finale: test
della **default route** in `/proc/net/route` (per-netns, offline-safe). Il
meccanismo di enforcement era corretto fin dal primo run (in `net-denied`
`eth0` era già sparito); sbagliava solo il giudizio del probe.

**Limiti onesti (documentati):** enforce di `!net` soltanto; `!fs` (Landlock),
`!proc.*`/`!rand` (seccomp) sono i prossimi incrementi (primitive kernel già
compilate). `!net.localhost` trattato come `!net` pieno per ora. La
supervisione vive provvisoriamente in `nv-rebuild run` (one-shot, manuale); un
supervisore di boot con restart policy reali è un pezzo a parte (probabile
`nv-init`).

### Milestone — wow-moment: un agente *dentro la VM* autora e dichiara un pacchetto da solo (2026-05-29)

Il deliverable narrativo dell'agent-primary OS. Dato un prompt **goal-level**
(non passo-passo) a `claude` *dentro la VM bootata*, l'agente ha attraversato
l'intera pipeline `author → package → declare → switch → run` **da solo**,
imparando il linguaggio dalle sole diagnostiche del compilatore — la proprietà
"tooling auto-documentante" (bundle `skills`, `explain`, codici stabili + repair
tipati) ha pagato: 5 round di diagnostica (`SCH001`, `TYP001`/`TYP002`,
`EFF001` → repair `add-uses-clause`, `PAR010`) e zero intervento umano sulla
sintassi.

**Cosa ha costruito** — `nv-watchdog` (Nullang construction mode): `enum
Severity = .healthy | .warning | .critical`, helper puri (`classify`,
`sev_code`/`sev_label`/`sev_from_code`, `max_int`), `worst(a,b,c)` che aggrega
tre severity prendendo il massimo via round-trip su `Int`, e `line(...)` che
compone un report dinamico con `concat`/`str_of_int` annidati. `main(world:
World) -> Int uses !tty` — capability dichiarata nella signature, richiesta
dall'effetto `print`.

**Pipeline eseguita in-VM:**

1. `nullang package … --install` → `e61a95da…-nv-watchdog-0.1.0` nel CAS;
2. `/etc/nullvoid/system.null` esteso con `pkgs.nv-watchdog` e un servizio
   `watchdog` (`restart = .on-failure` — **simbolo enum**, non stringa;
   `requires = [ !tty ]` ⊆ `caps`);
3. `null check` ok → `nv-rebuild check` ok → `nv-rebuild switch` ha attivato
   **`generation-3`** (l'accumulatore è monotòno attraverso i reboot: lo
   stretch test umano-guidato si era fermato a `generation-2`);
4. `/run/current/bin/nv-watchdog` → report strutturato; `mem=91 ≥ 90` →
   `overall: crit` → **exit code 2** (semantica 0/1/2 = ok/warn/crit).

**Verifica indipendente host-side** (questa sessione, **nessun boot**, store
offline): ricostruito `nullang` dal sorgente corrente (`cargoHash` valido,
`Cargo.lock` invariato) → ricompilato `nv-watchdog` via codegen→`cc`→ELF →
riprodotto exit 2 con output identico (`cpu=72 [warn]`, `mem=91 [crit]`,
`disk=34 [ok]`, `overall: crit`); `null eval` sul `system.null` → manifest
pulito con `restart: "on-failure"` (simbolo enum serializzato), `requires ⊆
caps`. Il fallimento di `null check` con `pkgs.*` su host è solo `REF002`
(`nv-pkg` assente dal PATH) — l'ambiente `pkgs.*` funziona, i pacchetti vivono
nel CAS della VM.

**Delta vs lo stretch test Phase 1** (real Rust ELF, umano-guidato): lì la
sequenza la guidava l'umano; qui la guida l'agente, da un obiettivo. Artefatto
catturato: `bootstrap/system/nullang/examples/nv-watchdog.null` (estensione
normalizzata `.nullang` → `.null`, convenzione del repo).

### null — test di regressione anti-drift sugli esempi (2026-05-29)

`all_examples_eval_clean` evala ogni `examples/*.null` via `null::run_eval` e
panica rumorosamente su qualsiasi errore, più un assert che fallisce se la glob
non matcha nulla (niente pass vacuo). Chiude la classe del drift triplo che era
marcito in silenzio perché nessuno rievaluava quei file. Suite `null` 54/54.

### Crossing nullang→.nvpkg→OS validata + fix drift esempi `.null` (2026-05-29)

Esercitata l'intera crossing sull'host in un prefix usa-e-getta (`NV_STORE_ROOT`
/ `NV_SYSTEM_ROOT` / `NV_CONFIG`, **nessun boot VM**): un programma autorato in
nullang (`supervise`) ha attraversato `nullang package` → `nv-pkg install` (CAS)
→ dichiarazione `pkgs.supervise` in `system.null` → `null eval` → `nv-rebuild
switch` → eseguito da `current/bin` (reason dinamica, exit 255). La **cucitura
capability** gira nel loop: il pacchetto *consuma* `tty` (`uses` → manifest
`capabilities:["tty"]`), il sistema lo *concede* via `caps`. Nessun gap di
linguaggio emerso — Nullang esprime già ciò che la crossing richiede; gli
arm-block restano §11 non urgenti.

La crossing ha però rivelato che gli esempi declaration-mode di `null`
(`examples/{minimal,standard,multi-service}.null`) erano **triplamente in
drift** rispetto allo schema `SystemManifest` e fallivano `null eval`:
(1) mancava il campo top-level richiesto `caps` (`SCH001`); (2) `restart` era
una stringa `"always"` invece del simbolo enum `.always`/`.on-failure`/`.never`
(`TYP004`); (3) i servizi non avevano il campo richiesto `requires`
(`SCH001`). Corretti tutti e tre con `caps`/`requires` coerenti
(`requires ⊆ caps`); ora evalano puliti (exit 0). `minimal.null` si
auto-descrive come smoke-test `null eval` ed era rotto.

### Nullang — composizione esplicita di stringhe: `concat` + `str_of_int` (2026-05-29)

Due builtin **puri** (nessun `World`, nessun effetto): `concat(String, String)
-> String` e `str_of_int(Int) -> String`. Implementano — non rovesciano —
l'anti-feature §10 *"compose explicitly"*: niente interpolazione, niente
overload di `+`, e `concat` è **binario** (niente variadici, §10), quindi
l'annidamento è il costo voluto. Codegen: `nullang_concat` (malloc + memcpy,
non liberato — i programmi v0.2 sono short-lived, arena/ownership è §11) e
`nullang_str_of_int` (snprintf `%ld`). `str_of_bool` rimandato (nessun bisogno
reale).

Emerso **demand-driven** da una sonda di dogfooding (un decisore di
supervisione stile nv-rebuild, *non* load-bearing): scritto come lo si vorrebbe
davvero, con reason dinamica, ha rivelato che il vero muro non erano i `match`
arm-block (aggirabili con una helper fn, A/B verificato) ma la composizione di
stringhe — una capability mancante senza workaround. Nuovo esempio
`examples/supervise.null`, verificato end-to-end: compone e stampa
`service failed with code 1 after 5 attempts`, esce 255. 26 test verdi (3
nuovi). Prossimo gap noto: arm-block (ergonomia, §11).

### Nullang v0.2 — enum payloads: gli enum portano dati (2026-05-29)

Una variante di enum può ora portare **un singolo payload tipato**
(`enum Status = .code(Int) | .message(String) | .none`); i tipi di payload
ammessi sono `Int`/`Bool`/`String` (enum annidati, `World` e `Unit` restano
rimandati — richiederebbero indirezione o non portano dato, SPEC §11). Chiude
l'item "Enum payloads" della roadmap §11 e rende reale l'anti-feature §10
("le operazioni fallibili restituiscono un enum result; il chiamante fa match").

- **Costruzione**: `.code(42)`, `.message("oops")`; le varianti nude (`.none`)
  restano senza argomento. Il disallineamento è `TYP021`
  (repair `supply-payload` / `drop-payload`).
- **`match` con binding**: `.code(n) => n`, `.message(_) => …`; il nome lega il
  payload nello scope dell'arm col tipo dichiarato. Un arm con payload **deve**
  legare (`_` per scartare), uno nudo non deve — entrambi `TYP021`
  (repair `bind-payload`).
- **Lowering (SPEC §7)**: gli enum **senza** payload restano un `long` nudo
  (zero costo per i flag); quelli con **almeno un** payload diventano una
  struct tagged per-enum `{ long tag; union {…} u; }`. Una sintassi, due
  lowering — paghi la union solo dove serve. Il `match` su enum tagged switcha
  su `.tag` e legge il payload dalla union; la costruzione è un compound
  literal C99.

Nuovo codice diagnostico `TYP021` (arità del payload). Loop chiuso verificato
end-to-end: `examples/result.null` stampa il payload `String` ed esce con il
payload `Int` (42). 23 test di integrazione verdi (9 nuovi); gli esempi
`status`/`compute`/`hello` invariati (percorso `long` nudo non regredito).

### Nullang cablato nell'initramfs — affiancato a `null` (2026-05-29)

`bootstrap/pkgs/nullang.nix` impacchetta il crate `nullang` come binario
musl-statico (`pkgsStatic.rustPlatform.buildRustPackage`, `cargoHash`
`sha256-ZH//AvI/0IiQGvjTmhHfaBLyZAtenSsA38keNbLVYws=`, `doCheck = false`).
La `src` usa `lib.cleanSourceWith` con esclusione esplicita di `target/`
(l'agente builda in-tree, e `lib.cleanSource` da solo non strippa `target/`).
Esposto come `.#nullang` via `default.nix`, copiato in `/bin/nullang`
nell'initramfs e aggiunto al banner di boot.

**Affiancato, non sostituito.** Il binario `null` resta nel critical path del
loop Phase 1 (nv-rebuild valuta `system.null` tramite lui); `nullang` viaggia
accanto per il dogfooding in-VM. Il ritiro di `null` aspetta la verifica che
nullang declaration mode legga `system.null` in modo identico (la CLI attuale
di `nullang` non espone ancora `eval`). Build verde verificata
(`nix build .#nullang` exit 0, `nullang 0.1.0`); wiring dell'initramfs
verificato via eval del `.drvPath`. Realizzazione completa dell'initramfs
rimandata a chiusura degli enum payloads, per shippare il compilatore finale.

### Nullang — `null package` closes the CAS+provenance half of §13 (2026-05-28)

`nullang package <file.null> --name N --version V [--author A] [--install]`
builds the ELF, then emits a CONTRACTS.md §1.1 `.nvpkg` (gzip tar with
`manifest.json` + `payload/bin/<name>` + `recipe.null`). The package's
`capabilities` are **derived from `main`'s `uses` clause** — the language's
static effect set becomes the package's declared capability set, saluting the
declaration/construction seam. Provenance lives in the manifest: `authoredBy`,
`createdAt`, `sourceLanguage: "nullang"`, and `buildSteps` carrying the
`source` and `emitted-c` SHA-256 hashes; the exact `.null` recipe ships inside
the package. With `--install`, shells out to `nv-pkg install`, which
content-addresses the tarball (the CAS).

Default is emit-only (Nullang builds, `nv-pkg` owns the store — clean contract
boundary); `--install` is opt-in. 14 integration tests pass (added capability
mapping + manifest shape). Verified: `package examples/hello.null` produces a
valid `.nvpkg` whose manifest lists `capabilities: ["tty"]`.

### Re-lock — Layer 3 is Nullang (one language, two modes) + Nullang v0.1 (2026-05-28)

Reverses the same-day Layer 3 lock (*".null v2, not Zero"*). The level
above both prior decisions: **Nullang is a single agent-native language
with two modes** — *declaration mode* (the eval-only `.null` profile,
unchanged) and *construction mode* (functions, effects, native codegen),
which replaces external ZeroLang across Layers 1-2 and Layer 4 over time.
Rationale recorded in `DESIGN.md` (RE-LOCK box, Layer 3 section):

1. **Sovereignty.** ZeroLang is Vercel Labs'; its death would force a
   rewrite of Layers 1-4. Owning the spec + compiler is non-negotiable
   now. Self-sufficiency (reimplementing stdlib/TLS) is a separate,
   incremental goal — not conflated with sovereignty.
2. **Codegen to C**, not LLVM/Cranelift: the substrate already ships a C
   compiler, so the backend adds no new external dependency that can die.
   Floor = kernel + libc + cc.
3. **Capability seam.** Declaration mode *grants* capabilities; construction
   mode *consumes* them via `World`. One vocabulary, two roles.

**Nullang v0.1** lands at `bootstrap/system/nullang/` (Rust host compiler,
bin `nullang`, SPEC at `system/nullang/SPEC.md`). The closed loop is green:

```
source.null → typed AST → effect check → C → cc → ELF → run
```

Implemented: `fn`, `let`, `Int`/`Bool`/`String`/`Unit`/`World`, the
capability/effect discipline (`uses`, checked statically; `World` is
erased at codegen — runtime enforcement deferred to Phase 2 per
CONTRACTS §4), arithmetic/comparison/logical operators, `if` expressions
(lowered via temporaries), and `enum` + `.symbol` + exhaustive `match`
(v0.1 rule: symbols globally unique). NDJSON diagnostics with stable
codes (`PAR`/`TYP`/`SCH`/`REF`/`CAP`/`EFF`/`MOD`/`CGN`) and typed repair
IDs. 12 integration tests pass; `examples/{hello,compute,status}.null`
build and run.

Built with `nix shell nixpkgs#cargo nixpkgs#rustc nixpkgs#gcc -c cargo …`
(rustc 1.95.0, gcc 15.2.0). Deferred: CAS+provenance wiring (the last
piece of SPEC §13, via `.nvpkg`), `mut`/ownership, generics, self-hosting.

### Milestone — Phase 1 stretch test passes (real Rust ELF + pkgs ambient) (2026-05-28)

`bootstrap/system/demos/hello-rust/stretch-test.sh` was driven
end-to-end inside the booted VM. Both gaps the original Phase 1 demo
left open are now closed:

1. **Real ELF binary.** `cargo build --release` (rustc 1.95.0 from the
   dev substrate, host-target nixpkgs glibc) compiled `hello-rust` in
   23.97s into a 301KB binary. No bash-script stand-in, no host-side
   cross-compilation.
2. **`pkgs.<name>` ambient exercised.** `system.null` referenced the
   package as `packages = [ pkgs.hello-rust ]`. The `.null` evaluator
   resolved it against the live `nv-pkg list --json` (which then
   listed both `hello-nv-0.1.0` from the earlier demo and the new
   `hello-rust-0.1.0`, confirming the store persists across reboots).
   The emitted `SystemManifest` carried the resolved literal
   `"hello-rust-0.1.0"` in its `packages` array.

The full loop ran clean:

```
cargo build --release             → 301 KB ELF in 23.97s
tar czf hello-rust-0.1.0.nvpkg    → 147 KB tarball
nv-pkg install                    → /var/lib/nv-store/b6b0bd5dbdf152d08fe0138fc4cb711c-hello-rust-0.1.0
null check + null eval            → SystemManifest with pkgs.hello-rust resolved
nv-rebuild switch                 → /var/lib/nv-system/current -> generation-2
which hello-rust                  → /run/current/bin/hello-rust
hello-rust                        → ran, printed pid/argv0/unix_ts/path_head
```

The generation counter bumped from 1 (from the previous demo) to 2,
confirming the activation engine's accumulator is monotonic across
reboots and the previous package (`hello-nv`) stayed installed but is
no longer in the active manifest — it's not garbage-collected, just
absent from `/run/current/bin/`, exactly as CONTRACTS §3.4 specifies.

Still not exercised (deliberately deferred, documented in the demo's
README):

- Static-musl target — needs `x86_64-unknown-linux-musl` added to the
  rustc target list in `devSubstrate`.
- Multi-package `deps` resolution — single-package install here.
- Runtime capability enforcement — recorded-only in Phase 1.

### Fix — `/etc/shells` + `/dev/pts` mount for PTY-aware services (2026-05-28)

Surfaced while driving the stretch test from the host: SSH into the
VM was failing in two distinct ways that the original Phase 0 (a)
boot had never tripped on (the interactive console seriale never
opened a fresh PTY or called `getusershell()`).

- **`/etc/shells` missing.** Dropbear validates each user's login
  shell against `getusershell(3)`. When `/etc/shells` does not exist,
  glibc returns a hardcoded `{/bin/sh, /bin/csh}` and dropbear
  rejects the login with *"user 'root' has invalid shell, rejected"*.
  Since `/etc/passwd` points root at `/bin/bash` (Phase 0 (a)
  contingency 1), every SSH from the host was permanently broken.
  Fixed by shipping `/etc/shells` with `/bin/sh` and `/bin/bash`.
- **`/dev/pts` not mounted.** `devtmpfs` populates `/dev/ptmx` but
  the slave nodes (`/dev/pts/N`) only appear under a separate
  `devpts` mount. Without it, any PTY-allocating program (`script`,
  interactive ssh sessions, tmux-like multiplexers) fails on the
  slave-open with ENOENT and the failure surfaces as something
  unrelated. Init now does `mkdir -p /dev/pts && mount -t devpts
  devpts /dev/pts` next to the other early filesystem mounts.

Both touch `bootstrap/pkgs/initramfs.nix`. A rebuild
(`nix build .#initramfs`) is needed before the next boot picks them
up. Known follow-up: even with these two fixes, dropbear still exits
with *"Failed to set euid"* after auth — likely a privsep / dropped-
privilege issue in the nixpkgs build that the in-VM agent never hit
(it uses the seriale directly). Tracked separately; SSH from host
is not on the Phase 1 critical path.

### Milestone — Phase 1 demo passes end-to-end (2026-05-28)

The six-step demo from CONTRACTS §5 — author → package → install
→ declare → switch → use — closes on the first try inside the
boot VM. The three Rust crates scaffolded by parallel sub-agents
in the previous session (`nv-pkg`, `null`, `nv-rebuild`) integrate
cleanly across the contracts they share, with no manual
glue-fixing between them.

Verified inside `nix run ./bootstrap`:

1. A package `hello-nv-0.1.0` was authored at `/tmp/pkg-src/`
   (manifest.json + `payload/bin/hello-nv`, a shell-script
   stand-in for a compiled binary).
2. `tar czf` produced a 447-byte `.nvpkg`.
3. `nv-pkg install` placed it at
   `/var/lib/nv-store/0fd5224c1a7c2b17f48218dcea2e3973-hello-nv-0.1.0/`.
4. A minimal `system.null` was written to `/etc/nullvoid/`:
   ```null
   {
     hostname = "nullvoid";
     caps = [ !tty ];
     packages = [ "hello-nv-0.1.0" ];
     services = {};
     environment = {};
   }
   ```
5. `null check` exited 0 silently; `null eval` emitted a clean
   `SystemManifest` JSON with the `!tty` capability serialised as
   `{"path":["tty"],"arg":null}`.
6. `nv-rebuild check` validated:
   `manifest ok: hostname=nullvoid`
   `[ok] hello-nv-0.1.0 -> /var/lib/nv-store/...`
7. `nv-rebuild switch` activated generation 1:
   `building generation 1...`
   `activated: /var/lib/nv-system/current -> generation-1`
8. `nv-rebuild generations` listed `generation-0` and
   `* generation-1 (current)`.
9. `/run/current/bin/hello-nv` resolved through the symlink chain
   to `/var/lib/nv-store/.../payload/bin/hello-nv`, `which`
   confirmed PATH lookup, and the binary ran:
   `hello from a package authored at 2026-05-28T18:15:57Z`.

This is the falsifiable test of Phase 1. The declarative loop
(edit `system.null` + `nv-rebuild switch` → atomic PATH change)
is closed end-to-end without an agent in the loop.

**Observations from the demo:**

- `null check` is silent on success (Unix convention). For
  agent-facing affordance, a future `--verbose` or non-JSON
  human summary on success would reduce "did it do anything?"
  doubt.
- `nv-rebuild generations` still lists `generation-0` (the empty
  bootstrap directory the initramfs creates at first boot).
  Cosmetic; a Phase 2 `nv-gc` would prune it.
- The `pkgs` ambient (SPEC §5.4 — populated by
  `nv-pkg list --json`) was not exercised: the demo references
  the package as a literal `"hello-nv-0.1.0"` string. Future
  test should use `packages = [ pkgs.hello-nv ];`.
- The package payload is a `bash`-script, not a compiled
  binary. The natural next stretch test is to compile a real
  Rust binary inside the VM (the dev substrate has rustc/cargo
  end-to-end), package it, switch, and run it — that exercises
  the build path inside the lab, not just the install path.

### Fix — /var probe is mount-first, not blkid-first (2026-05-28)

The smoke-test that verified the Phase 1 wire-up surfaced a latent
init bug. The /var bootstrap used `blkid /dev/vda` as the gate
between "mount existing fs" and "format then mount". `blkid` was
false-negativing on the existing qcow2 — likely because the
initramfs has no udev and blkid wanted a cache directory under
`/run` that the init script hadn't created yet — pushing init into
the mkfs branch every boot. `mkfs.ext4 -q` (no `-F`) then prompted
`Proceed anyway? (y,N)` on the existing fs and would have blocked
PID 1 forever; only the test's first piped character ("e" from
`echo`) accidentally answering "no" let the fs survive.

`bootstrap/pkgs/initramfs.nix`: switched to a mount-first probe.
The actual measure of "is this a usable ext4 fs?" is whether
`mount -t ext4` succeeds. Only fall through to `mkfs.ext4 -F` when
the mount truly failed; `-F` is safe at that point because there
is nothing to preserve on /dev/vda anyway.

Both branches verified by smoke-boot: existing-fs path mounts
cleanly with no mkfs noise, fresh-format path (qcow2 deleted)
runs `mkfs.ext4 -F`, mounts the new fs, and creates all four
Phase 1 directories under /var/lib/ (`dropbear nv-config nv-store
nv-system`).

### Milestone — Phase 1 tooling wired into the initramfs (2026-05-28)

The three Phase 1 crates scaffolded in the previous session
(`nv-pkg`, `null`, `nv-rebuild`) are now compiled by the Nix flake
and shipped inside the boot initramfs as standalone musl-static
binaries. The boot VM gains the full Phase 1 surface end-to-end:
`nv-pkg install` / `null eval` / `nv-rebuild switch`.

**New derivations:**

- `bootstrap/pkgs/null.nix` — `.null` CLI (1.1 MB stripped).
- `bootstrap/pkgs/nv-pkg.nix` — package manager (1.1 MB stripped).
- `bootstrap/pkgs/nv-rebuild.nix` — activation engine (1.6 MB stripped).

Each uses `pkgsStatic.rustPlatform.buildRustPackage`. Verified
`--version` runs on host; binaries are pure musl statics (no
`/lib/ld-musl-*` runtime dep, no `/nix/store` closure shipped).

**`bootstrap/pkgs/default.nix`** — exposes `nullLang`, `nv-pkg`,
`nv-rebuild` attrs (the `nullLang` name avoids the bare-`null` Nix
keyword clash; the binary on disk is still `null`). All three are
passed through to `initramfs` via `callPackage`.

**`bootstrap/pkgs/initramfs.nix`** — `cp`s the three binaries into
`/bin` alongside `zero`. The init script:

- Adds `/var/lib/nv-config/` to the persistent-`/var` mkdir set.
- Symlinks `/etc/nullvoid` → `/var/lib/nv-config/` on first boot so
  the agent-authored `system.null` survives reboots.
- Bootstraps an empty `generation-0/bin/` under
  `/var/lib/nv-system/` and points `current` at it, so
  `/run/current/bin` resolves to a real directory before the agent
  ever runs `nv-rebuild switch`. The first real generation will be
  `generation-1`.
- Creates `/run/` and symlinks `/run/current` →
  `/var/lib/nv-system/current` as specified in CONTRACTS §3.2.
- Bumps `PATH` to `/run/current/bin:/bin`, so a successful
  `nv-rebuild switch` immediately shadows the initramfs `/bin`.
- Banner now reports `null` / `nv-pkg` / `nv-rbld` versions.

**`pkgsStatic.rustPlatform.buildRustPackage` + `cargoLock.lockFile`
hit a crates.io regression** — the registry API endpoint
(`crates.io/api/v1/crates/<n>/<v>/download`) now returns HTTP 403
without a `User-Agent` header, and nixpkgs's `importCargoLock`-based
per-crate `fetchurl` doesn't set one. Switched to `cargoHash`, which
runs `cargo fetch` inside a fixed-output derivation — cargo's own
HTTP client sets a UA, the registry serves the bytes, and the
vendor tree is hashed as one FOD blob. Trade: a `cargoHash` line
per crate (set to `lib.fakeHash`, rebuild, paste the `got:` line
back) instead of an auto-derived hash from the lockfile. The
mechanism is the nixpkgs ≥25.05 default
(`useFetchCargoVendor = true` is implicit), no explicit flag
needed.

**Initramfs growth:** unchanged class — still 1.1 GB compressed.
The three binaries together add ~4 MB.

**Not yet wired and deferred to a future session:**

- A default `/etc/nullvoid/system.null` template — the agent is
  expected to author it the first time. Without a file, `nv-rebuild
  check` will refuse to evaluate; that's the intended starting
  point of the §5 demo flow.
- `examples/*.null` (v1-shaped) still present under `null/`, not
  yet migrated to v2.
- `null doctor` / `null fix --plan --json` (SPEC §6) still absent.

### Revised — Layer 3 language decision (2026-05-28, same day as lock)

The 2026-05-28 lock *"Layer 3 DSL is ZeroLang itself, no translator"*
has been revised the same day. Trigger: user surfaced that ZeroLang
is a systems programming language (`mut`, `set`, `World`, generics,
ownership, native codegen — Rust/Zig family), and forcing it into the
system-declaration role is a category error analogous to writing
NixOS configurations in Rust.

**New decision:** Layer 3 DSL is `.null` — a separate, deliberately
tiny, Nix-shaped declarative language that **inherits ZeroLang's
agent-first tooling recipe** (typed JSON diagnostics, repair IDs,
embedded skills bundle, single-form-per-concept, capability-explicit
syntax) transposed to the configuration domain. ZeroLang remains the
implementation language for layers 1-2 and layer 4 apps.

Authoritative spec: `bootstrap/system/null/SPEC.md` (new). DESIGN.md
section *"Layer 3 language model"* rewritten in place; the original
locked text is preserved verbatim in a `History` callout so the
reasoning trail stays visible. The mental-model NixOS-analogy table,
the layer-3 ASCII box, the *Language choice — ZeroLang* section, and
the *Open design questions* are all updated to match.

The CONTRACTS.md §2 sketch from the previous session (which had
already drifted to a separate `.null` language without flagging the
contradiction with the same-day lock) is now formally superseded by
SPEC.md v2.

### Migration — `.null` crate v1 → v2 (same session)

`bootstrap/system/null/` migrated to implement SPEC v2 in 8 steps,
each verified by build + smoke test before moving on:

1. **Lexer.** Added `Bang` (`!`) token. Symbol (`.identifier`) and
   capability (`!ident(.ident)*(."str")?`) literals are assembled at
   parse time from the dumb token stream, so the lexer stayed minimal.
2. **AST.** Added `Expr::Symbol { name, span }` and
   `Expr::Capability { path, arg, span }`. Field-access continues to
   work after a leading `Ident`; standalone `Dot` / `Bang` at the
   start of an expression now route to the new parsers.
3. **Parser.** `parse_symbol` and `parse_capability` added. Existing
   anti-feature detection (`let`, `if`, `import`) preserved. A
   pre-existing CLI bug (global `--json` clashing with `parse --json`)
   surfaced and was fixed by dropping the dual-mode toggle (v2 NDJSON
   is the default, in line with SPEC §6).
4. **Schema / types.** `types.rs` rewritten. `SystemManifest` gains
   `caps: [Capability]`; `Service` gains `requires: [Capability]` and
   `restart` becomes the enum-as-symbol `RestartPolicy`. The capability
   vocabulary from SPEC §5.5 is hardcoded in `known_capability()`.
   Subset rule enforced: every `service.requires` ⊆ `system.caps`
   (CAP004 with `repair = add-system-cap` if violated).
5. **Diagnostics.** Full rewrite to SPEC §7 shape: NDJSON on stderr,
   structured `expected` / `actual` / `span: SpanInfo` /
   `repair: Option<Repair>` (typed `id` + `args` JSON value). Stable
   error-code namespaces materialised: `PAR001`, `TYP001`, `TYP004`,
   `SCH001`, `REF002`, `CAP001`, `CAP004`. Initial repair-ID set from
   SPEC §7.3 wired in: `wrap-int-as-string`, `add-system-cap`,
   `fix-enum-symbol`, `add-required-field`, `quote-bare-identifier`,
   `homogenize-list`, `remove-unknown-field`.
6. **CLI `explain`.** `null explain <CODE>` and `null explain list`
   added. Per-code docs embedded as const strings in `src/explain.rs`
   — agent can recover the meaning of any diagnostic from the binary
   alone, no network access (SPEC §1).
7. **Skills bundle.** Six markdown documents at
   `bootstrap/system/null/skills/` (`null`, `language`, `schema`,
   `caps`, `cli`, `diagnostics`) embedded via `include_str!` and
   served by `null skills list` / `null skills get <name>`. This is
   the version-matched-skills-bundle property that lets an agent that
   has never seen `.null` author a correct `system.null` from the
   binary alone (SPEC §8).
8. **Test migration.** All 50 v1 integration tests adapted to v2
   schema (added `caps = []` to every `SystemManifest` test string,
   `requires = []` to every Service, `restart = .always` symbols
   instead of `"always"` strings, `err.span.line`/`err.repair`
   instead of `err.line`/`err.fix`). 3 example-file-bound tests
   deleted (the `examples/*.null` files are still v1-shaped — they
   parse but no longer typecheck under v2; restoration deferred).
   2 new schema-missing tests and 4 new capability tests added.
   **Final result: 53 tests passing, 0 failures.** Verified end-to-end
   with smoke files covering CAP001 (unknown cap), CAP004 (subset
   violation), TYP004 (string-instead-of-symbol restart). Each error
   now carries a typed `repair.id + args` payload an agent can apply
   without parsing prose.

Known gaps deferred to a future session:
- `null doctor` and `null fix --plan --json` (SPEC §6) not
  implemented.
- The `examples/*.null` files are still v1 shape; restore them or
  drop the dir.
- The v1 `./path` lexer shortcut is still accepted even though SPEC
  v2 §3.1 doesn't list it — either ban it in v2.1 or add it to SPEC.

### Added

- `bootstrap/system/null/SPEC.md` — authoritative spec for `.null`
  v2. Twelve sections including the five-tricks-transposed rationale,
  the anti-feature list, surface syntax, the `SystemManifest` schema,
  the capability-as-value system, CLI surface mirroring Zero's
  (`null explain`, `null skills`, `null fix --plan --json`),
  diagnostic NDJSON format with stable error-code namespaces
  (`PAR`/`TYP`/`SCH`/`REF`/`CAP`) and a closed initial repair-ID set,
  reference `system.null` example, and what the existing Rust crate
  owes to reach v2.

- **Layer 3 language model — Zero native, no translator** section in
  `DESIGN.md`. Decision locked: the system description language is
  ZeroLang itself; no separate DSL, no runtime module system bolted on
  top.
- **Mental model — how the layers relate** table in `DESIGN.md` mapping
  NixOS constructs to NullVoidOS equivalents (Nix language ↔ ZeroLang,
  module system ↔ static types, `/nix/store` ↔ CAS substrate, etc.).
- **Substrate ↔ Zero boundary** section in `DESIGN.md` explaining the
  per-package Zero wrapper pattern (`substrate/openssl.zero` over
  `libcrypto.so` via FFI; capability annotations enforced at Zero
  boundary).
- **Open design questions** section in `DESIGN.md` listing four pieces
  deferred to Phase 2: module shape, composition semantics,
  `SystemManifest` schema, activation capability primitives.
- `CLAUDE.md` at repo root scoping behaviour for this branch
  (workflow, communication conventions, phase awareness).
- `bootstrap/CHANGELOG.md` (this file).
- `bootstrap/flake.lock` generated by first `nix develop ./bootstrap`.
  Pins `nixpkgs` to `64c08a7` (2026-05-23) and `flake-utils` to
  `11707dc` (2024-11-13).

### Changed

- `bootstrap/flake.nix`: busybox source switched from cross-compiled
  dynamic (`pkgsMusl.busybox`) to fully static (`pkgs.pkgsStatic.busybox`).
  Verified: `statically linked`, `ldd` reports "not a dynamic executable".
  Rationale: initramfs cpio should not depend on shipping `ld-musl` as a
  runtime interpreter.
- `bootstrap/flake.nix`: dev shell now includes `zerolang` derivation
  (callPackage from `bootstrap/pkgs/`). Shell hook prints `zero --version`.
- `bootstrap/pkgs/zerolang.nix` added — ported from `nix-rewrite` branch
  (commit `d903bae`). Multi-platform derivation fetching Vercel's
  release binaries for v0.1.4 with SHA256 pinned (linux-musl-x64/arm64,
  darwin-x64/arm64). Verified: `zero --version` → `zero 0.1.4`. Binary
  itself is statically-linked musl ELF, ready to drop into the initramfs.
- `bootstrap/pkgs/default.nix` added — package set entry point for
  `callPackage` extensibility (next additions: llama.cpp, substrate
  wrappers).
- `bootstrap/pkgs/kernel.nix` added — minimal Linux 6.6.141 LTS kernel
  derivation. Starts from `make tinyconfig`, adds curated options via
  `scripts/config`, reconciles with `make olddefconfig`. Targets
  x86_64, VirtIO paravirt, serial ttyS0 console, basic TCP/IP, no
  modules. Build time on host: ~46 s after tarball cached. Result:
  `bzImage` **1.5 MB**, `.config` 52 KB. Verified: boots in QEMU
  through to "No working init found" panic — expected (initramfs is
  task #7). Includes `patchShebangs scripts` workaround for the Nix
  build sandbox.
- `bootstrap/pkgs/default.nix`: exposes `kernel` alongside `zerolang`.
- `bootstrap/pkgs/initramfs.nix` added — Phase 0 variant (d) initramfs.
  Assembles cpio.gz from: static-musl busybox + ~40 standard symlinks,
  the static `zero` binary, and an `/init` sh script that mounts
  `/proc /sys /dev`, prints kernel/hostname/zero versions, and drops
  to a busybox shell. Built via `runCommand` with `cpio` + `gzip` from
  nativeBuildInputs. Final size: **1.2 MB compressed**.
- `bootstrap/pkgs/default.nix`: exposes `initramfs`, wires it to the
  in-tree `zerolang` via `inherit (self) zerolang`.

### Milestone — Phase 0 boot pipeline alive (variant d)

End-to-end QEMU boot succeeds: SeaBIOS → kernel (1.5 MB bzImage) →
initramfs (1.2 MB cpio.gz) → `/init` → busybox shell. `zero --version`
runs inside the VM and prints `zero 0.1.4`. No AI in the loop yet —
this proves the kernel + initramfs + userland pipeline before adding
the agent backend.

Outstanding for full Phase 0 demo:
- Variant (a): Claude Code CLI inside the VM, consuming the user's
  Claude Max subscription. Plan in `bootstrap/PHASE0_A_PLAN.md`.
  Decisions locked: Node.js via `pkgsMusl.nodejs_22`, credentials
  passed in via 9P read-only mount of host's `~/.config/claude/`,
  init drops to busybox shell and user types `claude` manually.
  Delete the plan file when (a) ships.
- Cosmetic: `can't access tty; job control turned off` warning from
  busybox sh. Fix later with `setsid cttyhack`.

### Changed (within session)

- Replaced operational plan `PHASE0_B_PLAN.md` with `PHASE0_A_PLAN.md`.
  Reason: variant (b) called Anthropic API directly per-token, while
  user pays a Claude Max subscription that covers Claude Code usage.
  (b) would be double-paying. (a) reuses the subscription. See memory
  `feedback_claude_subscription`.
- `bootstrap/flake.nix`: added `apps.boot-vm` for one-command interactive
  boot of the Phase 0 (d) VM. Usage: `nix run ./bootstrap#boot-vm` or
  the shorter `nix run ./bootstrap` (alias as `apps.default`). Wraps
  `qemu-system-x86_64` with the kernel + initramfs derivations baked
  in; exit with `Ctrl-A x`.

### Milestone — Phase 0 (a) alive: Claude Code inside the VM

End-to-end boot of variant (a) succeeds. Final banner:

```
kernel:   Linux 6.6.141
zero:     0.1.4
claude:   2.1.148 (Claude Code)
IP:       10.0.2.15/24
creds:    yes  — backups cache debug ...
```

The VM mounts the host's `~/.claude/` directory over 9P/virtio at
`/root/.claude/` (read-only), brings up `eth0` via DHCP through QEMU
user networking, and `claude --version` runs the upstream Node-based
Claude Code CLI from inside the initramfs.

Artifact sizes:

- `bzImage`: 1.7 MB (unchanged class — added options are a few KB each)
- `initramfs.cpio.gz`: **100 MB** (up from 1.2 MB in variant (d); the
  entire `claude-code` Nix closure of ~312 MB uncompressed across 30
  store paths now lives under `/nix/store` inside the initramfs)

Interactive verification (manual, after `nix run ./bootstrap`):

- Send a prompt, confirm the Max-subscription token is consumed (and
  not a per-token API key).
- Send "create a file /tmp/test.txt with content hello", confirm tool
  use writes the file inside the VM.
- Stretch: ask `claude` to write a small Zero program and execute
  `zero run` on it.

### Changed (Phase 0 (a) work)

- `bootstrap/pkgs/kernel.nix`: enabled `VIRTIO_MENU` (parent Kconfig
  gate for `VIRTIO_PCI` / `VIRTIO_BLK` / `VIRTIO_CONSOLE`), `PCI_MSI`
  (required by modern virtio transports), `NET_9P`, `NET_9P_VIRTIO`,
  `9P_FS`, `9P_FS_POSIX_ACL`. Also added the userspace runtime block
  needed by modern glibc + Node.js: `FUTEX`, `EVENTFD`, `SIGNALFD`,
  `TIMERFD`, `EPOLL`, `INOTIFY_USER`, `AIO`, `POSIX_MQUEUE`,
  `PREEMPT_VOLUNTARY`. Without these, `claude --version` aborts with
  "futex facility returned an unexpected error code" and the virtio
  devices stay unbound (symptom: "no channels available for device
  claudefs"). bzImage stayed at 1.7 MB.
- `bootstrap/pkgs/initramfs.nix`: Phase 0 (a) initramfs. Ships the
  full `claude-code` Nix closure (30 paths) under `/nix/store`,
  symlinks `/bin/claude` to the wrapper, plus the `cacert`
  `ca-bundle.crt` at `/etc/ssl/certs/` and a hand-rolled
  `udhcpc/default.script` (busybox's bundled one hardcodes nix-store
  paths to its own bin/, useless inside the initramfs). Init script
  now mounts the 9P share, runs `udhcpc -i eth0`, exports
  `NODE_EXTRA_CA_CERTS=/etc/ssl/certs/ca-bundle.crt`, and prints a
  variant-(a) banner with `claude --version` and the credentials
  listing.
- `bootstrap/pkgs/default.nix`: passes `claude-code` and `cacert`
  through to `initramfs` via `callPackage`.
- `bootstrap/flake.nix`: imports nixpkgs with
  `config.allowUnfree = true` (claude-code is unfree-licensed).
  `apps.boot-vm` extended for variant (a): preflight-checks
  `~/.claude/.credentials.json` on the host, mounts that directory as
  a read-only 9P share (`mount_tag=claudefs`), attaches a virtio user-
  network NIC, bumps memory to 1 GB, and adds `-cpu max` so the AVX2
  /  BMI2 instructions the nixpkgs glibc + bundled Node require don't
  trap as "Illegal instruction" on QEMU's default `qemu64` CPU.

### DESIGN.md

- Added "Phase 0 (a) — documented deviation: glibc in the bootstrap
  initramfs" under §Phase 0 decisions. Documents the trade-off
  (ship the full claude-code Nix closure + `/nix/store` as the
  CAS-of-convenience for now) and the revisit condition.

### Removed

- `bootstrap/PHASE0_A_PLAN.md` — superseded by this milestone entry.

### Fix — Phase 0 (a) interactive login

First interactive run blocked at `claude` startup with:

```
Claude configuration file not found at: /root/.claude.json
A backup file exists at: /root/.claude/backups/.claude.json.backup.<ts>
```

Root cause: Claude Code splits its on-disk state in two — `~/.claude/`
(credentials, history, cache) and `~/.claude.json` (config: project
trust, model prefs, MCP servers). Our 9P share only exposes the
directory, so the JSON config file at `$HOME/.claude.json` is missing
inside the VM and `claude` refuses to start. The on-screen rescue
command Claude prints (`cp …backup.<ts> ~/.claude.json`) gets line-
wrapped to 80 cols on the serial console, which is what made it look
like an OAuth `Missing code_challenge` problem.

Fix in two pieces:

- `bootstrap/pkgs/initramfs.nix`: init now seeds `/root/.claude.json`
  from the newest backup under `/root/.claude/backups/` on every boot.
- `bootstrap/flake.nix` and `initramfs.nix`: drop `readonly=on` from
  the 9P share so claude-code can refresh the OAuth token in-place.
  This is the trade documented as Phase 0 plan contingency 5 — the VM
  may now mutate the host's `~/.claude/` (token refresh + history
  writes). Accepted for Phase 0; a multi-tenant separation comes later.

### Performance + TTY polish (after first interactive Claude session)

User confirmed Phase 0 (a) works interactively (`claude` answers
prompts inside the VM), but flagged two issues:

- `claude` writes responses slowly — the boot-vm app was running in
  TCG (software emulation), which has to interpret every AVX2/BMI2
  instruction emitted by the nixpkgs glibc + bundled Node.js.
- Hitting `Ctrl-C` inside the Claude TUI left the serial terminal
  wedged in raw mode; the user had to close the host terminal window
  to recover. Symptom of the earlier `can't access tty; job control
  turned off` warning — the shell had no controlling tty, so signals
  bypassed its line discipline.

Fixes:

- `bootstrap/flake.nix` (boot-vm app): probes `/dev/kvm` at runtime
  and switches to `-accel kvm -cpu host` when usable (still falls
  back to `-accel tcg -cpu max` if not). KVM brings Node.js workloads
  back to native speed.
- `bootstrap/pkgs/initramfs.nix`: respawn loop now launches the
  shell as `setsid cttyhack /bin/sh` instead of bare `/bin/sh`. The
  `cttyhack` busybox applet grabs the first available tty
  (`/dev/console` here) and sets it as the controlling terminal, so
  job control works and TUIs that catch `SIGINT` (Claude Code's Ink
  UI) can restore the terminal cleanly on exit.

### Fix — Phase 0 (a) tool-use blocked without bash

User feedback after first real `claude` session: agent runs but
"cannot do commands". Claude Code invokes its Bash tool through
`bash -c "<cmd>"` rather than `sh -c`, and the initramfs only
shipped busybox ash at `/bin/sh`. No `/bin/bash` → every Bash
tool-use call fails (silently or with `ENOENT`).

- `bootstrap/pkgs/initramfs.nix`: closure root now includes `bash`
  alongside `claude-code`. Symlinks `/bin/bash` to the GNU bash
  wrapper and `/usr/bin/env -> /bin/env` (canonical shebang path,
  busybox's `env` applet covers the binary side).
- `bootstrap/pkgs/default.nix`: passes `bash` through to initramfs.
- Closure size impact: `+2 MB` compressed (47 MB uncompressed
  bash closure shares almost everything — glibc, ncurses, readline
  — with the already-shipped claude-code closure).

`bash --version` reports `5.3.9(1)-release` inside the VM; tool-use
should now reach a real GNU bash. Git, ripgrep, etc. are still
absent — add as needed when Claude reports the next missing tool.

### Milestone — Phase 0 (a) lab edition + Phase 1 components scaffolded

The project reframed mid-day, from "rewrite NixOS in Zero" (judged not
a real thesis) to a research lab for the question **"can an agent
author a working OS end-to-end?"** — with a path to a small specialised
model (NullAgent) eventually replacing the big general one on the
governance side. See DESIGN.md for the new framing.

Two parallel deliverables landed in this session:

**1. Lab substrate.** The initramfs now ships a developer toolchain
big enough for the agent to compile and package real software:
python313, rustc+cargo, nodejs_22, gcc, make, git, curl, jq, ripgrep,
fd, neovim, sqlite, GNU coreutils. Added dropbear (SSH server) and
e2fsprogs (ext4). A persistent `/var` on a qcow2 disk auto-provisioned
under `$XDG_CACHE_HOME/nullvoid/var.qcow2` (8 GB sparse) survives
reboots. Host SSH pubkey shared via 9P, dropbear authorized_keys
populated at boot, port 22 forwarded to host:2222. VM RAM bumped
1 GB → 8 GB (needed for compiling Rust + running Node + LLM in-VM).
Compressed initramfs grew 100 MB → 1.1 GB.

Kernel additions: `BLOCK`, `BLK_DEV`, `VIRTIO_BLK`, `EXT4_FS`. The
tinyconfig base disables CONFIG_BLOCK, which silently masks every
block driver and filesystem we tried to `--enable`; turning BLOCK on
first unblocks the rest. bzImage 1.7 MB → 2.0 MB.

`/etc/{passwd,group,shadow}` minimal stubs so dropbear's getpwnam
lookup for `root` succeeds.

**2. Phase 1 components built by 3 sub-agents in parallel.** Locked
the contracts in `bootstrap/system/CONTRACTS.md` so the three could
work without colliding. Results:

- `bootstrap/system/nv-pkg/` (Rust crate, package manager per
  CONTRACTS §1). 11 integration tests green. Install / resolve /
  list / remove / verify. Tarball-hash addressing in the store path,
  separate content-hash file for tamper detection.
- `bootstrap/system/null/` (Rust crate, configuration language per
  CONTRACTS §2). 50 integration tests green. Hand-rolled lexer +
  recursive-descent parser + single-pass typecheck/eval against the
  SystemManifest schema. `check` / `eval` / `fmt` / `parse --json`.
  Diagnostics with PAR/TYP error codes.
- `bootstrap/system/nv-rebuild/` (Rust crate, activation engine per
  CONTRACTS §3). 9 integration tests green. Atomic `rename(2)`-based
  symlink swap. `check` / `build` / `switch` / `rollback` /
  `generations`.

Each crate is self-contained, targets `x86_64-unknown-linux-musl`
when shipped. Not yet wired into the initramfs — that integration is
the next session's work.

### Polish (from first interactive session)

- `bootstrap/pkgs/initramfs.nix`: replaced the hand-curated busybox
  symlink list with auto-enumeration via `busybox --list`. Now every
  applet busybox was compiled with (~400, including `whoami`, `date`,
  `dmesg`, `vi`, `wget`, ...) gets a symlink in `/bin`. Also created
  `/root` and `/etc` dirs preemptively for variant (a).
- `bootstrap/pkgs/initramfs.nix`: init script now runs `dmesg -n 1`
  early to silence late kernel info-level messages that were leaking
  into the shell prompt during the first interactive session.
- `bootstrap/flake.nix` (boot-vm app): added `quiet` to the kernel
  cmdline. Suppresses boot info-level messages from the console.
- `bootstrap/pkgs/initramfs.nix` (init): replaced `exec /bin/sh` with
  a respawn loop. Typing `exit` (or any accidental shell death) no
  longer kills PID 1 and panics the kernel. Banner now suggests
  `poweroff` as the in-VM exit command (busybox applet, signals the
  kernel; QEMU `-no-reboot` makes that translate into a clean QEMU
  exit), with `Ctrl-A x` as the host-side fallback.
- `bootstrap/pkgs/kernel.nix`: enabled `CONFIG_ACPI`, `CONFIG_ACPI_BUTTON`,
  `CONFIG_ACPI_PROCESSOR`, `CONFIG_PNP`, `CONFIG_PNPACPI`. Without
  ACPI, busybox `poweroff` puts the kernel in halt but QEMU keeps
  running (the VM appears frozen). With ACPI, `poweroff` issues S5
  power-off, QEMU detects it and exits cleanly. bzImage grew from
  1.5 MB → 1.7 MB.

## 2026-05-28

### Added

- Branch `lfs-bootstrap` created off `main`.
- `bootstrap/README.md` and `bootstrap/DESIGN.md` scaffolded
  (commit `6759b53`).
- **Phase 0 decisions locked** in `DESIGN.md` (commit `5cd9531`):
  - libc → musl
  - init → sh-based custom
  - Agent backend → pluggable (default Claude Code)
  - Build env → Nix cross-compile on host
  - VM image → initramfs + qcow2 `/var`
  - Kernel → vanilla Linux LTS, minimal `.config`
- `bootstrap/flake.nix` cross-compile dev shell (commit `5cd9531`).
- Graphical UI decision **deferred** until Phase 0-1 base is booting.
  Provisional preference: browser-as-desktop kiosk (option b) once
  revisited. Documented in `DESIGN.md` (commit `7081559`).
