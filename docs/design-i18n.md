# Design — i18n (pt-BR + en)

Status: **fase 2 em execução (17/10)** — fase 1 concluída (infra + conteúdo
de topo). Decisão do fundador: antes do launch
do Product Hunt o CLI precisa de `--lang` com catálogo **pt-BR + en** e
**default `en`**. Enquanto a migração não fecha, o default continua `pt-BR`
(status quo — nenhuma mensagem muda de idioma sem o usuário pedir).

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
  locale cai no default** — que vira `en` após o flip final, produzindo
  exatamente a regra "padrão en quando o locale não é BR". Durante a
  migração o default é `pt-BR`, então o locale só tem efeito de opt-in
  (status quo preservado, sem saída mista).
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
  (`--lang en` troca cabeçalho, default segue pt-BR, env var funciona,
  `--lang` inválido barra, JSON não muda).
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

### Status da migração (atualização 17/10)

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

## 5. Fora de escopo (por ora)

- Pluralização/gênero (não temos mensagens com plural em contagem).
- `format` specs dentro de catálogo (compor número fora — ver §2).
- Idiomas além de pt-BR/en (a API não muda pra adicionar: enum + catálogo).
- Tradução de docs (README EN é primário; pt-BR sincronizado por milestone).
