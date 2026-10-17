# Design — i18n (pt-BR + en)

Status: **fase 1 em execução (17/10)**. Decisão do fundador: antes do launch
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

- **Fase 1 (atual)**: infra (`Lang`, `t`, `t_fmt`, extract, resolve), e a
  migração do conteúdo "de topo": `section()` (17 cabeçalhos — a função
  inteira passa por `t`, os call sites não mudam), `banner`, saída humana
  de `env`, prompts de `permission`, títulos de etapa do `up`.
  Testes: paridade/unicidade do catálogo, ordem de resolução, integração
  (`--lang en` troca cabeçalho, default segue pt-BR, env var funciona,
  `--lang` inválido barra, JSON não muda).
- **Fase 2**: corpos dos comandos (`audit`, `run`, `prepare`, `context`,
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
  corrente. Implementar junto da migração do `diagnose` (fase 2).

## 5. Fora de escopo (por ora)

- Pluralização/gênero (não temos mensagens com plural em contagem).
- `format` specs dentro de catálogo (compor número fora — ver §2).
- Idiomas além de pt-BR/en (a API não muda pra adicionar: enum + catálogo).
- Tradução de docs (README EN é primário; pt-BR sincronizado por milestone).
