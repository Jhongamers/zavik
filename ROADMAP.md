# Roadmap — próximas tarefas (Zavik)

> Anotado para implementação **futura**. Nada disso foi feito ainda.
> Objetivo: transformar o Zavik num produto bom — ferramenta que uma IA
> orquestra gastando pouco token, com segurança de verdade.

---

## 1. Segurança / supply-chain (fechar lacunas)

### 1.1 Execução no install para TODAS as linguagens (hoje é só npm)
O check de `postinstall` hoje cobre **apenas JS/TS**. Cada ecossistema tem o
equivalente e **não é checado**:

- [ ] **Python**: `setup.py` / hooks de build (`pyproject.toml` `[build-system]`)
      rodam no `pip install .`.
- [ ] **Rust**: `build.rs` + hooks/alias do `.cargo/config.toml` rodam no `cargo build`.
- [ ] **PHP**: `composer install` roda scripts (`pre/post-install-cmd`).
- [ ] **JVM (Maven/Gradle)**: plugins de build executam código no build.
- [ ] **.NET**: scripts NuGet / targets MSBuild.
- [ ] **Ruby**: compilação de extensão nativa no `bundle install`.
- [ ] **Elixir/Mix**: hooks de compilação.

### 1.2 `postinstall` das DEPENDÊNCIAS (hoje só do projeto raiz)
Vetor mais perigoso (ex.: `event-stream`). Hoje só lemos o `package.json` raiz.
- [ ] Opção A: instalar com `--ignore-scripts`, inspecionar `node_modules`, só
      aí liberar os scripts.
- [ ] Opção B: baixar tarballs só dos pacotes suspeitos e ler o `package.json`
      de dentro (pesado; faz sentido só pra suspeitos).

### 1.3 Cobertura do "existe no registry"
A "busca" cobre agora **todos** os ecossistemas suportados:
- [x] npm (downloads/semana anti-typosquat), crates.io, PyPI.
- [x] RubyGems, Packagist (PHP), NuGet (.NET), pub.dev (Dart), Hex (Elixir).
- [x] **Go** via `proxy.golang.org/@latest` (escapa maiúscula → `!lowercase`).
- [x] **Maven Central** via `repo1.maven.org/maven2/{group}/{artifact}/`
      (detectores Maven/Gradle agora guardam a coordenada `groupId:artifactId`
      pra permitir a checagem).

### 1.4 Sinais anti-typosquat mais fortes
- [ ] Popularidade/downloads é um **sinal, não prova**. Avaliar:
      idade da conta, nº de versões, similaridade de nome com pacote famoso.

### 1.6 Toolchain faltando (node/npm/go/...) — detectar e sugerir
O usuário não deve descobrir "node não encontrado" só quando o `npm install`
falhar. Cada SO tem seu jeito de instalar e não dá pra prever tudo (sudo,
distro, conflitos), então é **faseado e conservador**:

- [x] **Fase 1 (feita): detectar + sugerir** — antes do `prepare` (local),
      checa se as ferramentas do stack estão no PATH (`which`/`where`) e mostra
      a sugestão de instalação **pra família de SO atual** (apt/dnf/pacman,
      brew, winget). Só leitura, não instala nada.
- [x] **Fase 2 (feita): perguntar e rodar a instalação** — com design
      conservador: só um **set curado** de ferramentas (node/npm/go/python3/
      java/php/ruby) tem comando automático; as demais (rust via `curl|sh`,
      dotnet/java com variantes) ficam só na sugestão. Cada instalação exige
      confirmação **crítica** no `PermissionGate` (nem "sim para todas" nem
      `--yes` cobrem). Roda **sem shell** (argv) com **stdio herdado**, então
      `sudo`/`brew` que pedem senha funcionam; se o gerenciador não existir ou
      a ferramenta não for curada, não oferece. Depois **re-verifica** o PATH
      e reporta com honestidade (pode falhar / recusado).
- [ ] **Fase 3: corrigir/sugerir em erro** — quando um comando falhar, ler a
      causa e sugerir o próximo passo (ex.: "faltou lib X", "versão conflitante").
      O mais imprevisível; deixar por último.
- Limitação honesta: as sugestões são um palpite razoável por SO, não
  garantia. Não substituem ler o erro da sua máquina.

---

## 2. Economia de token / virar produto de orquestração por IA

**Posicionamento:** o LLM é o **cérebro** (decide o quê), a CLI é as **mãos**
(sabe como, com permissão + verificação). Tudo que a CLI faz é **Rust local
determinístico = zero token de IA**. A peça que falta é saída **estruturada**.

### 2.1 `--json` em TODOS os comandos (prioridade máxima)
- [x] `detect --json`, `analyze --json`, `prepare --json`, `docker --json`.
- [x] Saída compacta (uma linha) no stdout; logs de diagnóstico indo pra stderr.
- [x] Schema estável com `"schema": 1`.
- [x] `prepare`/`docker --json` entregam **plano + verificação de confiança** e
      **não executam** (execução é interativa).

### 2.2 `detect --json` como "ground truth"
- [x] Feito junto com 2.1. Substitui a IA lendo N arquivos de manifesto.

### 2.3 `prepare --dry-run --json`
- [x] Coberto por `prepare --json` (plano + trust estruturados, sem executar).
- [ ] (Opcional) expor também `--dry-run` explicitamente igual ao `--json`.

### 2.4 Resumo de erro / próximo passo — `zavik diagnose` ✅
- [x] Um `zavik diagnose` que recebe uma saída que falhou e devolve
      **diagnóstico curto**: causa provável **real** (não o sintoma), conserto
      sugerido, e as linhas que casaram (a "prova"). Economiza ler stack trace
      gigante — e resolve o caso em que o erro literal aponta pro lugar errado.
- [x] Entrada por `--message "..."`, `--input <arquivo>` ou **pipe**
      (`meu_comando 2>&1 | zavik diagnose`). Só lê stdin se não for terminal.
- [x] ~16 regras de padrão conhecido (linguagem-agnósticas), Rust local e
      determinístico, zero token. Parte dinâmica extraída do próprio erro:
      `-lssl` → `libssl-dev`, `No module named 'requests'` → `pip install
      requests`, `cargo: not found` → toolchain. Regras de sintoma que engana
      (link, node-gyp) marcadas com `note` explicando que é só consequência.
- [x] Fallback honesto: se nenhuma regra casa, devolve as linhas com mais cara
      de erro (resumo) em vez de fingir que entendeu.
- [x] `--json` (schema 1, stdout puro).

> Decisão: **`diagnose` sobre texto**, não `zavik run <comando>`. Assim o
> Zavik não ganha uma superfície nova de execução arbitrária (coerente com o
> postura de segurança); a IA/usuário já tem o erro e só quer a tradução.

### 2.5 Cache determinístico
- [ ] Chamadas repetidas não re-detectam (detecção é pura e barata).

### 2.6 Briefing de uma chamada (`context`) — economizar exploração
- [x] **Feito.** `zavik context [--json]` junta `detect` + `trust` + um
      **runbook** (setup/build/test/run/lint, de `scripts` do `package.json`,
      alvos de `Makefile`/`justfile`, wrappers `mvnw`/`gradlew`, convenções por
      linguagem) + **entrypoints** + **arquivos-chave** (manifesto, lockfile,
      CI, Dockerfile, `.env`, README). Troca N `glob`/`grep`/leitura de uma IA
      por **1 JSON**. Leitura pura, não executa.
- Limitação honesta: o runbook é **inferido** (comando mais provável, não
  garantia); CI não é parsed de YAML (só listada).

---

## 1.5 Detecção — melhorias descobertas testando em projetos REAIS
Testado em 12 projetos reais (anyhow, cobra, itsdangerous, cobra, junit4,
polyfill-mbstring, sinatra, Newtonsoft.Json, dart-lang/args, phoenix,
swift-argument-parser, googletest). Achados:
- [x] **Bug corrigido**: Makefile sozinho virava falso C/C++ (ex.: `cobra`/Go).
      Agora exige fonte C/C++ real ou CMakeLists.txt.
- [x] **Descoberta de projeto aninhado/monorepo** (`--all`): varredura recursiva
      opcional que acha manifesto em subpasta (`.csproj` em `Src/...`,
      `package.json` em `pkgs/...`). Pula dirs de dependência/build
      (`node_modules`, `target`, ...) com limite de profundidade/quantidade.
      Padrão continua olhando só a raiz (comportamento estável).
- [ ] Cobertura de frameworks: nos testes reais só o Phoenix acusou framework;
      ampliar/validar o matching de frameworks contra manifests do mundo real.
- [ ] **Corpus de validação por ecossistema (design `docs/design-corpus.md`,
      aprovado 17/10)** — transformar o teste-em-projeto-real (que achou o
      bug do Makefile e os 4 do dogfooding) em prática permanente: projetos
      reais **pinados em SHA**, partial clone com cache, **sem vendorizar**
      código de terceiro (repo vira público no PH), corpus **só read-only**
      (`run`/`prepare`/`up` **nunca** rodam código alheio — doutrina de
      permissão), ground truth anti-circular (lockfile = verdade de
      versão + metadados curados por projeto), testes `#[ignore]` fora do
      `cargo test` padrão (segue offline). Fases: A Rust → B npm/Python/Go
      → C diagnose com stderr reais → D monorepo não-Rust (decidir
      implementar ou documentar). Seed: os12 do §1.5.

---

## 3. Produto / monetização (a cunha: supply-chain audit)

### 3.1 `zavik audit` — auditoria de supply-chain ✅
- [x] **Feito.** `zavik audit [--online] [--fail-on sev] [--policy] [--ignore]
      [--json]`. Padrão dos consolidados: **Socket.dev** (risk factors +
      confluência de score), **Snyk/cargo-audit** (policy + `fail_on` + exit
      code), **OWASP** (supressão de FP com motivo + expiry).
- [x] **Motor puro** em `plugins/src/audit.rs`: classificador de hook benigno vs
      suspeito (`node-gyp rebuild` OK; `curl|sh`/base64/credencial = crítico) +
      score por **confluência** (nenhum sinal isolado é veredito; limiar 25 evita
      alertar range normal ou hook benigno). 8 testes offline (incl. esbuild real
      NÃO alertado, `curl|sh` → critical, allowlist, supressão vigente/expirada).
- [x] **Segurança**: supressão **exige expiração** (ignorar nunca vira escudo
      eterno); política/supressão em TOML versionado; `--fail-on` dá exit 1 que
      gateia CI; nada é executado (só leitura de manifest/registry).
- [x] Enriquecimento online npm (hook + downloads) via bridge
      `npm_audit_signals`; offline pega origem (registro/git/path) + pin.
- Limitações honestas: analisa o **texto** do script, não os arquivos que ele
  invoca; typosquat por similaridade de nome e integridade de lockfile ainda
  não implementados (ver 1.4).
- [x] **SARIF 2.1.0** (`--sarif`): achações viram `results[]` com `ruleId`
      (fator), `level` (sev→note/warning/error) e `location` = manifesto + **linha
      exata** (lookup lazy, sem regex/dependência). Lido pelo GitHub Code
      Scanning → anota o PR inline. Validado (linha 4 do package.json).
### 3.2 GitHub Action + empacotamento ✅ (templates prontos; validação em GH pendente)
- [x] **`action.yml`** (composite): baixa binário da Release → verifica
      `sha256` (input opcional ancora o hash no action) → roda
      `audit --sarif --fail-on` → publica SARIF **antes** do gate (anotação
      aparece mesmo quando o PR reprova) → exit code final. Inputs via `env:`
      (imune a script injection); ref pinado por tag.
- [x] **`.github/workflows/release.yml`**: binários por tag `v*` (linux
      x64/arm64, macOS arm64, windows) + `.sha256` como asset.
- [x] **README**: workflow de exemplo do consumidor + limitações honestas
      (SARIF inline exige Code Scanning; gate por exit code funciona sempre;
      Windows só download manual).
- Pendência (requer repositório no GitHub): publicar a `v0.1.0` e validar o
  action de ponta a ponta num PR real.
- Próximos (futuro): typosquat por similaridade, integridade de lockfile,
  advisory DB (GHSA) online.

### 3.3 Monetização — fundação de licença offline (fase 1, custo $0) ✅
- [x] **Modelo decidido**: open core + assinatura. CLI grátis (cunha de
      adoção + confiança); gate server-side só quando a receita da fase 1
      bancar o backend (fase 2). Dados frescos = moat (código a IA reescreve).
- [x] **`zavik license status|install|remove`** (+ `--json`): token
      `ZV1` = payload (plan/holder/issued/expires) assinado **Ed25519**; o
      binário carrega só a pública; verificação 100% local (offline-first
      mantido). 5 testes de motor + E2E manual (válido/adulterado/expirado/
      chave-errada/lixo — todos rejeitados corretamente).
- [x] **Ferramenta de cunho** `examples/license_tool.rs` (`genkey`/`mint`) —
      não entra no binário; chave privada fica com o vendor, **nunca** no repo.
- Limitações honestas: gate anti-honesto (patch burla), não defende relógio
  atrasado; hoje ela bloqueia **apenas** `data update` — o resto do CLI,
  incluindo `context`, segue 100% grátis.
- [x] **Dataset de risco fase 1 — `zavik data status|update`** ✅
      (design + amendments: `docs/design-data.md`; staging 1–3, 5, 6 feitos):
      envelope Ed25519 com **chave separada** da licença (privada em
      `~/.zavik-dataset.key` + secret `ZV_DATASET_KEY` — nunca no repo);
      `status` livre/offline (ausente / inválido / idade / stale >14d);
      `update` gate de licença **antes da rede** (`no_license`/
      `license_invalid`, exit 1), download `https`/`file://` com teto de
      32 MB, verificação → escrita atômica (recusa deixa o anterior
      intacto); `audit` ganha seção `dataset_findings` — kill list **exata**
      → `high`, advisory só por **nome** → `low` (sem fuzzy, sem
      range-matching) — que **não toca** `score`/`passed`/`--fail-on`; sem
      dataset, o JSON do audit é **byte idêntico** (invariante testada).
      24 testes novos (15 unit + 9 integração; total 123) + E2E real com as
      chaves de produção: update → status → audit com a seção → dataset
      adulterado recusado com o anterior permanecendo.
- [x] **Dataset fase 1 completa (staging 4)** ✅: builder
      `license_tool build-dataset` (GHSA **reviewed** só, janela 90d;
      kill list por 4 patterns de título medidos nos 389.683 arquivos
      reais → 61 nomes; exclui título version-scoped = anti-FP) +
      workflow `.github/workflows/dataset.yml` (cron diário + dispatch,
      clone esparso, build → sign `ZV_DATASET_KEY` → **verifica contra a
      chave do binário antes de publicar** → asset `dataset-latest` via
      `gh`, sem action de terceiro). 11 testes + E2E real (36.885 GHSAs →
      750 KB → audit com kill list real). Ativação executada: repo
      `Jhongamers/zavik`, secret `ZV_DATASET_KEY`, `DEFAULT_DATASET_URL`
      preenchido; dispatch do workflow + E2E local verdes (license `ZV1`
      → `data update file://` → audit com seção FRESH DATA). Download
      direto do URL embutido resolve com repo público (asset privado
      hoje → 404 honesto). Falta: tag `v0.1.0`.
      Upgrade opcional (founder): corpus OSV `ossf/malicious-packages`
      (238k nomes ≈ 28 MB/asset vs ~1 MB hoje) — ver design-data §2.
- [ ] Action no GitHub Marketplace (billing deles, compute do cliente) —
      após publicar repo + tags.
- [ ] Gate server-side (GitHub App, dashboard, trilha de auditoria) — só
      com receita da fase 1 pagando a infra.

## 4. Lançamento & monetização (plano detalhado: `docs/plano-lancamento.md`)
- Modelo: open core + assinatura (grátis: CLI toda; pago: dataset fresco +
  serviço). Fases, fluxo de cobrança (MoR → chave via expiry) e kit de
  Product Hunt documentados no plano.
- [x] **Princípio de UX (decidido)**: etapas isoladas E composto (`up`) —
      ver `docs/plano-lancamento.md` 5.0 (WorkflowEngine + gates por etapa).
- [x] **`up` v1 implementado** (design + amendments: `docs/design-up.md` §12):
      pipeline `[clone?] → audit → prepare (trust embutido) → run` reusando os
      comandos isolados inteiros (mesmo código, mesmos gates); `run_prepare` →
      `PrepareOutcome::{Completed,Aborted}` — trust/toolchain/instalação
      abortada interrompe ANTES do `run` (nunca executa código após aborto);
      `--until/--skip/--dry-run`; clone seguro (`--depth 1`, sem submódulos,
      `-c core.hooksPath=/dev/null`, nunca sobrescreve destino; `file://` ou
      terminando em `.git`; `--skip clone` é sempre erro); JSONL no `--json`
      (audit real, prepare=plano, run nega sem TTY — modo IA não executa
      risco). 17 testes novos (10 unit + 7 integração), E2E real com repo git
      local: clone + pipeline completo saiu 0; trust negada interrompeu antes
      do `run`; destino existente → erro honesto + exit1; non-TTY → nega.
- [x] **i18n da CLI (decidido, implementar ANTES do PH)** ✅ **CONCLUÍDO
      18/10 (fases 1–3)** — **FASE 1 ✅
      (17/10**, design `docs/design-i18n.md`): infra no shared (`Lang`,
      `t`/`t_fmt` — pt-BR é a chave, `EN` é o catálogo, fallback honesto =
      identidade), resolução `--lang` > `ZAVIK_LANG` > locale >
      default; `--lang` extraído **pré-dispatch** (vale pra `license`/
      `data`), respeita flags de valor e para em `--`; migrados na fase 1:
      `section()` (17 cabeçalhos), banner, saída humana do `env`, prompts
      do gate de permissão, títulos de etapa do `up`; +14 testes (paridade
      e placeholders do catálogo, ordem de resolução, integração: flag/env/
      default/inválido/JSON independente/pós-`--`). **FASE 2 EM CURSO
      (17/10)**: corpo do `audit` (incl. "SEM DADOS" anti-false-positive)
      + `help` completo migrados (consts `HELP_PT`/`HELP_EN` — a chave é o
      texto emitido, sem drift), decisão registrada de prose de JSON em EN
      forçado via `t_en` (achado do dogfooding, `docs/dogfooding.md`);
      ✅ `diagnose` (17/10, incl. plugins), ✅ `run` (17/10) e ✅
      `data`/`license` + SARIF p/ EN (17/10, `message_in(lang)` pros erros
      compostos); migrados também `up` (corpo/erros/clone construction-lang
      p/ JSON), `analyze`, `print_stacks`/`print_execution`, `context`
      (envelopes) e o corpo do `prepare` (confiança/toolchain/execução);
      restos de dispatch/parse migrados (logger, plugins, policy, parse) —
      varredura de residuais no main.rs: ZERO. **Reautoria de conteúdo ✅
      (18/10)**: `evidence` + `finding/risk message` reescritos em EN
      (classe conteúdo — nunca traduzido em runtime; teste trava o split
      shell-`--lang`/conteúdo-EN). **FASE 3 ✅ (18/10)**: sweep final
      (`KERNEL:`/`EVENTBUS:` dos crates kernel/events + prefixos do
      `Display` de erro do shared) + **flip do default `en`** + testes
      atualizados (default pinado como EN, lado pt coberto por `--lang
      pt-BR` explícito, helpers de teste removem locale — máquina BR não
      vira flake) + README. Catálogo hand-rolled
      (`pt-BR` + `en`, zero dependência — mantém offline-first), idioma
      automático pelo locale (`LC_ALL`/`LANG`) com override `--lang
      pt-BR|en`, **padrão `en`** vigente desde o flip (o default era
      `pt-BR` durante a migração, pra nunca sair texto misto).
      Regras: **JSON/SARIF/exit codes nunca traduzem** (contrato travado em
      en); identificadores, comentários e rustdoc **em inglês desde o 1º
      commit** (migração feita antes de versionar). Custo: extrair ~100+
      mensagens soltas em `println!` + taxinha contínua de 2 entradas por
      mensagem nova. Racional do padrão en: erro técnico em inglês é
      pesquisável; PH/issues são em inglês.
- [x] **README em inglês como principal** ✅ (16/10): `README.md` traduzido
      (EN) + cópia em pt-BR (`README.pt-BR.md`, sincronizada nos **marcos**,
      não a cada commit — custo honesto de manter dois textos). Posicionamento
      **agent-agnóstico**: seção "Works with any agent" (Claude Code, Codex,
      OpenCode, Cursor, Antigravity, aider, Copilot) como **compatibilidade,
      não filiação** — o título antigo dizia "OpenCode Agent" (corrigido;
      associação a um único agente limitaria o mercado e induziria a erro).
      Nota honesta no README: mensagens humanas seguem pt-BR até o i18n.
- [ ] **Lançar no Product Hunt** (repo aberto + LICENSE + GIF no topo + tag
      v0.1.0; terça–quinta manhã EUA; primeiro comentário com limitações
      honestas).
- [ ] **Matriz de distribuição (F0–F3) — 3 portas: instalador, prompt
      (gerenciador), IA/Action**. Recomendado por público marcado ✅;
      página do Chocolatey olhada (convenção padrão `choco install
      zavik` — sem inspiração extra necessária):
  - **F0 (founder — bloqueante de todas)**: `git push -u origin main` +
    `git tag v0.1.0 && git push origin v0.1.0` → `release.yml` publica
    binários (linux x64/arm64, macOS arm, windows) + `.sha256`.
  - **F1**: `install.sh` ✅ **recomendado (usuário final, inclui WSL e
    macOS)** — **FEITO** (17/10): detecta OS/arch, baixa da Release, confere
    sha256 (sem flag de pular — adulterado é recusado, com teste), não
    interativo; overrides `ZAVIK_VERSION`/`ZAVIK_INSTALL_DIR`/
    `ZAVIK_BASE_URL`; 4 testes (sintaxe POSIX, E2E via `file://`
    instalando binário funcional, recusa de checksum divergente, SO
    desconhecido com mensagem clara). Versão manual de 2 linhas documentada
    ao lado (comemos o pastel: nosso próprio `audit` classifica `curl|sh`
    como risco) + seção "Install" do README escrita pra humano E pra IA
    (linha canônica copiável por OS — a IA usa o mesmo prompt). Ativa quando
    a tag `v0.1.0` publicar os binários (F0).
  - **F2**: `cargo install` ✅ **recomendado (dev com Rust já instalado)**
    + testar `action.yml` no E2E do release — caminho zero-install de CI
    (é o que a IA escreve num workflow).
  - **F3** (conforme adoção): brew tap, `npx` (binário por plataforma),
    winget/scoop, **`choco install zavik`** ✅ **recomendado (Windows
    nativo)** — até lá o `.zip` da Release é a porta Windows.
- [x] **5.1 `zavik run` v1 — FEITO**: resolve start (declarado > convenção >
      perguntar), readiness heurístico (heurística de porta + `/proc` com
      descendentes — pega o neto npm→node), SIGTERM→SIGKILL no **grupo**,
      Ctrl-C gracioso, crash cedo → `diagnose`, gate crítico (humano decide),
      `--dry-run`/`--timeout`/`--health`/`--`/`--json`. 23 testes novos
      (18 unit + 5 integração). Desenho: `docs/design-run.md`.
- [x] **5.1.1 `run` v1.1 — `--detach` FEITO** (17/10): sobe em **sessão própria**
      (`setsid` — sobrevive ao terminal), devolve `pid` + `log`
      (`.zavik/run.log`) + `stop_command` no `--json`; sem supervisão de
      readiness (limitação honesta documentada); mesmo gate crítico (nem
      `--yes` cobre — teste trava o invariante); no Windows fica preso ao
      console (limitação conhecida). Testes: 3 unit (sessão própria provada
      via `getsid`, log capturado, falha vira erro) + 2 integração (negado
      sem humano mesmo com `--yes`, dry-run não executa). Resta de v1.1:
      health https.
- [ ] **5.2 `git clone` integrado** — **absorvido pelo `up <url>`** (o passo de
      clone do pipeline substitui um comando isolado `from`; ver
      `docs/design-up.md`). Clone seguro: sem hooks (`core.hooksPath=/dev/null`),
      `--depth 1`, sem submódulos, via CommandRunner (sem shell) + trust check
      antes de qualquer execução.
- [ ] **5.3 `zavik why <pacote>`** (analisada, aguardando desenho/aprovação):
      árvore de dependências reversa + origem + fatores de risco do `audit` num
      JSON só. Existe demanda real (`npm why`, `cargo tree -i`,
      `why-is-installed`); humano debuga na mão, IA substitui 5 comandos por 1
      chamada. Read-only, zero execução, reforça o cunha de supply-chain.
- [x] **5.4 `zavik env --json`** ✅ (16/10): fingerprint do ambiente numa
      chamada — `os`/`arch` + toolchains por stack detectado com presença
      (PATH) e **versão real** (`<tool> --version`, argv sem shell; falha →
      `version: null`, nunca inventa) + lista `missing`. Mata "works on my
      machine", é 1 call de IA em vez de sondar 10 comandos, e alimenta o
      `diagnose` com evidência de ambiente. Read-only, sem gate (não instala
      nada, não lê `.env`). Humano: seções por stack + sugestão de instalação
      do que falta. 4 testes novos (presença+versão, dedupe JS/TS, versão
      nunca inventada, integração --json/PATH vazio).

---

## Notas / princípios já estabelecidos (manter)
- **Correção automática é FORA** (decisão do founder, out/2026): executar
  consertos pode quebrar algo que escapa do nosso controle — multi-linguagem ×
  framework = blast radius imprevisível. O produto só **propõe** (o `diagnose`
  já entrega o "conserto"); qualquer `--fix` futuro seria restrito a regras
  inequívocas atrás de gate individual, nunca execução automática.
- **Idioma**: identificadores, testes e comentários em **inglês**; mensagens
  humanas da CLI em **pt-BR + en** com default **en** desde o i18n de 18/10
  (ROADMAP §4); contrato de máquina (JSON/SARIF/exit codes) **sempre en**.
- Injeção de comando é **impossível por construção**: sem shell
  (`Command::new(prog).args([...])`), imagem Docker saneada `[a-z0-9._-]`.
- Riscos de execução de código exigem confirmação **individual** —
  "sim para todas" e `--yes` **nunca** cobrem isso (protege o caso de IA dirigir).
- Sem terminal (não interativo) → **nega por padrão**.
- Arquivos existentes **nunca** sobrescritos sem `--force`.
- Ser honesto nas limitações: não prometer "roda em tudo sem erro".
