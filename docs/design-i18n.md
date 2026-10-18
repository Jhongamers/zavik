# Design — i18n (pt-BR + en)

Status: **fase 3 concluída (18/10)** — infra (fase 1), runtime (fase 2),
reautoria de conteúdo em EN, sweep final (kernel/events/Display) e **flip do
default `en`** fechados. Decisão do fundador atendida antes do launch do
Product Hunt: `--lang` com catálogo **pt-BR + en** e **default `en`**; pt-BR
segue disponível via `--lang pt-BR`, `DEVFORGE_LANG` ou locale BR — nenhuma
mensagem troca de idioma sem o usuário pedir.

## 1. O que traduz e o que NÃO traduz

| Traduz | NÃO traduz (sempre inglês) |
| --- | --- |
| Mensagens humanas do CLI (stdout/stderr legível) | JSON (`--json`), SARIF, exit codes |
| Help, prompts de permissão, títulos de seção | Chaves/valores dentro dos JSON |
| Erros humanos de parse | Logs de evento (`EVENTBUS`), nomes de campos |
| Texto do `diagnose` (causa/conserto) | Identificadores, comentários, docs-fonte |

Rationale: o contrato de máquina é a interface de integração — mudar idioma
de chave JSON quebraria consumidores. Idioma de máquina = sempre inglês.

## 2. Arquitetura: catálogo com **pt-BR como chave** (estilo gettext)

- Fonte = **pt-BR**: a string literal pt no call site É a chave.
- Catálogo `EN: &[(&str, &str)]` mapeia `pt-BR → en`.
- `t(msg)`: em `en`, traduz se existe entrada; senão **identidade** (fica
  pt-BR até ser migrado — faseamento honesto, sem inventar texto).
- `t_fmt(msg, &[args])`: substitui `{}` na ordem — templates com argumentos
  traduzem ANTES de formatar (nunca formatar-antes-de-traduzir: a chave
  seria o texto com valores embutidos e nunca casaria).
  Limitação aceita: sem specs de formato (`{:.1}`) — a mensagem compõe o
  número fora do template.
- Vantagem: **call sites não mudam durante a migração** — migrar = adicionar
  1 linha ao catálogo + envolver o `println!` em `t()`/`t_fmt()`. Sem chave
  derivada que quebra com refactor de texto.

Onde vive: `crates/shared/src/i18n.rs` (shared é a camada interna — CLI,
plugins e core dependem dele; o `diagnose` em plugins entra na fase 2).

## 3. Resolução de idioma

Ordem: **`--lang <pt-BR|en>` > env `DEVFORGE_LANG` > locale do sistema >
`DEFAULT_LANG`**.

- Locale (decisão registrada no ROADMAP §4): `LC_ALL` > `LC_MESSAGES` >
  `LANG`; família `pt` (pt_BR, pt_PT, pt-BR…) → `pt-BR`; **qualquer outro
  locale cai no default** — que é `en` desde o flip (18/10), produzindo
  exatamente a regra "padrão en quando o locale não é BR". O locale `pt`
  só tem efeito de opt-in de pt-BR, nunca muda o default sozinho.
- `--lang` é extraído dos args **antes do dispatch** (helper
  `extract_lang`), então vale para TODOS os comandos — inclusive
  `license`/`data`, que fazem parse próprio. O extractor respeita flags de
  valor (`--message`, `--url`, …) e para em `--` (args após `--` são do
  comando filho, não nossos). Aceita `--lang=x` e `--lang x`.
- `DEVFORGE_LANG` existe para agentes/CI/testes (configurar sem tocar no
  comando). Valor inválido em qualquer nível → erro com as opções válidas,
  exit 1 (nunca silencioso).
- `DEFAULT_LANG` é **uma constante só** — o flip final `PtBr → En` é 1 linha
  + atualização dos testes que travam o default (fase 3).

Estado global: `OnceLock<Lang>` (a CLI é single-run; testes de unidade
usam a API pura `translate(lang, msg)`, testes de integração são processos
separados).

## 4. Fases (cada fase = commit com testes)

- **Fase 1 (concluída)**: infra (`Lang`, `t`, `t_fmt`, extract, resolve), e a
  migração do conteúdo "de topo": `section()` (17 cabeçalhos — a função
  inteira passa por `t`, os call sites não mudam), `banner`, saída humana
  de `env`, prompts de `permission`, títulos de etapa do `up`.
  Testes: paridade/unicidade do catálogo, ordem de resolução, integração
  (`--lang en` troca cabeçalho — na fase 1 o default ainda era pt-BR, env
  var funciona, `--lang` inválido barra, JSON não muda).
- **Fase 2 (em curso)**: corpos dos comandos (`audit`, `run`, `prepare`, `context`,
  `diagnose`, `analyze`, `license`, `data`), help completo, strings de
  erro de parse, `diagnose` em plugins, hints de toolchain.
- **Fase 3 (pré-PH, obrigatória)**: migração completa verificada
  (busca por literal pt em `println!` ≈ 0 fora do catálogo),
  **flip do `DEFAULT_LANG` para `En`**, testes atualizados, README:
  `--lang` documentado com a regra "default en, pt-BR via flag/env".

Mensagem humana não migrada no meio do caminho aparece em pt-BR mesmo com
`--lang en` — **limite aceitável enquanto o repo é privado**; some na fase 3,
que é gate de lançamento.

### Status da migração (atualização 18/10)

- **Fase 2 em curso**: migrados o corpo humano completo do `audit` (incl.
  veredito "SEM DADOS") e o `help` inteiro. O help foi movido para as
  consts `HELP_PT`/`HELP_EN` no shared — **a chave do catálogo é o texto
  emitido** (const compartilhada), impossível driftar; `--lang` documentado
  nos dois idiomas. Cada superfície ganhou teste de integração `--lang en`.
- **Decisão (dogfooding, `docs/dogfooding.md` #6)**: *prose dentro de JSON*
  (`diagnose.summary`, `causes[].cause`/`fix`, hints) sai em **inglês
  forçado** via `t_en()` — tradução direta do catálogo, ignorando o
  `--lang` (contrato "JSON sempre inglês": chaves/enums já eram EN; a
  prose que máquina lê também deve ser). Saída humana segue o idioma
  corrente. **Feito no `diagnose` (17/10)** — ver próximo item.
- **`diagnose` migrado (17/10, fechando o maior item da fase 2)**:
  `diagnose_in(lang, …)` compõe causa/fix/note/summary **no idioma-alvo na
  construção** — templates traduzem ANTES do `format` (`t_fmt_in`, fixes
  dinâmicos com `{}` posicionais), porque um summary já composto nunca mais
  casaria com a chave. Duas travas independentes no caminho do JSON: a
  construção recebe `Lang::En` (garante o summary composto) **e** os
  emitters aplicam `t_en()` (garante textos estáticos de qualquer caminho).
  Humano = idioma corrente + rótulos (`nota/conserto/provas`) via `t()`.
  Chaves novas: 46 (17 causas, 16 fixes, 3 notes, 3 templates dinâmicos, 2
  summaries, 4 rótulos, 1 erro de input). `evidence`/`top_errors`/ids são
  factos brutos — nunca traduzidos. Testes: unitário EN no plugins,
  `t_en`/`t_fmt_in` no shared, integração `--lang en` + "JSON idêntico sob
  qualquer idioma".
- **`run` migrado (17/10, lote `086a619`)**: `run_run` calcula o idioma uma
  vez (`json ⇒ Lang::En`); caminhos humanos usam `translate`/`t_fmt_in`;
  strings compartilhadas entre humano e campo `error` do JSON são compostas
  no idioma-alvo (logo o `error` do JSON sai EN); helpers 100% humanos
  (`pick_start_candidate`, `readiness_note`) usam `t`/`t_fmt`; specs de
  formato são compostas FORA do template. Teste de integração
  `run_output_follows_lang_flag`.
- **`data` + `license` migrados + SARIF corrigido para EN (17/10, lote
  atual)**: todo o corpo humano de `run_data`/`data_status`/`data_update`/
  `fetch_dataset`/`run_license`; campo JSON `hint` via `t_en`. Padrão novo
  para erros compostos: `DatasetError`/`LicenseError` ganharam
  `message_in(lang)` — os emitters JSON chamam `message_in(Lang::En)` e o
  `Display` delega com o idioma corrente (humano); assim `--lang en` não
  deixa resíduo pt nem no conteúdo dos erros. **O SARIF estava em pt
  (violação pré-existente do contrato "SARIF sempre EN") e foi corrigido**:
  a mensagem composta usa `t_fmt_in(Lang::En, …)` e `factor_label` passou a
  envolver `t_en` (rótulos usados só no SARIF). Chaves novas: ~64. Testes:
  `data_and_license_follow_lang_flag` (status EN, JSON `hint`/`error` EN sob
  default pt).
- **Pendência registrada (fase 3, autoria)**: as strings de `evidence` do
  audit são escritas em pt (`plugins/audit.rs`: "downloads/semana=…",
  "versão não fixada…") mas `evidence` é fato de máquina — nunca traduzido
  em runtime (§1). Como o contrato é "JSON/SARIF sempre EN", a correção é
  reescrever o autoral em inglês antes do launch (mudança de conteúdo, não
  de runtime); wrappers humanos seguem pelo catálogo.
- **Toolchain hints migrados (18/10)**: `run_hints` (provision) e o hint
  composto do `install_hint` (core/toolchain). Padrões: hints estáticos
  traduzem NOS emitters (humano `t`, JSON `t_en` — comandos puros caem na
  identidade honesta do catálogo); o fallback composto
  `instale '{}' (pacote sugerido: {})` recebe `lang` por plumbing
  (`missing_tools(…, lang)` → construction-lang — composição nunca casa com
  a chave no emit). `env` humano passa `i18n::lang()`; o JSON de env não
  leva hint. Testes: `prepare_run_hints_json_is_english_even_under_default_pt`
  e `prepare_run_hints_human_follows_lang_flag` (fixture Python).
- **Provision steps migrados (18/10)**: títulos/reasons dos steps de
  `prepare` (local por linguagem + docker) construídos no idioma-alvo —
  `build_plan(…, lang)` (JSON = `Lang::En`, humano = corrente), porque
  títulos compostos (`Instalar dependências ({})` com `{pm}`) não casam
  chave no emit. ~40 chaves novas. **Bug de método pego no smoke**: uma
  chave minha saiu com placeholder nomeado `{pm}` — `t_fmt_in` só substitui
  `{}` posicional, e o literal vazou cru; corrigido para `{}` e a lição
  virou travamento: teste novo `keys_use_only_positional_placeholders`
  (toda `{` de chave/tradução precisa pertencer a um `{}` literal — pega
  também format specs como `{:?}`). Teste de integração estendido para
  assertar step title EN no JSON sob default pt.
- **up + analyze + printers migrados (19/10)**: corpo humano do pipeline `up`
  (plano/`Alvo`, seção `UP — ETAPA`, clone, resumo com `(pulado)`), erros do
  `up_plan`/`repo_name`, `analyze` (plugin err, linha de registries, `Resumo`
  com contagens, `Nada a reportar`), `print_stacks`, `print_execution`,
  `mode_label` + `PLANO ({})`. `clone_repo` recebe `lang` (construction-lang):
  o erro composto também cai no `"error"` do JSON — travado por
  `up_clone_failure_json_error_is_english` (EN sob default pt; git limpa o
  destino — assert de limpeza incluído). Plano em ambos os idiomas travado
  por `up_dry_run_output_follows_lang_flag`. ~33 chaves novas.
  **Registrado como reautoria fase 3 (classe evidence)**: `finding.message`
  (análise), `risk.message` (confiança, entra no JSON trust/context) e
  labels de `key.kind` — prosa autorada no core; os envelopes ao redor já
  traduzem. `context` ficou só com esses itens (cabeçalhos já eram fase 1).
- **Corpo do prepare migrado (19/10)**: `pre_flight_trust` inteiro (linha de
  confiança limpa, seção de riscos, "só avisos", nota de dry-run,
  "N risco(s) exigem confirmação EXPLÍCITA" + prompt "Aceitar risco"),
  `print_missing_tools` (sugestões), `offer_toolchain_install` (prompts
  "Instalar X", "Vou rodar (sem shell…)", recusa, instalando, sucesso/
  exit/não-rodou) e o corpo do `run_prepare` (stack vazio, "Toolchain
  instalada", aborto de segurança, seção EXECUÇÃO, erros de plugin, nota de
  dry-run). ~26 chaves. Teste `prepare_trust_and_plan_follow_lang_flag`
  trava trust+plano nos dois idiomas. **Limitação honesta**: caminhos
  interativos (senha do sudo/brew, toolchain faltando de verdade) não são
  exercitáveis em non-TTY — o gate nega fora de TTY e os textos ficam
  cobertos por catálogo/paridade. O scan de literais pt pegou restos para
  os próximos lotes: logger "evento de domínio", `plugin 'stack_detector'`,
  erro `--lang exige um valor`.
- **Lote de restos de dispatch/parse (18/10)**: logger de eventos,
  `plugin 'stack_detector'`/`'local_command_runner'`, erros de policy e
  supressão (`--fail-on`, `fail_on` do policy, factor, `expires`), erros de
  parse (`--lang`, `--timeout`, opção desconhecida, caminho duplicado),
  "nenhuma stack detectada pra auditar". Dois achados de composição:
  `spawn_detached` ganhou `lang` — seus erros internos vazavam **em pt como
  argumento** do wrapper já-EN do caller; e `findings_for`/`build_report` do
  dataset ganharam `lang` — os `detail` das exposições/kill-list chegam no
  JSON, então JSON/SARIF constroem em `Lang::En`. ~17 chaves. Teste
  `parse_and_policy_errors_follow_lang_flag`. **Varredura de residuais do
  main.rs agora: ZERO literais pt fora de catálogo/reautoria.**
- **Reautoria de conteúdo em EN (fase 3, content-level, 18/10)**: a classe
  conteúdo (evidence/finding/risk) foi reescrita em EN e **nunca é
  traduzida em runtime** — sai EN até sob o default pt-BR; o shell/UI
  continua seguindo `--lang` (pt por enquanto). `audit.rs` (15: origens,
  hooks, downloads/week, integridade, comportamento), `offline.rs` (6:
  lockfile, versão fixada, origem, versões divergentes), `registry.rs`
  (2: versão mais nova, limite de checagem), `trust.rs` (14: scripts de
  instalação, ausência no registry, downloads, verificação offline) —
 39 trocas literais com validação1×-por-string. Fora da classe conteúdo,
  migração **runtime** do que a varredura do main.rs não alcançava: prints
  do `WorkflowEngine` em `core/src/lib.rs` (`CORE: tentativa` /
  `executando passo` / `concluído` / `transitória` / `falhou` — `{:?}` dos
  steps composto pra `{}` positional antes da chave) e o erro do detector
  (`diretório de projeto não encontrado`) — 6 chaves. Teste
  `finding_and_evidence_content_stays_english_under_pt_default` trava o
  split (shell pt + conteúdo EN sob default). **Registrado pro sweep
  final da fase 3**: linhas `KERNEL:`/`EVENTBUS:` (crates `kernel`/
  `events`), prefixos do `Display` de erro do shared (`erro de I/O:`),
  asserts de teste em pt (não são superfície de usuário) e o `reason`
  do dataset no build (authoring, `docs/design-data.md`).
- **Sweep final + flip do default (fase 3 concluída, 18/10)**: `KERNEL:`
  (registrar/inicializado) e `EVENTBUS:` (publicando) migrados pro catálogo
  via `t_fmt`/`t` — são `eprintln`, então continuam seguros pra JSON; os
  prefixos do `Display` de erro do shared (`erro de I/O` /
  `desserialização` / `desconhecido`) agora leem o idioma global no momento
  do emit (padrão do `LicenseError::Display`) — 6 chaves. **`DEFAULT_LANG`
  virou `Lang::En`** (+ docs do const e do `resolve`). Testes: o default
  passou a ser pinado como EN (`default_language_is_english_after_the_
  phase3_flip`), os lados pt dos testes `*_follows_lang_flag` ganharam
  `--lang pt-BR` explícito (cobertura pt mantida e até mais forte), testes
  de comportamento (audit/data/env) migraram os asserts pro EN, os helpers
  de teste removem `LC_*`/`LANG` do processo (locale BR não vira flake) e o
  teste `t_en` fixa o contraste via `translate(PtBr)` em vez de depender do
  default. Bug de catálogo achado no caminho: o valor EN de `--url exige um
  valor` carregava `<caminho>` em pt → `<path>`. Smoke: default EN em
  `audit`/`detect`/erros, `--lang pt-BR` e locale `pt_BR` seguem pt, e o
  `Display` de erro sai nos dois idiomas. **Migração fases 1–3 FECHADA.**

## 5. Fora de escopo (por ora)

- Pluralização/gênero (não temos mensagens com plural em contagem).
- `format` specs dentro de catálogo (compor número fora — ver §2).
- Idiomas além de pt-BR/en (a API não muda pra adicionar: enum + catálogo).
- Tradução de docs (README EN é primário; pt-BR sincronizado por milestone).
