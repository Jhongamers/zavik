# Design — `zavik up`

> Desenho (out/2026). Status: **IMPLEMENTADO** (v1 — 17 testes novos, E2E real).
> Contexto: princípio 5.0 do `docs/plano-lancamento.md` (toda etapa funciona
> isolada E composta), aprovado pelo founder. Absorve o antigo 5.2 (`from <url>`).
> Amendamentos feitos durante a implementação estão na seção 12.

## 1. Objetivo

`zavik up [url|path]` = o pipeline inteiro numa comando só, com os **mesmos
gates das versões isoladas** (nada de porta dos fundos):

```
[url: clone seguro] → audit (relatório) → trust check (bloqueio) → prepare (gate) → run (gate, foreground)
```

Regra-mãe: **cada etapa é exatamente o comando isolado** — mesma resolução,
mesmos avisos, mesmo comportamento de permissão. Compor não pode mudar
segurança.

## 2. Alvo

- `zavik up` → pipeline no diretório atual (começa no audit).
- `zavik up <path>` → idem em outro diretório.
- `zavik up <url>` → clone (passo 0) e pipeline no clone.
  - Detecção de URL: prefixos `http://`, `https://`, `git@`, `ssh://`, `file://`
    (testável offline), ou terminando em `.git`. Qualquer outra coisa é path.

## 3. Passo de clone (só com URL) — segurança primeiro

Execução direta via `std::process::Command` (argv, **sem shell**):

```
git -c core.hooksPath=/dev/null clone --depth 1 <url> <destino>
```

- **Sem submódulos** (não passamos `--recurse-submodules`), **shallow**
  (história não é código executável, mas reduz superfície/tempo).
- **Hooks**: `-c core.hooksPath=/dev/null` antes do subcommand é
  defense-in-depth contra hooks de *template* local/sistema — hooks do repo
  remoto não viajam em clone (vêm do template do `git init`), e os vetores
  reais (submódulo, filtros LFS/checkout) exigem config que um clone não
  copia. Custo zero, mantemos a trava.
- Destino: `./<nome-do-repo>` no cwd. Se já existir → erro honesto e nada
  executado (não sobrescrevemos; sem `--force` no clone v1).
- git ausente → erro honesto + sugestão (o `diagnose` tem a regra).
- Falha do clone interrompe **antes** de qualquer outra etapa.
- Após clone, o trust check (passo 3) roda normalmente — repo malicioso é
  pego antes do prepare.

## 4. Gates — a composição não engole ninguém

| Passo | Gate | `--yes` cobre? | non-TTY |
|---|---|---|---|
| clone | nenhum (baixa só; sem execução de código do repo) | sim | segue (git falha sozinho) |
| audit | nenhum (read-only) | — | — |
| trust check | **bloqueio existente** (`pre_flight_trust`) | não | nega |
| prepare | gate por passo (como hoje) | cobre não-críticos (como hoje) | nega |
| run | **gate crítico (como hoje)** | **NUNCA** | nega |

- Implementação: trust **roda dentro do passo `prepare`** (reuso literal do
  caminho isolado — zero gate duplo). `run_prepare` passou a devolver
  `PrepareOutcome::{Completed,Aborted}` e o `up` **interrompe o pipeline em
  `Aborted` ANTES do `run`** — trust negada, toolchain ausente ou instalação
  falhada nunca chegam perto de executar código do projeto. O prepare isolado
  inalterado (continua exit 0 no aborto).
- `--dry-run` → mostra o **plano de etapas** (com o que seria executado em cada
  uma) e não executa nada.

## 5. Flags novas (as existentes continuam globais)

- `--until <etapa>` → para depois daquela etapa (`clone|audit|prepare|run`).
  Ex.: `up --until audit` no CI.
- `--skip <etapa>` → pula a etapa (ex.: `--skip run` = só preparar).
  `--skip clone` é **sempre erro** (sem URL não há clone; com URL o clone é o
  próprio alvo — pular rodaria o pipeline no cwd, silenciosamente errado).
- Globais já valem: `--json`, `--dry-run`, `--yes`, `--online`, `--force`.
- Sem flag nova de audit: dentro do `up` o audit roda **relatório
  (advisory)**; quem gateia severidade é o comando `audit` dedicado
  (`--fail-on`). O bloqueio de segurança dentro do `up` é o **trust check**.

## 6. Orquestração — onde o código vive (decisão técnica)

O registro 5.0 dizia "WorkflowEngine"; a API atual
(`run_workflow(steps: &[String], context, detector)`) só roda **steps de
detecção** com retry — não comporta closures com gates. Decisão honesta:

- **v1: composição na camada CLI** — `run_up()` reusa `run_audit()`,
  `run_prepare()` e `run_run()` inteiros (que já têm gates, JSON e exit codes).
  WorkflowEngine continua no que ele faz bem (fluxos de detecção).
- Se um dia o WorkflowEngine ganhar steps com gate, migrar.

Isso é o "isolado ⇒ composto" **literal**: os mesmos caminhos de código.

## 7. Contrato `--json` — JSONL (desvio documentado)

`up --json` emite **um objeto JSON por linha** (JSON Lines) em stdout, um por
etapa conforme completa, terminando com o objeto final
`{"schema":1,"command":"up","status":"ok|failed","steps":[...],"exit_code":N}`.

- Por que não um objeto só: reuso das funções existentes sem refactor de
  retorno; consumidor (IA) lê progressivamente.
- Cada linha é JSON válido com `schema:1` — o "stdout puro" continua puro
  (zero ruído de terminal). Documentar no README.
- **Nuance implementada**: o objeto final é impresso quando o controle volta
  ao `up`; etapa que **termina o processo** com falha (negativa de gate no
  `run`, exit do `run`) já imprimiu a própria linha de status antes — o
  consumidor trata a última linha como fonte de verdade.
- **Modo IA nunca executa risco**: no `--json`, o audit entrega resultado
  real, o prepare entrega o **plano** (branch existente de prepare `--json`,
  não executa) e o `run` nega sem TTY. Quem executa é o humano (regra-mãe).

## 8. Exit codes

Propaga o da primeira etapa que falhar; `up` nunca mascara:
audit advisory = 0 (ele próprio falha só com `--fail-on`, que aqui não passa);
trust negado = 1; prepare = exit do prepare; run = exit do run (inclui 130 do
Ctrl-C).

## 9. Testes (obrigação)

- **Unit**: seleção de etapas (`--until`/`--skip` — função pura), URL vs path.
- **Integração** (binário real, sem executar código de projeto):
  - `up --dry-run --json` num fixture → plano completo com todas as etapas.
  - `up --until audit --json` → só audit roda; etapa seguinte marcada skipped.
  - non-TTY + prepare sem `--yes` → nega e interrompe (exit 1).
  - path inexistente → erro honesto (exit 1).
- **E2E manual**: repo git local (`git init` num fixture) → `up <path>` e
  `up file://...`/path-clonável com gate por pipe; confere clone sem hooks.

## 10. Limitações v1 (dizer na cara)

- `run` final é **foreground**: o `up` termina quando o app termina (é o
  esperado no uso dev; `--detach` é v1.1).
- Sem resume/checkpoint: falhou, roda de novo (o prepare é idempotente o
  bastante na prática; audit é barato).
- Clone só de git remoto/path gitável; `--depth 1` fixo.
- macOS/Windows: mesmas regras do `run` (POSIX v1).

## 11. Staging

- **up v1**: tudo acima (path + URL/clone + flags + JSONL + testes).
- **v1.1**: `--detach` (herdado do `run`), talvez `--into <dir>` pro clone.

## 12. Amendamentos da implementação (out/2026)

- `run_prepare` → `PrepareOutcome` (ver seção 4): sem isso, trust negada seria
  soft-stop (`Ok(())`) e o `up` teria **rodado o `run` depois de uma abortação
  de segurança** — bug pego no desenho, corrigido no código.
- Etapas do plano: `clone? → audit → prepare → run` (trust embutido no
  `prepare`; `audit` sempre advisory dentro do `up`).
- Lógica pura em `crates/cli/src/up.rs` (`UpStep`, `up_plan`, `is_git_url`,
  `repo_name` — 10 testes unitários); orquestração em `run_up` (`main.rs`).
- Clone: mensagem honesta de destino existente + `diagnose` quando git falta;
  progresso do git vai pra stderr (stdout JSONL fica limpo).
- E2E real: clone `file://` + pipeline humano completo (`--skip run`) saiu 0;
  trust negada (fixture `postinstall: curl|sh`) interrompeu antes do `run`;
  destino existente → erro + exit 1; non-TTY → nega + exit 1.
