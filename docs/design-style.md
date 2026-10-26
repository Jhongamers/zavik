# Design — apresentação do terminal (ANSI, molduras, símbolos)

Status: **implementado (26/10)** — lote fechado com testes, clippy limpo e
invariante `--json` byte-idêntico verificado. Pedido do fundador ("não tem uma
exibição bonitinha" / "pode ajustar tudo igual clis moderna").

## 1. O que muda e o que NÃO muda

| Muda (apenas saída humana) | NÃO muda (contrato de máquina) |
| --- | --- |
| Banner `env/audit/prepare/run/diagnose` → moldura `┌─┐ │ └─┘` | `--json` bytes idênticos |
| Títulos de seção → **bold ciano** + régua `───` dim | SARIF |
| Ícones emoji (✅ ❌ 🛑 ⚠️ ✨ ⏭️) → símbolos Unicode (✓ ✗ ⚠ ℹ ▸ ▶ ↷) | exit codes |
| Árvore de comandos do runbook → `├─` / `└─` com `$` dim | chaves/valores JSON |
| Cabeçalhos de stack → `▸` + `·` | catálogo i18n (textos iguais) |
| Severidade → cor (crítico vermelho, aviso amarelo, info ciano) | |

Regra central: **cor é apresentação**. O texto é o mesmo com ou sem ANSI —
uma tela TTY e um pipe recebem as mesmas palavras, só muda o envoltório.

## 2. Por que ANSI hand-rolled (zero dependência)

Stance de supply-chain do projeto: toda dependência embarcada é superfície de
ataque. Um módulo `crates/shared/src/style.rs` de ~180 linhas resolve sem
puxar `owo-colors`/`colored`/`anstream`. Sem cadeia de dependência nova, sem
versão para auditar, sem `unsafe`.

## 3. Detecção e convenções (no-color.org)

Ordem de resolução em `style::colors_enabled()`:

1. `NO_COLOR` definido (qualquer valor) → **sem cor**, sempre (vence force).
2. `CLICOLOR_FORCE` definido e ≠ `0` → **com cor**, mesmo sem TTY (CI/logs).
3. Senão: stdout é TTY → com cor; pipe → sem cor.

Consequência prática: **pipe = texto puro**. É o que mantém os testes de
integração existentes verdes sem `NO_COLOR` (stdout capturado nunca é TTY).

## 4. Invariantes (testados em `crates/cli/tests/style.rs`)

- `piped_stdout_has_no_ansi` — pipe nunca emite escape.
- `clicolor_force_emits_ansi_even_without_tty` — force funciona sem TTY.
- `no_color_beats_clicolor_force` — `NO_COLOR` vence qualquer force.
- `json_is_byte_identical_regardless_of_color` — `--json` byte-idêntico
  com/sem cor, e **nunca** contém ANSI.
- `styled_human_output_keeps_the_same_words` — com cor, as palavras
  (`ENVIRONMENT`, `project:`) continuam intactas.

Unidade em `style.rs`: símbolos são de 1 célula (não emoji largo, que quebra
alinhamento em logs/CI) e `framed()` adapta a largura ao conteúdo (piso 24,
teto 58 colunas).

## 5. Relação com o i18n

Ícone **nunca** vive dentro da string do catálogo — ele é montado no `main.rs`
via `style::symbol(...)` e envolve o `t(...)`. Assim o par pt/en do catálogo
continua sendo só texto, a paridade do catálogo se mantém, e trocar a paleta
não toca tradução nenhuma.

## 6. Fora de escopo (depois)

- Paleta configurável por tema/config file.
- `--no-color` como flag explícita do CLI (hoje só env `NO_COLOR`).
- Barras de progresso (o `run` ainda é linha-a-linha).
