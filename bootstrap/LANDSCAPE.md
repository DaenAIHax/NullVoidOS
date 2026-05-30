# NullVoidOS — Landscape & Posizionamento

> Materiale di ricerca per blog/paper. Analisi del paesaggio: cosa esiste
> già là fuori, chi lo fa, come, e dove sta (o non sta) il contributo
> proprio di NullVoidOS. Tutte le fonti sono citate in fondo.
>
> Raccolto il 2026-05-30. Le date e le versioni sono quelle note a quella
> data; verificarle prima di pubblicare.

## La domanda

Cosa esiste già che fa una cosa simile a NullVoidOS? Se esiste, come lo
fanno? Perché non usano questo approccio — è una cosa che gli esperti
hanno già provato e bocciato? Se non esiste, perché?

Risposta breve: **ogni singolo mattone esiste ed è area calda di ricerca
industriale.** Nessuno l'ha bocciato — il contrario, ci stanno buttando
team e budget. Quello che non si trova è la *combinazione verticale
completa* come un'unica cosa.

## Lo stack, smontato in quattro layer

NullVoidOS non è un blocco monolitico: è la sovrapposizione di quattro
filoni distinti. Per ognuno, chi lo persegue e come.

### Layer 1 — Linguaggio progettato per agenti (= Nullang)

Idea: diagnostica machine-readable (JSON, codici errore stabili), effetti
e capability dichiarati nelle firme, output del compilatore pensato per
essere consumato da un agente, non da un umano.

- **Zero / Vercel Labs** (rilasciato 2026-05-15, estensione `.0`,
  v0.1.x, sperimentale). Linguaggio di sistema che compila a binari
  nativi <10 KiB. Diagnostica JSON con codici stabili (es. `NAM003`) e
  `repair-ID` tipati che l'agente può matchare senza parsare testo.
  Effetti espliciti via capability-based I/O nelle firme, enforced a
  compile time. `ProgramGraph`: struttura semantica navigabile a slice
  invece del sorgente grezzo. **È la stessa identica tesi di Nullang,
  fatta da un team pagato.** (Già citato nel prodotto NullVoid.)
- **Pel** (arXiv 2505.13453, aprile 2025). Linguaggio per orchestrare
  agenti, ispirato a Lisp/Elixir/Gleam/Haskell. Punto chiave: *capability
  control a livello di sintassi*, esplicitamente per "eliminare la
  necessità di sandbox complesse". Cioè la tesi "l'effetto-linguaggio È la
  capability", scritta in un paper.

### Layer 2 — Agente che estende il proprio compilatore / i propri tool (= self-improvement loop)

Idea: l'agente sbatte contro un limite del *suo stesso strumento*, lo
estende, si ricompila, riprova.

- **SICA — Self-Improving Coding Agent** (arXiv 2504.15228, ICLR 2025
  SSI-FM workshop). Parte equipaggiato solo con "sovrascrivi file"; da
  solo implementa tool di edit a diff/range e un localizzatore di simboli
  AST-based. Meccanismo di apprendimento non-gradient, guidato da
  riflessione LLM + update di codice. Gain 17%–53% su subset di SWE-bench
  Verified. **È letteralmente il "fabbro che si forgia il martello".**
- **Darwin Gödel Machine — Sakana AI + UBC (Jeff Clune) + Vector
  Institute** (maggio 2025). Agente che riscrive il *proprio codice
  sorgente*; lignaggio evolutivo di varianti, selezione per fitness
  empirica, archivio. SWE-bench 20.0%→50.0%, Polyglot 14.2%→30.7%, in
  autonomia. Nota di sicurezza: in alcuni casi ha hackerato la reward e
  fabbricato log falsi → serve cautela. **È il self-host/self-improvement
  nella forma più estrema già pubblicata.**
- **Voyager** (arXiv 2305.16291, 2023). Skill-library di codice
  eseguibile che cresce da sola; self-verification con un secondo GPT-4
  critico; refinement iterativo da errori di esecuzione. **Il pattern
  "agente che accumula capability come codice" ha quasi tre anni.**

### Layer 3 — Confinamento via capability del kernel (= Traccia A, le 4 capability)

Idea: policy applicata da Landlock (filesystem, path) + seccomp-bpf
(syscall), con un supervisore per le decisioni runtime-dependent.

- **Sandlock** (arXiv 2605.26298). Split-enforcement: policy statica
  compilata in Landlock + seccomp-bpf; supervisore seccomp-notification
  per decisioni runtime ed effetti virtualizzati. **È, layer per layer,
  identico alla Traccia A di NullVoidOS** (netns, Landlock, seccomp,
  USER_NOTIF).
- **OpenAI Codex** — usa Landlock + seccomp; è l'unico agente major con
  sandbox abilitata di default.
- Misura empirica citata in letteratura: i meccanismi kernel (capabilities,
  seccomp, MAC) bloccano ~67.6% degli attacchi di privilege escalation vs
  ~21.6% di namespace/cgroup da soli.

### Layer 4 — OS dichiarativo che converge allo stato voluto (= visione "tutto in VM")

Idea: l'utente dichiara il *risultato*, il sistema riconcilia la realtà
verso quello stato.

- **AIOS** (Rutgers, COLM 2025). LLM come kernel dell'OS, agenti come
  applicazioni. Il tentativo accademico più rigoroso di "agent-native OS".
- **Snap declarative agent standard** — stato desiderato dichiarato +
  riconciliazione continua, modello esplicitamente Kubernetes.
- **NixOS** — il riferimento dichiarativo già noto all'autore; richiede
  però moduli scritti a mano in un linguaggio già finito (Nix).

## Verdetto sul posizionamento

1. **"Perché non usano il mio approccio?"** — Lo usano. Tutti e quattro i
   layer sono attaccati da Vercel, OpenAI, Sakana, UBC, Rutgers, Snap.
2. **"È inutile, l'hanno bocciato?"** — No. È area calda con investimenti
   reali. Nessuna evidenza di bocciatura.
3. **"È utile / è mio?"** — Ogni *mattone* singolo è già preso. Quello che
   NON si trova in letteratura è la **combinazione verticale completa come
   una cosa sola**: un linguaggio dove l'effetto-è-la-capability, *dentro*
   un OS dichiarativo, dove l'agente *estende il proprio linguaggio
   nativo* per raggiungere lo stato voluto, il tutto confinato dal kernel.
   I lab attaccano **un layer ciascuno a scala industriale**; NullVoidOS
   li **impila tutti, a scala hobby**.

La tesi difendibile non è "ho inventato X" (ogni X esiste). È:
**l'integrazione verticale di quattro filoni che gli altri perseguono
separatamente, e la proprietà emergente che ne deriva — il confinamento è
"gratis" perché vive nel linguaggio, non bolt-on.**

## Asticelle (non confondere)

Lo stesso lavoro vale cose diverse a seconda del frame:

- **Ricerca da difendere** → asticella alta: reggere il confronto con
  Sakana/Vercel, che hanno team e benchmark veri.
- **Hobby per capire impilando con le mani** → asticella: insegna qualcosa
  e diverte. Cullis resta la scommessa commerciale.
- **Materiale per distribuzione/reach (blog, paper, video)** → asticella:
  comunica bene un'idea vera a un pubblico. Gioco ortogonale alla
  profondità tecnica.

Il senso di smarrimento ("cosa dimostro?") nasce dal misurarsi con
l'asticella-ricerca mentre si vive su un frame diverso.

## Fonti

- Vercel Zero — https://github.com/vercel-labs/zerolang ·
  https://www.marktechpost.com/2026/05/17/vercel-labs-introduces-zero-a-systems-programming-language-designed-so-ai-agents-can-read-repair-and-ship-native-programs/
- Pel: A Programming Language for Orchestrating AI Agents —
  https://arxiv.org/pdf/2505.13453
- SICA: A Self-Improving Coding Agent — https://arxiv.org/abs/2504.15228
- Darwin Gödel Machine (Sakana AI) — https://sakana.ai/dgm/
- Voyager: An Open-Ended Embodied Agent with LLMs —
  https://arxiv.org/abs/2305.16291 · https://voyager.minedojo.org/
- Sandlock: Confining AI Agent Code with Unprivileged Linux Primitives —
  https://arxiv.org/html/2605.26298v1
- AIOS / agent-native OS landscape —
  https://medium.com/@marc.bara.iniesta/who-is-building-the-agent-native-operating-system-c6bae5a5a3f5
- Snap declarative agent standard — https://eng.snap.com/agent-format
