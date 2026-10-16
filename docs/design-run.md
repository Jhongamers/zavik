# Design — `devforge run`

> Desenho (out/2026). Status: **v1 IMPLEMENTADO** (`devforge run`); pendências
> em staging (v1.1: `--detach`; health https). Contexto: análise 5.1 do
> `docs/plano-lancamento.md`; princípio 5.0 (etapa isolada E composta no `up`).

## 1. Objetivo

Após `prepare` (ou de forma isolada), **iniciar o projeto** em qualquer
linguagem suportada — com a mesma cultura do resto do produto: sem adivinhar
em silêncio, permissão de execução individual, saída honesta.

## 2. Superfície do comando

```bash
devforge run [caminho] [flags] [-- args...]   # args vão pro comando de start
  --dry-run          mostra o comando resolvido, sem executar
  --timeout <secs>   quanto tempo esperar "pronto" (padrão: 60)
  --health <spec>    override do readiness: "tcp:3000" | "http://localhost:8000/health"
  --json             plano/resultado estruturado (contrato schema:1)
```

- **v1 sem `--detach`** (fica pra v1.1): processo gerenciado em foreground,
  Ctrl-C limpo. Detach/PID mgmt adiciona superfície; não agora.

## 3. Resolução do comando de start (escada de 3 camadas)

Ordem estrita — **declarado > convenção > perguntar**. Nunca adivinhar.

### Camada A — declarado (confiável, o projeto diz como roda)
| Ecosistema | Fonte | Comando |
|---|---|---|
| JS/TS | `package.json` `scripts.start` | `npm run start` (pnpm/yarn se detectado) |
| Qualquer | `Procfile` `web:` | o comando do `web` (sem `web`: 1º processo; >1: perguntar) |
| Qualquer | `Makefile` alvos `run`/`start`/`serve` | `make <alvo>` (nesta ordem) |
| Qualquer | `docker-compose.yml` | `docker compose up` |

### Camada B — convenção por linguagem (quase confiável)
| Linguagem | Condição | Comando |
|---|---|---|
| Python | `manage.py` existe | `python3 manage.py runserver` (Django) |
| Python | `main.py` existe (sem manage.py) | `python3 main.py` |
| Ruby | `bin/rails` existe | `bin/rails server` |
| Ruby | `config.ru` existe | `rackup` |
| Elixir | `phoenix` nas deps do `mix.exs` | `mix phx.server` |
| Rust | target bin único (vários: perguntar) | `cargo run` |
| Go | main package na raiz | `go run .` |
| Go | único `cmd/*/main.go` | `go run ./cmd/<nome>` |
| .NET | projeto runável único | `dotnet run` |
| Swift | executável único | `swift run` |
| PHP | `artisan` existe (Laravel) | `php artisan serve` |
| Java | plugin `spring-boot` (Maven/Gradle) | `spring-boot:run` / `bootRun` |
| Dart | `bin/main.dart` | `dart run` |

### Camada C — ambíguo ou ausente → **perguntar**
- Nada encontrado, ou **vários pontos de entrada** (monorepo, N bins Go/Rust,
  vários `scripts`): listar candidatos e perguntar. Interativo sem TTY / modo
  IA (`--json`): **entregar as opções no JSON e NÃO executar** (nega por
  padrão, como sempre).
- C/C++: listar binários detectados do build (nunca chutar nome).

## 4. Permissão

- `run` executa **código do projeto** → risco **crítico**: confirmação
  **individual, sempre humana**; `--yes`/auto-approve **não cobre** (regra
  fixa do produto, protege o caso de IA dirigir).
- O prompt mostra o comando EXATO resolvido + o caminho.
- Modo IA (`--json` sem interação): entrega `{command, args, cwd}` como plano,
  não executa.

## 5. Ciclo de vida do processo (o miolo)

1. **Spawn** via `CommandRunner` (sem shell; argv explícito; sem `eval`).
2. **Classificação de comportamento:** se o processo sai rápido → one-shot
   (reporta exit code + saída). Se continua vivo após ~2s → tratado como
   **servidor**.
3. **Readiness (honesto, heurística em cascata):**
   a. `--health` explícito, se dado;
   b. **porta candidata** (PORT do ambiente/comando, portas comuns 3000/8000/
      8080/5173/4000) → poll TCP no loopback;
   c. Linux: achar as portas **reais** lendo os sockets do filho
      (`/proc/<pid>/net/tcp`); macOS: heurística por log;
   d. fallback: vivo sem crash após N s → "assumindo pronto" (**dito
      explicitamente na saída**, nunca vendido como certeza).
4. **Crash antes do ready** → últimas linhas do log → **`diagnose`** → causa
   + conserto (reusa 2.4 inteiro).
5. **Timeout** → derruba o processo e relata honestamente ("não subiu em Xs;
   últimas linhas: ...").
6. **Parada limpa:** SIGTERM → espera 5s → SIGKILL. Ctrl-C encaminha o
   sinal pro filho (process group). v1 POSIX (Linux/macOS); Windows: básico.

## 6. Saídas

- **Humana:** comando resolvido + "de onde veio" (ex.: `package.json
  scripts.start`), sinal de prontidão (porta), como parar, log em stream.
- **`--json` (schema:1):**
```json
{"schema":1,"command":"npm run start","resolved_from":"package.json scripts.start",
 "status":"running|exited|failed|timeout","exit_code":null,"port":3000,"duration_ms":1234}
```

## 7. Composição (`up`) — 5.0

`devforge up` = `clone? → audit → prepare → run`, orquestrado pelo
`WorkflowEngine`; `--until/--skip/--only`; cada etapa com **seu próprio gate**
(`run` continua perguntando mesmo dentro do `up`).

## 8. Estratégia de teste

- **Unit:** escada de resolução por fixture de projeto fake por ecossistema
  (incluindo casos de ambiguidade → "pergunta").
- **Integração (loopback, sem rede externa):** servidor Node mínimo que
  sobe porta → ready detectado; binário Rust que escuta; caso crash →
  diagnose; caso timeout.
- Regressão anti-FP: projeto sem entrypoint claro NÃO roda (só lista opções).

## 9. Limitações honestas (v1)

- Readiness é **heurística** (porta/log/vivo) — não é garantia universal.
- Monorepo com N entrypoints → pergunta; nunca escolhe sozinho.
- Fora de escopo v1: Flutter/mobile, JVM sem Spring, microserviços, detach,
  Windows robusto (sinais).
- É um runner de **dev**, não substituto de pm2/systemd em produção.

## 10. Staging

- **run v1:** escada + gates + readiness heurístico + diagnose no crash +
  `--json` + foreground. ✅ **feito.**
- **v1.1:** `--detach` + PID file; `--health http` com https.
- **up:** compor depois que `run` v1 estiver estável.

## 11. Descobertas da implementação (vale registrar)

- **`npm run start` é neto**: o `node` que ouve a porta é descendente do npm.
  Solução: o filho roda em **process group própria** e a parada usa
  `killpg` (pega netos); a descoberta de porta caminha `/proc` por
  descendentes (`task/<pid>/children`). Sem isso: neto sobrevivia e segurava o
  pipe de stdout pra sempre (a CLI travava no join).
- **Leitura de log com deadline**: os leitores de stdout/stderr são aguardados
  com `recv_timeout` — um descendente órfão não pode segurar a CLI.
- **macOS**: sem `/proc`, a descoberta de porta real fica indisponível (usa
  `--health` ou candidatas). Documentado como limitação v1.
