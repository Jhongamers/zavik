<div align="center">

# Zavik

**Motor de dev CLI & orquestração por IA** de código aberto — detecta sua
stack, audita a **supply-chain** das suas dependências, entrega a qualquer
agente de IA um briefing de uma chamada.

[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)
[![CI](https://github.com/Jhongamers/zavik/actions/workflows/ci.yml/badge.svg)](https://github.com/Jhongamers/zavik/actions/workflows/ci.yml)

[English](README.md) · [Português (Brasil)](README.pt-BR.md)

</div>

**Funciona com qualquer agente** — Claude Code, Codex, OpenCode, Cursor,
Antigravity, aider, Copilot — ou sem agente nenhum. O Zavik é uma CLI pura:
saída `--json` estável e exit codes *são* a interface; não existe plugin de
vendor pra instalar. *(Compatibilidade, não filiação.)*

## Como rodar

**Instalação rápida** (linux x64/arm64, macOS arm64; precisa da release
`v0.1.2` publicada):

```bash
curl -fsSL https://raw.githubusercontent.com/Jhongamers/zavik/main/install.sh | sh
```

Detecta seu OS/arch, baixa o tarball da release, **confere o sha256** (sem
flag de pular) e instala em `~/.local/bin`. De preferência baixe o script,
leia e rode — o nosso próprio `audit` classifica `curl | sh` como risco.
Windows: pegue o `.zip` em
[Releases](https://github.com/Jhongamers/zavik/releases). Overrides:
`ZAVIK_VERSION`, `ZAVIK_INSTALL_DIR`, `ZAVIK_BASE_URL` (espelhos,
ambiente air-gapped, testes).

**Da fonte** (sempre funciona):

```bash
cargo build            # compila todos os crates
cargo test             # roda todos os testes
cargo run              # analisa o diretório atual (detect)
```

O binário fica em `target/debug/zavik`.

## Comandos

```bash
zavik [caminho]                     # detecta a stack (padrão)
zavik detect [caminho]              # idem
zavik analyze [caminho] [flags]     # analisa as dependências do projeto
zavik context [caminho] [flags]     # briefing: runbook + entradas + riscos (1 chamada)
zavik diagnose [flags]              # resume um erro: causa real + conserto
zavik env [caminho] [flags]         # fingerprint do ambiente: versões das toolchains + o que falta
zavik run [caminho] [--detach]  # sobe o projeto (--detach: background + PID)
zavik up [url|caminho] [flags]      # pipeline: clone? → audit → prepare → run (mesmos gates)
zavik audit [caminho] [flags]       # auditoria de supply-chain (--fail-on gateia CI)
zavik license status|install|remove # licença offline (assinatura Ed25519, local)
zavik data status|update            # dataset de risco (grátis: status; Pro: update)
zavik prepare [caminho] [flags]     # instala dependências (SEM Docker)
zavik docker  [caminho] [flags]     # gera Dockerfile + build da imagem
```

Flags:

| Flag        | Efeito                                                              |
| ----------- | ------------------------------------------------------------------- |
| `--dry-run` | Mostra o plano e **não executa nada**.                              |
| `--yes`/`-y`| Aprova tudo automaticamente, sem perguntar.                         |
| `--force`   | Permite **sobrescrever** arquivos existentes (ex.: `Dockerfile`).    |
| `--online`  | (`analyze`/`audit`) Consulta os registries (versões / hooks).      |
| `--json`    | Saída estruturada pra IA (stdout = JSON puro).                     |
| `--all`     | Desce em subpastas procurando projetos (monorepo/aninhado).        |
| `--fail-on` | (`audit`) Severidade mínima que reprova o CI (exit 1).             |
| `--policy`  | (`audit`) Arquivo de política TOML (`fail_on`, `allowlist`).       |
| `--ignore`  | (`audit`) Supressões TOML com **expiração obrigatória**.           |
| `--sarif`   | (`audit`) Emite SARIF 2.1.0 (GitHub Code Scanning).                |
| `--url`     | (`data`) Origem do dataset: `https://...` ou `file://<caminho>`    |
|             | (alternativa: env `ZV_DATASET_URL`).                               |
| `--message` | (`diagnose`) Texto de erro inline.                                  |
| `--input`   | (`diagnose`) Arquivo com a saída de erro.                           |
| `--timeout` | (`run`) Segundos esperando ficar pronto (padrão 60).                |
| `--health`  | (`run`) Readiness explícito: `tcp:<porta>` \| `http://...`.        |
| `--until`   | (`up`) Para depois da etapa: `clone\|audit\|prepare\|run`.         |
| `--skip`    | (`up`) Pula etapa(s), separadas por vírgula (nunca `clone`).       |

### Saída JSON (orquestração por IA)

Com `--json`, qualquer comando devolve **JSON compacto no stdout** (o "barulho"
de diagnóstico vai pra stderr). Pensado pra quem chama ser um **agente**: texto
pros humanos obriga o modelo a interpretar e gasta token; JSON entrega o fato
pronto. Todo documento tem `"schema": 1`.

```bash
zavik detect  --json          # o que é este projeto (ground truth)
zavik context --json          # briefing: runbook + entradas + riscos (1 chamada)
zavik analyze --json          # dependências doentes
zavik diagnose --json         # causa real de um erro (+ conserto)
zavik env --json              # fingerprint do ambiente: OS/arch, versões, o que falta
zavik run --dry-run --json    # comando de start resolvido + de onde veio
zavik run --json              # plano sem executar (modo IA) ou resultado final
zavik run --detach --json     # background: pid + log + stop_command (sem supervisionar)
zavik up --dry-run --json     # plano do pipeline (etapa a etapa, sem executar)
zavik up --json               # JSON Lines: 1 objeto por etapa + resumo final
zavik audit --json            # relatório de supply-chain (score, fatores, veredito)
zavik license status --json   # licença instalada (plan, holder, expires, valid)
zavik data status --json      # dataset instalado? versão, idade, staleness (livre)
zavik data update --json      # Pro: baixa+verifica; sem licença → no_license, exit 1
zavik prepare --json          # plano + verificação de confiança (não executa)
zavik docker  --json          # idem, modo Docker
```

> `prepare`/`docker` com `--json` entregam o **plano** e os **riscos** de forma
> estruturada e **não executam** — execução é interativa/humana. É o combo
> "IA propõe, humano aprova" (roadmap 2.3).

Exemplo (`detect --json`, enxugado):
```json
{"schema":1,"command":"detect","stacks":[{"language":"Rust","package_manager":"cargo","frameworks":["Tokio"],"dependencies":[{"name":"serde","version":"=1.0.100","kind":"Runtime"}]}]}
```

> **O gancho de economia de token:** tudo que a CLI faz é Rust local
> **determinístico = zero token de IA**. O LLM é o cérebro (decide o quê), a CLI
> é as mãos (sabe como, com permissão + verificação). Um agente chama
> `detect --json` e recebe o fato em vez de ler N arquivos de manifesto.

### Idioma (`--lang`)

Texto humano da CLI: **inglês por padrão** (default virado no i18n de
pré-lançamento); `--lang pt-BR` (também `--lang=pt-BR` e
`ZAVIK_LANG=pt-BR`, ou locale BR do sistema `LC_ALL`/`LANG=pt_BR…`)
troca as mensagens pro português, `--lang en` volta pro inglês. **Saída de
máquina nunca troca**: chaves/valores de JSON, SARIF e exit codes são
sempre inglês. Design e status da migração: `docs/design-i18n.md`.

```bash
zavik env --lang en            # seções humanas em inglês
ZAVIK_LANG=en zavik audit   # idem, via env (agentes/CI)
```

### Briefing de uma chamada (`context`)

O maior gasto de uma IA num projeto estranho é **exploração** (glob/grep/ler
README pra descobrir "como buildo/testo/rodo?"). O `context` responde tudo isso
**numa chamada só**:

```bash
zavik context --json
```

```json
{"schema":1,"command":"context",
 "runbook":{"setup":["npm install"],"build":["npm run build"],"test":["npm test"],"run":["npm run dev"],"lint":["npm run lint"]},
 "entrypoints":["src/index.js"],
 "key_files":[{"kind":"manifest","path":"package.json"},{"kind":"lockfile","path":"package-lock.json"},{"kind":"ci","path":".github/workflows"}],
 "trust":{"risks":[],"blocking":0}}
```

- **runbook**: comandos de `setup`/`build`/`test`/`run`/`lint`, tirados de
  `scripts` do `package.json`, alvos de `Makefile`/`justfile`, wrappers
  (`mvnw`/`gradlew`) e convenções por linguagem.
- **entrypoints**: arquivos de entrada (`src/main.rs`, `index.js`,
  `cmd/api/main.go`, ...).
- **key_files**: manifesto, lockfile, CI, Dockerfile, `.env.example`, README.
- Junta `detect` + `trust` + isso tudo — troca N explorações por 1 JSON.

Sem `--json`, mostra o mesmo em seções legíveis (COMO RODAR / ENTRADAS /
ARQUIVOS-CHAVE / CONFIANÇA).

> **Honestidade:** o runbook é inferido (manifestos + convenções). Ele dá o
> comando **mais provável**, não uma garantia de que roda sem ajuste — projetos
> com build peculiar podem precisar de correção manual. CI não é parsed de YAML
> (só listada nos `key_files`).

### Diagnóstico de erro (`diagnose`)

Quando um comando falha, o erro literal quase sempre é **sintoma, não causa**.
`cannot find -lssl` parece problema de linking, mas a causa é "falta o
`libssl-dev`"; `node-gyp ERR` parece problema do npm, mas a causa é "falta
python3/gcc". O `diagnose` casa **padrões conhecidos** na saída inteira e devolve
a causa **real** + o conserto, com a "prova" (as linhas que casaram):

```bash
# 1) cola o erro direto
zavik diagnose --message "sh: 1: cargo: not found"
#   → conserto: `cargo` não está no PATH — rode `zavik prepare` pra toolchain

# 2) ou pipe (sem nem sair do fluxo de build)
meu_build 2>&1 | zavik diagnose

# 3) ou de um arquivo
zavik diagnose --input build.log --json
```

É Rust local e determinístico (zero token). Se **nenhuma** regra casa, ele é
honesto e devolve as linhas com mais cara de erro (resumo) em vez de fingir que
entendeu. Por quê não `zavik run <comando>`? Pra não criar uma superfície
nova de execução arbitrária — você/IA já tem o erro, falta só a tradução.

### Executar o projeto (`run`)

O `run` fecha o ciclo: **audit → prepare → run**. Ele **resolve como o projeto
sobe** e sobe — nunca adivinha em silêncio:

1. **Declarado** (o projeto diz): `package.json` `scripts.start`, Procfile
   (`web:`), `make run/start/serve`, `docker compose up`.
2. **Convenção**: Django, Rails, Phoenix, `cargo run`, `go run .`, `dotnet run`,
   Spring Boot, Laravel, `dart run`…
3. **Ambíguo? Pergunta.** Nada achou? diz honestamente (não roda nada no chute).

```bash
zavik run                      # resolve, PERGUNTA se pode executar, e sobe
zavik run --detach             # background: devolve pid + log (sem esperar readiness)
zavik run --dry-run            # só mostra o comando resolvido (sem executar)
zavik run --timeout 90         # espera ficar pronto por até 90s (padrão 60)
zavik run --health http://localhost:3000/health   # readiness explícito
zavik run . -- --port 3001     # args após -- vão pro comando de start
zavik run --json               # plano/resultado estruturado (modo IA)
```

**Detached:** `--detach` sobe o projeto na **própria sessão** — fechar o
terminal não derruba o dev server — e devolve `pid`, `log`
(`.zavik/run.log`, acompanhe com `tail -f`) e `stop_command` (ex.:
`kill 1234`). O trato é honesto: **não** espera readiness nem supervisiona;
rode sem `--detach` pra ter health check + crash→diagnose. Mesmo gate:
executar é risco crítico, `--yes` não cobre. *(No Windows o processo
detached continua preso ao console — limitação conhecida.)*

**O que torna "deu certo" honesto:** um servidor não termina, então exit code
não basta — o `run` detecta **porta aceitando conexão** (heurística: `--health`,
portas candidatas, e no Linux a porta **real** do processo e dos descendentes
via `/proc` — pega até o neto `npm → node`). Ctrl-C/SIGTERM derruba o **grupo
inteiro** (SIGTERM → espera → SIGKILL). Crash antes de subir vira entrada do
`diagnose` (causa + conserto).

**Limitações honestas (v1):** prontidão é heurística de porta (não é garantia);
`--health http` sem https; no macOS não há descoberta de porta (use
`--health`/candidatas); `--json` não faz stream ao vivo (entrega o tail no
resultado); `--detach` não supervisiona readiness; POSIX (Linux/macOS).
E a regra de sempre: **executar é risco crítico — `--yes` não cobre**, o humano
decide (modo IA: `--json` entrega o plano, não executa).

### Pipeline completo (`up`)

O princípio do produto em um comando: toda etapa funciona **isolada E
composta**. O `up` encadeia o pipeline usando **literalmente os mesmos caminhos
de código** dos comandos isolados — compor não enfraquece segurança:

```bash
zavik up                        # audit → prepare → run no diretório atual
zavik up ./meu-projeto          # idem em outro caminho
zavik up https://github.com/x/y # clone seguro (--depth 1, sem submódulos,
                                   # sem hooks; destino = ./y — nunca sobrescreve)
zavik up --until audit          # para depois do audit (útil em CI)
zavik up --skip run             # só prepara, não sobe
zavik up --dry-run              # mostra o plano de etapas; não executa nada
zavik up --json                 # JSON Lines: 1 objeto por etapa + resumo final
```

**Gates por etapa, intactos:** confiança/toolchain/instalação e a execução do
`run` perguntam exatamente como nos comandos isolados — e **um aborto de
segurança interrompe o pipeline ANTES do `run`, sempre** (não-interativo nega
por padrão; `--yes` nunca cobre execução de código). No `--json` (modo IA), o
audit entrega resultado real, o prepare entrega o **plano** (não executa) e o
`run` nega sem terminal — quem executa é o humano.

**Limitações honestas (v1):** o `run` final é foreground (o `up` termina quando
o app termina — `--detach` é v1.1); sem resume (falhou, roda de novo); clone
exige `git` e um destino inexistente.

### Auditoria de supply-chain (`audit`)

O `audit` responde "**posso confiar** nesse código pra entrar na minha
empresa?". Copia o coração dos consolidados — **Socket.dev** (risk factors +
confluência), **Snyk/cargo-audit** (policy + `fail_on` + exit code) e **OWASP**
(supressão de FP) — com três travas nossas:

1. **Nenhum sinal isolado é veredito.** A severidade vem da *soma* de pesos
   (confluência) e só um score ≥ 25 vira achação. Então `^4.18.0` (range
   normal) ou um `postinstall` benigno (`node-gyp rebuild`, `prebuild-install`)
   **não** viram alerta — testado com o esbuild real.
2. **Classificamos o CONTEÚDO do hook**, não só a existência: `curl|sh`,
   base64+eval, shell reversa e acesso a credenciais são suspeitos;
   build-nativo/husky são esperados.
3. **Supressão exige expiração** (`expires` no TOML) — um ignore nunca vira
   escudo eterno pra um pacote comprometido; supressão expirada reativa o
   alerta e é listada pra re-triagem.

```bash
zavik audit                      # offline: origem (registro/git/path) + pin
zavik audit --online             # + hooks de instalação e downloads do npm
zavik audit --fail-on high       # exit 1 se achar high/critical (gateia CI)
zavik audit --sarif > results.sarif   # SARIF 2.1.0 p/ GitHub Code Scanning
zavik audit --policy .zavik/policy.toml --ignore .zavik/ignore.toml
```

Política (`.zavik/policy.toml`, versionada):
```toml
fail_on = "high"
allowlist = ["meu-pacote-interno"]   # git interno esperado → não alerta
```
Supressão (`.zavik/ignore.toml` — `expires` é **obrigatório**):
```toml
[[ignore]]
package = "algum-pacote"
factor = "non-registry"
reason = "fork interno auditado"
expires = "2027-01-01"
```

**Honestidade:** analisamos o *texto* do script de instalação, não os arquivos
que ele invoca (`node install.js`) — é um sinal, não garantia. Detecção de
typosquat por similaridade de nome e verificação de integridade de lockfile
estão no radar (ainda não implementadas). O `--online` consulta o registry npm.

#### CI/CD — GitHub Action (`action.yml`)

O repositório expõe um **composite action** que roda o audit em PRs: baixa o
binário da Release (com verificação de `sha256`), publica o SARIF no Code
Scanning e **reprova o job** pelo exit code. Empacotamento vem junto — o
workflow `.github/workflows/release.yml` publica binários (linux x64/arm64,
macOS arm64, windows) a cada tag `v*`:

```bash
git tag v0.1.2 && git push origin v0.1.2   # publica as Releases
```

Uso no repositório do consumidor (`.github/workflows/audit.yml`):
```yaml
name: supply-chain audit
on: [pull_request]
permissions:
  contents: read
  security-events: write   # necessário p/ publicar o SARIF
jobs:
  audit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: Jhongamers/zavik@v0.1.2
        with:
          fail-on: high
          # version: v0.1.2   # tag pinada (proposital, não "latest")
          # sha256: <hash>    # opcional: ancora a integridade no action
          # online: "true"    # default; offline o score só chega a Medium
```

**Honestidade sobre o CI:**
- O **gate funciona sempre**: `fail-on` → exit 1 reprovando o job, sem
  dependência de nada além do binário. Mas repare: **offline** o audit só vê
  origem + pin, então o score não passa de **Medium** — com `fail-on high` um
  audit offline nunca reprovaria nada. Por isso o action vai com
  `online: "true"` (é o conteúdo do hook que produz High/Critical).
- A **anotação inline no PR** (SARIF) exige Code Scanning: grátis em repo
  público; em repo **privado** exige GitHub Advanced Security (pago). Sem ele,
  use `upload-sarif: "false"` e fique só com o gate.
- Inputs entram via `env:` (não interpolados no `run:`) — imune a script
  injection; binário verificado por `sha256`; ref **pinado** por tag.
- Action cobre Linux (x64/arm64) e macOS (arm64). Windows: baixar o `.zip` da
  Release manualmente (o binário é publicado).

#### Licença (`license`) — fundação da monetização (offline)

Modelo escolhido: **open core + assinatura**. O CLI é grátis; a licença é a
chave que destrava (e destravará) o que é pago — sem nenhuma chamada de rede.

```bash
zavik license install <chave|@arquivo>   # valida localmente (Ed25519) e instala
zavik license status [--json]            # plano, titular, expiração, validade
zavik license remove
```

- **Formato:** token `ZV1.<payload-b64url>.<assinatura>`; o binário carrega só
  a chave **pública**. Verificação 100% local, em `~/.config/zavik/license.key`.
- **Honestidade:** é um gate **anti-honesto**, não anti-atacante — patchear o
  binário burla (é o mesmo trato de TablePlus/Transmit). Também não defende
  relógio do sistema atrasado. A proteção de verdade chega na fase server-side.
- **O que ela bloqueia hoje:** só o `data update` (dataset Pro abaixo). Todo o
  restante do CLI — incluindo `context`, que economiza token de IA — segue
  **100% grátis**. É a fundação da fase server-side, não uma DRM.
- Cunho de licenças (vendor-only, não vai no binário):
  `cargo run --example license_tool -- genkey|mint ...` — a chave privada fica
  com o vendor, **nunca** no repo.

#### Dataset de risco Pro (`data`)

O `audit` offline de hoje (fatores de confluência, `--online`) é **o tier
grátis, intacto**. O `data update` acrescenta, por cima, dados de risco
**assinados e frescos** — advisories recentes (GHSA **revisados**, janela de
~90 dias) e uma kill list **exata** de typosquat/maliciosos (patterns
precisos de título medidos no banco real — sem fuzzy), baixados daqui da
Release e verificados com a chave pública embutida.

```bash
zavik data status [--json]              # livre, offline: versão, idade, staleness
zavik data update [--json] [--url <url>]# Pro: baixa, VERIFICA e instala (exige licença)
```

| | Grátis (hoje) | Pro (licença + `data update`) |
| --- | --- | --- |
| `audit` offline (confluência, FP controlado) | ✅ | ✅ |
| `context` (economia de token), `run`, `up`, `prepare`... | ✅ | ✅ |
| Advisories recentes por nome + kill list no `audit` | — | ✅ seção `dataset_findings` |
| Atualização diária do dataset (Release assinada) | — | ✅ |
| `data status` (estado do dataset) | ✅ (livre) | ✅ |

Regras e limites (por design, ver `docs/design-data.md`):

- **Gate só no `update`** — sem licença válida → `no_license`/`license_invalid`,
  exit 1, **sem tocar a rede**. `status` é livre.
- **Assinatura obrigatória**: envelope Ed25519 verificado antes de escrever
  qualquer byte; recusa (adulterado/chave errada) deixa o dataset anterior
  intacto. Chave de dataset **separada** da de licença (blast radius).
- **Staleness honesta**: dataset com mais de 14 dias ganha aviso — o freshness
  é parte do produto, não escondido.
- **FP controlado**: advisory casa só por **nome** → achado `low` "possível
  exposição — confira sua versão" (sem range-matching por ecossistema no v1);
  kill list casa **exato** → `high`. **Nenhum** fuzzy.
- **A seção não mexe no CI**: não altera `score`, `passed` nem `--fail-on` —
  audit grátis e Pro continuam comparáveis; sem dataset, o JSON é
  **byte idêntico** ao de sempre (sem chave `dataset`).

### Análise de dependências (`analyze`)

O `analyze` é **leitura apenas** — não instala nem altera nada. Ele combina duas
camadas:

- **Offline (sempre roda):** versão coringa/não fixada (`*`, `latest`, vazia),
  dependência sem versão, dependência vinda de fora do registro (git/path/file),
  **lockfile ausente** e a mesma dependência declarada com versões diferentes.
- **Online (`--online`):** consulta **crates.io / npm / PyPI** e avisa quando há
  versão mais nova que a que você pediu. Limita a 40 requisições por execução
  (por causa de rate limit e tempo).

```
⚠️  Rust: 'serde' tem versão mais nova no registro: 1.0.100 -> 1.0.229.
⚠️  JavaScript: nenhum lockfile encontrado (esperado: package-lock.json ou ...).
ℹ️  JavaScript: 'meu-lib' vem de 'git+https://...' (fora do registro de pacotes).
```

> Diferente do `prepare`/`docker`, o `analyze` **não pede permissão** porque não
> executa nada na sua máquina — só o `--online` faz requisições HTTP de leitura.

### Permissões

Por padrão o Zavik **não faz nada sozinho**: antes de cada ação sensível
(instalar, gerar/sobrescrever arquivo, rodar `docker build`) ele pergunta:

```
🔐 Permissão necessária: Instalar dependências (npm)
   ação   : npm install
   motivo : Baixa e instala os pacotes do package.json.
   Executar? [s]im / [n]ão / [a]sim para todas:
```

- `s` executa só aquele passo, `n` pula, `a` aprova todos os restantes.
- Sem terminal (execução não interativa) ele **nega por padrão**, por segurança.
- Arquivos existentes **nunca** são sobrescritos sem `--force`.

### Segurança e supply-chain (verificação automática)

Um manifesto (`package.json`, `Cargo.toml`, ...) é **input não confiável**: um
projeto malicioso (ou uma IA dirigindo o Zavik sem perceber) pode mandar
instalar código arbitrário. Por isso, **antes** de qualquer `prepare`/`docker`
roda um **pré-flight de confiança** que não confia cegamente no manifest:

- **Scripts de lifecycle do npm** (`preinstall`/`install`/`postinstall`/`prepare`)
  → rodam código arbitrário no `npm install`. Mostra o comando exato. Isso vale
  **tanto pro projeto raiz quanto pra cada dependência**: antes de `npm install`,
  ele busca no registry (sem baixar tarball) os hooks de instalação de cada
  dependência e agrega tudo num aviso só — ex.: `esbuild → postinstall: node install.js`.
- **Dependência fora do registro** (git/path/file) → código de terceiros sem
  auditoria do package manager.
- **A "busca" no registry**: para npm, confere **downloads/semana** (pega
  typosquat vivo: `lodahs` ~125 vs `lodash` ~162.000.000); para **Rust, Python,
  Ruby, PHP, .NET, Dart, Elixir, Go e Java/Maven**, confere se o pacote
  **existe** no registry oficial (crates.io, PyPI, RubyGems, Packagist, NuGet,
  pub.dev, Hex, `proxy.golang.org`, Maven Central).

Cada risco exige confirmação **individual** — **"sim para todas" e `--yes` NÃO
valem aqui**. Se qualquer risco for recusado (ou `--yes` for usado), o
`prepare`/`docker` **aborta sem instalar nada**.

> **Injeção de comando:** já é impossível por construção — os comandos rodam
> sem shell (`Command::new(prog).args([...])`), então um nome de pacote com
> `; rm -rf /` vira só um argumento literal. O nome de imagem Docker é saneado
> pra `[a-z0-9._-]`.
>
> **Limitações honestas:** o check de downloads é um sinal, não prova — um
> typosquat com tráfego alto ou um pacote legítimo muito pequeno podem passar.
> No `postinstall` de dependências, ele olha a versão que o `npm install`
> puxaria (exata quando o requisito é pinado; `latest` quando é faixa), então
> pode perder um script que só existe em outra versão — e nada disso substitui
> ler o código. É uma rede de proteção extra, não uma garantia.

### O que ele prepara

- **`prepare`** (sem Docker): roda o instalador de cada stack detectado
  (`cargo build`, `npm install`, `pip install` em `.venv`, `go mod download`,
  `composer install`, `bundle install`, `dotnet restore`, `mix deps.get`,
  `cmake`/`make`, ...) e mostra o comando para rodar depois.
- **`docker`**: gera um `Dockerfile` por stack (multi-stage quando faz sentido),
  roda `docker build` e mostra o `docker run`. Os Dockerfiles vêm com um
  comentário avisando que `CMD`/variáveis podem precisar de ajuste — são um
  **ponto de partida**, não uma garantia de que todo projeto roda sem ajustes.

### Fingerprint do ambiente (`env`)

Uma chamada read-only que substitui ~10 sondagens de um agente por *"essa
máquina sequer funciona?"*:

```bash
zavik env --json
```

```json
{"schema":1,"command":"env","os":"linux","arch":"x86_64",
 "stacks":[{"language":"JavaScript","tools":[
   {"name":"node","present":true,"version":"v22.10.0"},
   {"name":"npm","present":true,"version":"10.9.0"}]}],
 "missing":[]}
```

- **Presença** = busca no PATH; **versão** = `<tool> --version` de ferramentas
  já instaladas (argv, **sem shell**). Se a sonda falhar,
  `"version": null` — ele nunca inventa uma versão.
- **Read-only**: não instala nada e **nunca lê arquivos `.env`** (seus
  segredos não são assunto dele).
- Modo humano mostra o mesmo em seções (OS + ferramentas por stack + sugestão
  de instalação do que falta) — o formato JSON é a interface pro agente.

### Toolchain faltando (node/npm/go/...)

Antes de rodar o `prepare` (sem Docker), ele checa se as **ferramentas** do
stack estão no PATH (`cargo`, `node`+`npm`, `go`, `python3`, `php`+`composer`,
...) e, se algo faltar, **avisa na hora** com a sugestão de instalação pra **o
seu sistema** (apt/dnf/pacman no Linux, `brew` no macOS, `winget` no Windows):

```
FERRAMENTAS FALTANDO
  • ruby (Ruby)
      sugestão: sudo apt install ruby
```

- **Oferece instalar (Fase 2, com permissão):** pra um **set curado** de
  ferramentas (node/npm/go/python3/java/php/ruby), ele **pergunta** se quer que
  ele rode o comando de instalação. Cada uma é uma confirmação **crítica** —
  **nem "sim para todas" nem `--yes` cobrem**. Roda **sem shell** (argv a
  argv) com **stdio herdado**, então um `sudo`/`brew` que peça senha funciona.
  Depois ele **re-verifica** o PATH e diz se funcionou (pode recusar ou falhar).
  As demais ferramentas (ex.: Rust, que é via `curl | sh`) ficam **só na
  sugestão** — ele não chuta comando/versão.
- No modo **Docker** não checa: a toolchain vive dentro do container.
- As sugestões são um **palpite razoável** por SO, não garantia — cada máquina
  pode variar, e o gerenciador de pacotes pode nem existir.

> ⚠️ Instalar dependências executa comandos na sua máquina (e `npm install`
> roda scripts do projeto). Use `--dry-run` para inspecionar antes.

## Arquitetura

O Zavik segue **Clean Architecture** e o padrão **Microkernel**, garantindo
modularidade, testabilidade e extensibilidade. Os componentes críticos são:

- **EventBus:** comunicação reativa e desacoplada entre módulos.
- **Plugin SDK:** contratos (`Capabilities`) que isolam os plugins.
- **Workflow Engine:** execução transacional em etapas (`Workflow -> Step -> Action`).
- **Kernel:** orquestrador que gerencia o ciclo de vida dos plugins.

### Estrutura do workspace (`crates/`)

As dependências fluem de fora para dentro (veja `arquiteture.md`):

```text
shared  ->  events  ->  sdk  ->  { core, kernel } / plugins
```

| Crate              | Papel                                                             | Depende de                     |
| ------------------ | ----------------------------------------------------------------- | ------------------------------ |
| `zavik-shared`  | Domínio: tipos universais (`Context`, `ProjectStack`, `Language`). | —                              |
| `zavik-events`  | Barramento de eventos (`EventBus`, `EventEnvelope`).               | `shared`                       |
| `zavik-sdk`     | Contratos: `Capability`, `FileSystemAccess`, `ProjectDetector`, `CommandRunner` e a porta `PermissionGate`. | `shared`, `events` |
| `zavik-plugins` | Plugins V1: `LocalFileSystem`, `StackDetector`, `LocalCommandRunner`. | `shared`, `sdk`             |
| `zavik-core`    | Casos de uso: `WorkflowEngine` e `Provisioner` (preparar projeto).  | `shared`, `sdk`                |
| `zavik-kernel`  | Microkernel: registry de plugins e ciclo de vida.                  | `shared`, `events`, `sdk`      |
| `zavik` (CLI)   | Ponto de entrada + `PermissionGate` interativo.                    | todos                          |

## Detecção de stack

O plugin `StackDetector` reconhece a stack de um projeto lendo os manifestos da
raiz. Um mesmo projeto pode ter **vários** stacks (ex.: um app Tauri tem Rust e
Node ao mesmo tempo).

Com a flag **`--all`**, ele também **desce nas subpastas** procurando manifestos —
útil pra **monorepo** ou projeto com manifesto aninhado (ex.: `.csproj` em
`Src/Core/`, `package.json` em `pkgs/api/`). A varredura pula dirs de
dependência/build (`node_modules`, `target`, ...) e tem limite de profundidade e
quantidade. Sem `--all`, ele olha só a raiz (comportamento previsível).

| Linguagem        | Manifesto                    | Frameworks reconhecidos (exemplos)          |
| ---------------- | ---------------------------- | ------------------------------------------- |
| Rust             | `Cargo.toml`                 | Tauri, Actix Web, Axum, Rocket, Bevy, Yew... |
| JavaScript/TS    | `package.json`               | React, Next.js, Vue, Angular, Express, NestJS, Electron, Vite... |
| Python           | `pyproject.toml`, `requirements.txt`, `Pipfile`, `setup.py` | Django, Flask, FastAPI, Celery, Pytest... |
| Go               | `go.mod`                     | Gin, Echo, Fiber, GORM, Chi, Cobra          |
| Java / Kotlin    | `pom.xml`, `build.gradle[.kts]` | Spring, Quarkus, Micronaut, Android      |
| PHP              | `composer.json`              | Laravel, Symfony, CakePHP, WordPress        |
| Ruby             | `Gemfile`                    | Ruby on Rails, Sinatra, Hanami, Jekyll      |
| C# / .NET        | `*.csproj`                   | ASP.NET Core, Entity Framework, MAUI        |
| Dart / Flutter   | `pubspec.yaml`               | Flutter                                     |
| Elixir           | `mix.exs`                    | Phoenix, Ecto, LiveView                     |
| Swift            | `Package.swift`              | Vapor, Hummingbird                          |
| C / C++          | `CMakeLists.txt`, `Makefile` | —                                           |

A tabela completa de palavras-chave está em
`crates/plugins/src/detector/`.

## Status

- [x] Estrutura do workspace e direção de dependências.
- [x] EventBus funcional com assinantes.
- [x] Contratos de capability (`FileSystemAccess`, `ProjectDetector`, `CommandRunner`).
- [x] Caso de uso: verificar `README.md` (POC inicial).
- [x] Caso de uso: detecção de linguagem, framework e dependências.
- [x] Caso de uso: preparar projeto para rodar (sem Docker e com Docker).
- [x] Sistema de permissão (interativo, `--yes`, `--dry-run`, `--force`).
- [ ] Analisador de dependências desatualizadas/vulneráveis.
- [ ] Execução isolada (sandbox) para instalações.
- [ ] UI desktop via Tauri v2 (ADR 0006).
