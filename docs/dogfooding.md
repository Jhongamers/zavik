# Dogfooding — DevForge no DevForge (17/10/2026)

Diretriz do fundador: usar o DevForge neste repositório (projeto real)
"onde der" — antes de seguir a fase 2 do i18n. Este repo é um workspace
virtual Cargo (7 crates), exatamente o formato que o produto precisa
servir. Registro honesto: o que a sessão encontrou, o que foi corrigido
(agora com teste) e o que ficou pendente.

## Comandos usados no repo

`detect`, `env --json`, `analyze` (raiz e `crates/cli`), `audit`,
`run --dry-run`, `context --json`, `up --dry-run --json`, `data status`,
`license status`, `diagnose` (com um erro de compilação real gerado num
crate temporário), mais `cargo run` cru pra validar a resolução de start.

## Achados

### Corrigidos nesta sessão (cada um com teste)

1. **Workspace virtual invisível** (severidade ALTA — era o comando wedge):
   apontar na raiz mostrava 1 stack "Dependências: 0" e o `audit` dizia
   "Pacotes auditados: 0 … Veredito: APROVADO ✅" — aprovar **zero** pacotes
   atesta nada (viola o princípio anti-false-positive).
   **Correção:** o detector Rust expande `[workspace].members` (literais e
   `*`, respeitando `exclude`) em um stack por membro, com dedup contra a
   descoberta recursiva de manifests. Neste repo real: **1 → 7 stacks,
   0 → 35 pacotes auditados**.
2. **Versões herdadas perdidas**: dependências `dep = { workspace = true }`
   saíam com `version: None` — as versões vivem no `[workspace.dependencies]`
   da raiz. **Correção:** resolução pelo `[workspace.dependencies]` mais
   próximo (o manifesto próprio primeiro, depois ancestrais). Sem raiz →
   `None` honesto (nunca inventa). Neste repo: **19/35 deps com versão**
   (os path-deps internos `devforge-* = { path = … }` ficam `None` —
   correto: pacote path não tem versão de registro).
3. **`diagnose` não reconhecia `error[E0308]`** — o erro de compilação
   Rust mais comum — e caía no fallback genérico.
   **Correção:** regra `rustc-error` (`error[E0…]` / `error: could not
   compile`), prioridade 21: **abaixo** das regras de toolchain (25/30)
   porque "could not compile" também aparece quando só falta o linker — a
   causa real precisa vencer o sintoma. Teste cobre os dois lados.
4. **`audit` com 0 pacotes**: humano agora avisa "⚠ 0 pacotes auditados —
   não há o que atestar" e o vira **"Veredito: SEM DADOS ⚠️"** (em vez de
   APROVADO). JSON **intocado** — `packages_audited` já era visível e
   `passed`/exit codes mantidos: contrato de máquina estável.

### Registrados, ainda pendentes

5. ~~`context --json` → `entrypoints: []`~~ **FECHADO (17/10, corpus
   Fase A)**: era artefato da raiz virtual **pré-expansão** (1 stack, sem
   `src/` na raiz). O detector já cobre `src/main.rs` (`briefing.rs`);
   pós-fix o context do próprio DevForge lista 7 entrypoints incl.
   `crates/cli/src/main.rs`. Verificado também em projeto externo: só
   lista o que existe (serde omitiu um layout fora do padrão, sem inventar).
6. **Prose dentro de JSON está em pt-BR** (ex.: `diagnose.summary`).
   O contrato é "JSON sempre inglês": chaves/enums já são EN; a prose que
   máquina lê deve usar EN **forçado** (`t_en()` — tradução direta do
   catálogo, independente do `--lang`) — decisão registrada pra fase 2 do
   i18n.
7. Expansão de workspace é **Rust-only** nesta versão: monorepo com
   JS/Python na raiz continua não expondo membros (honestidade no doc
   comment do detector).
8. `diagnose` em saída de `cargo bench` caiu no fallback com top_errors
   ruins (linhas "ignored") — o fallback é honesto, mas dá pra cobrir
   `no bench targets`/`error: no tests` (futuro, baixa prioridade).

### Achados extras — corpus Fase A/B (17/10, mesmo dia)

9. **`run`/`up`/runbook/hint do prepare sugeriam `cargo run` em projeto
   só-lib (NOVO-1)**: serde e anyhow (reais, SHA pinado) têm **zero bins**
   (ground truth: `cargo metadata`) — o comando falharia. **Corrigido**:
   a resolução Rust conta binários do pacote **e dos membros do
   workspace** (`[[bin]]`, `src/main.rs`, `src/bin/*`); zero → `NotFound`
   honesto ("Não achei como iniciar"), `runbook.run` e hint do prepare
   ficam em silêncio; workspace com **1 único bin** segue `cargo run` (o
   caso do DevForge — verificado sem regressão). Testes: runner ×4,
   briefing ×1, provision ×2. Detalhes: `~/devforge-corpus/pagina-interna.md`.
10. **Runbook Go prometia `go run .` sem `main.go` (NOVO-2)**: cobra (real,
    Fase B) é lib Go — o `run` já respondia NotFound, mas o
    `context.runbook.run` mandava `go run .`, que falharia. **Corrigido**:
    o runbook Go só promete `go run .` com `main.go` na raiz, ou
    `go run ./cmd/<único>` com um único pacote cmd; zero/vários → silêncio
    (o `run` pergunta). Teste ×1. Candidatos da mesma classe, ainda sem
    prova real: runbook C#/Dart/Swift/Elixir (JS e Python já condicionais).
11. **Runbook Dart prometia `dart run` sem `bin/*.dart` e swift pedia
    `swift run` em qualquer `Package.swift` (NOVO-3 e NOVO-4)**: args
    (real, lib pura) e swift-argument-parser (real, **9 executáveis** —
    `swift run` puro falharia pedindo o produto) confirmaram os dois.
    **Corrigidos**: runbook Dart condicional a `bin/*.dart` (gate
    compartilhado com o runner); runner Swift conta produtos
    `.executableTarget/.executable(name: …)` — zero → NotFound, um →
    `swift run`, vários → Ambiguous numerado (mostrou os9 reais); runbook
    Swift só promete com exatamente um. Testes: runner ×3, briefing ×1.
    Item 10 resolvido na prática: JS/Python/Dart/Go/Swift/Rust já são
    condicionais; **C# e Elixir seguem como candidatos sem prova real**
    (newtonsoft: raiz sem `.sln`/`.csproj` → stacks0 = comportamento
    documentado `--all`, não FP; observação registrada: o aviso de
    "nenhuma stack" poderia sugerir `--all`).

### Comportamentos verificados OK (sem achado)

- `env --json`: os logs do KERNEL vão pra **stderr**; stdout é JSON puro.
- Resolução do `run` em manifesto virtual → `cargo run` correto (workspace
  tem um único bin).
- `context` runbook (build/lint/run/setup/test) + key_files (CI, lockfile,
  manifest, README) corretos.
- `up --dry-run --json` planeja audit → prepare → run.
- `license status` / `data status` offline honestos (tier gratuito intacto).
- `diagnose` sem padrão conhecido: fallback honesto ("Não reconheci o
  padrão…") em vez de alucinar causa — comportamento desejado.
