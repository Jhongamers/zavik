# Design — Corpus de validação por ecossistema (projetos reais)

Aprovado pelo fundador em 17/10 ("pode ser, mas voce acha valido clonar
projetos reais…"). Antes de codar: registro aqui as regras, o que é e o que
NÃO é válido clonar/testar, e as fases.

## 1. Por quê (histórico, não teoria)

O método já se pagou **duas vezes**:

- **§1.5 do ROADMAP**: 12 projetos reais testados à mão (anyhow, cobra,
  itsdangerous, junit4, Newtonsoft.Json, phoenix, googletest,
  dart-lang/args, sinatra, swift-argument-parser, polyfill-mbstring,
  Newtonsoft.Json…) acharam o bug "Makefile sozinho virava falso C/C++"
  (um **falso positivo** de detecção) e originaram o `--all`.
- **Dogfooding (docs/dogfooding.md)**: rodar o DevForge no próprio DevForge
  achou 4 problemas que **165+ testes verdes não pegaram** (workspace
  invisível, `workspace = true` sem versão, `diagnose` sem E0308, "APROVADO"
  com 0 pacotes).

Conclusão operacional: **teste = regressão/invariante no que modelamos;
corpus = espécie contra o mundo real.** Um não substitui o outro — o
dogfooding acha, o teste tranca (cada achado vira fixture + teste).

## 2. Vale clonar? Sim, com regras — as4 armadilhas de código alheio

No dogfooding o código é nosso; em projeto alheio não. Regras fixas:

| # | Armadilha | Regra |
|---|---|---|
| 1 | **Flaky**: `main` do upstream muda sozinho → CI aleatório | **Pin em SHA** por projeto; bump de SHA = atualização revisada (diff do manifesto do upstream). Nunca branch solta. |
| 2 | **Rede/tempo**: projeto grande, Actions privado = 2000 min/mês | **Partial clone** (`git clone --filter=blob:none` [+ `--sparse`]) — a maioria dos comandos lê só manifesto/lockfile (KBs). Fetch é etapa **separada com cache** (Actions cache); os testes do corpus rodam **offline** depois do fetch (offline-first preservado). |
| 3 | **Licença**: o repo fica **público** no PH → vendorizar código de terceiro no nosso tree = attribution/compatibilidade a resolver | **Nunca vendorizar**. O nosso repo guarda só **metadados nosso** (URL + SHA + expectativas, arquivo pequeno no formato que quisermos). O clone vive no ambiente de teste. |
| 4 | **Execução de código alheio**: `run`/`prepare`/`up`/`docker` num terceiro = exatamente o que a doutrina de permissão proíbe (humano manda; `--yes` nunca cobre execução) | Corpus automatizado = **somente comandos read-only**: `detect`, `analyze`, `audit`, `context`, `env` + `diagnose` alimentado com **stderr golden capturado**. Rodar código de terceiro **nunca** no corpus/CI — uso real de `run`/`up` continua sendo decisão humana. |

## 3. Ground truth anti-circular

Asserção nunca é "output atual == output salvo" puro (isso congela bug
invisivelmente). Duas fontes independentes, escritas **antes** de rodar:

1. **Lockfile = verdade de versões**: o `Cargo.lock`/`package-lock.json`/
   `go.sum` diz a versão *resolvida*; o `analyze`/`audit` lê o manifesto.
   Aferição: versão reportada == versão resolvida no lockfile.
   (Se divergir em projeto real, pode ser bug de parser **ou** gap
   declared-vs-resolved — pergunta honesta registrada aqui; decidir caso a
   caso, não silenciar a aferição.)
2. **Metadados curados por humano (uma vez por projeto)**, no arquivo
   nosso: linguagem esperada, framework esperado, entrypoints esperados
   (`src/main.rs`, `bin/www`, `Procfile`…), e para `diagnose`: um stderr
   **real** do projeto + a causa esperada.

Golden files só onde a comparação é justa: **formato** (schema/keys do
`--json`) e **diagnose** (input → causa). Veredito/score de audit nunca
entra como golden (é opinião do motor, muda com política — aferir é outro
jogo, §9).

### 3.1 Sondas duplas: anti-FP e anti-FN (exigência do fundador, 17/10)

Toda entrada do corpus carrega **duas asserções** — uma sem a outra é
meio teste:

- **Anti-FP** ("não pode acusar X"): ground truth limpo/esperado — se o
  DevForge acusar (linguagem errada, finding sem base, sugerir comando que
  falharia), **o teste falha**. Exemplos reais já vistos: Makefile de
  projeto Go acusado como C/C++ (§1.5); `cargo run` sugerido em projeto
  só-lib (corpus Fase A, NOVO-1).
- **Anti-FN** ("tem que acusar Y"): entrada onde o problema é **real e
  conhecido** — se o DevForge passar limpo, **o teste falha**. Exemplos:
  manifesto sintético declarando pacote da kill list (**só declarado,
  nunca executado**), manifest com fatores de risco reais (install script,
  shell access), stderr real de erro pro `diagnose` (Fase C).

O veredito de audit ganha as duas sondas nos **fixtures sintéticos**
(motor offline, determinístico, roda em qualquer lugar); confronto com
GHSA real do mundo continua sendo linha separada (§6).

## 4. Fases

- **Fase A — Rust**: seed = projetos já usados no §1.5 (anyhow) + **1
  workspace de verdade** (valida a expansão de membros nova: stacks ==
  membros, versões herdadas do `[workspace.dependencies]` == lockfile).
- **Fase B — npm/Python/Go**: 1 app com lockfile + 1 monorepo npm
  (`workspaces`) por ecossistema; seed também dos12 do §1.5.
- **Fase C — diagnose com erros reais**: coleção de stderr reais capturados
  (rustc, npm, pip, go build, gcc) + causa esperada em metadados; roda
  offline purinho.
- **Fase D — monorepo não-Rust**: hoje a expansão de workspace é Rust-only
  (limitação registrada em docs/dogfooding.md #7); a fase existe pra
  decidir se implementamos ou documentamos.

Cada fase: projeto entra no arquivo de metadados + teste `#[ignore]` +
execução dedicada. Candidatos finais (quais projetos exatos) = decisão do
fundador na hora da fase — sugestões de classe, não commit de nome.

## 5. Como roda (decisões de implementação)

- Testes do corpus ficam **fora do `cargo test` padrão** (`#[ignore]` +
  gate por env, ex.: `DF_CORPUS=1`) — `cargo test` continua 100% offline e
  determinístico (invariante de sempre).
- Job dedicado no CI (não no `ci.yml` de sempre): fetch com cache → roda os
  ignorados. Custo honesto: KBs de blob + ~minutos; se estourar quota do
  Actions privado, o corpus roda manual/atualizado — nunca em cima do gate
  de merge.
- Harness é nosso (shell/Rust), comandos do DevForge via binário compilado
  (`CARGO_BIN_EXE_*`), saídas `--json` conferidas contra metadados.

## 6. Limitações honestas (e o que NÃO é este design)

- Corpus **não** cobre `run`/`prepare`/`up` (nunca executa terceiro) —
  uso real desses comandos continua: dogfooding manual + usuários reais.
- Metadados são curadoria humana: ~meia hora por projeto; começar com
  poucos projetos de alto valor, não centenas.
- Monorepo não-Rust e entrypoint de crate Rust (dogfooding #5/#7) seguem
  registrados como pendências próprias — o corpus os *exporá*, não os
  resolve.
- O corpus **não prova o veredito de audit contra o mundo real**
  (aprovar/reprovar é juízo do motor offline; confronto com GHSA conhecido
  é linha separada, design-data §9). As **sondas sintéticas** anti-FP/anti-FN
  do §3.1 (kill list, fatores reais) valem e entram sim no corpus.
- Fora de escopo por ora: executar projetos no corpus, rede fora do fetch,
  comparação entre versões antigas do DevForge (regressão temporal).
