# Plano de Lançamento & Monetização — Zavik

> Registro das decisões de produto/lançamento (out/2026). Status: **plano** —
> nada aqui é implementado sem decisão explícita. A base técnica que já existe
> está no `ROADMAP.md`; este arquivo guarda o lado de produto, cobrança e
> lançamento.

## 1. Modelo escolhido

**Open core + assinatura, offline-first.** O CLI é grátis e de código aberto
(cunha de adoção + confiança — ferramenta de segurança fechada gera desconfiança
e é reescrita por IA de qualquer forma). O que se vende é **dado fresco e
serviço**, que a IA não reescreve e que tem custo recorrente pra nós.

## 2. O que é gratuito e o que é pago

### Gratuito pra sempre (código aberto)
- `detect` / `analyze` / `context` / `diagnose`
- `prepare` / `docker` (com o gate de permissões)
- `audit` **offline** + `--fail-on` + `--policy` / `--ignore` + `--sarif`
- `license status|install|remove`
- `--json` em tudo (interface pra IA)

### Pago (assinatura)
- **Fase 1 — dataset de risco:** atualizações assinadas de GHSA, typosquat e
  sinais de exposição, desbloqueadas pela licença + prioridade em correções.
- **Fase 2 — serviço:** gate server-side no PR (incontornável), dashboard
  multi-repo, política por organização, trilha de auditoria (evidência de
  compliance), SSO, espelho on-prem de advisory.

## 3. Fases de monetização (custo $0 primeiro)

| Fase | Entrega | Infra |
|---|---|---|
| 0 — produto | Fechar os dados que valem dinheiro: GHSA online, integridade de lockfile, typosquat (ROADMAP 1.4). Hooks de outros ecossistemas (1.x) ficam **grátis** como cunha. | $0 |
| 1 — receita | Licença offline Ed25519 (**feito**) + dataset assinado via GitHub Releases/repo privado + Action no GitHub Marketplace. Cobrança via MoR. | $0 |
| 2 — plataforma | Gate server-side (GitHub App), dashboard, trilha de auditoria, SSO — paga **com** a receita da fase 1. | paga |

### Fluxo de cobrança (fase 1 — como o dinheiro vira chave)

```
cliente paga no checkout (MoR: Paddle/Lemon Squeezy global, ou Kiwify/PIX Brasil)
   ↓
webhook do MoR (ou entrega manual no começo)
   ↓
vendor emite a chave:
   ZV_SIGNING_KEY=$(cat ~/.zavik-signing.key) \
   license_tool mint --plan team --holder "Acme" --expires 2027-10-15
   ↓
chave entregue por e-mail / link do checkout
   ↓
cliente:  zavik license install ZV1....
   ↓
dataset:  zavik data update   ← só baixa com licença válida
```

- **Renovação = expiry da chave** (12 meses): expirou → `data update` recusa →
  staleness honesta na tela. Recorrência sem servidor de billing.
- **Cancelamento = dupla trava:** você não emite a próxima chave E revoga o
  token de acesso ao repo do dataset (por cliente, revogável).
- Meios: fase 1 = MoR (sem custo fixo, só %). Stripe só na fase 2 (backend).
- **Preços são pesquisa de mercado fora daqui** (comparar com Socket/Snyk e
  ferramentas indie; validar com 10 conversas com times de segurança).

## 4. Kit de lançamento — Product Hunt

### Posicionamento
- Ângulo duplo (o mais quente hoje): **"CLI que dá mãos pros agentes de IA de
  código + auditoria de supply-chain que gateia seu PR"**.
- Slogan candidato (EN): *"Prep any repo to run. Audit it before it runs."*
- Badge "Open source" é ativo de conversão no PH; repo público com GIF bom.

### Copy pronta (colar no PH — EN, pronta pro founder revisar)

**Name:** Zavik

**Tagline (≤60 chars — candidatas medidas; recomendada a nº 1, o slogan
registrado):**

1. `Prep any repo to run. Audit it before it runs.` (46)
2. `Audit your supply chain, brief your AI agent.` (45) — diz o que é
3. `One CLI: supply-chain audit + AI agent briefing` (47) — explícita

**Description (campo do produto):**

> Zavik is an open-source dev CLI & AI-orchestration engine. One call
> detects the stack and hands any AI coding agent a complete briefing —
> runbook, entrypoints, trust (`zavik context --json`). Another audits
> your dependency supply-chain and gates PRs through a GitHub Action
> with SARIF output.
>
> It's a plain CLI: stable `--json` and exit codes *are* the interface —
> works with Claude Code, Codex, Cursor, or no agent at all. Free
> forever, MIT core; the paid add-on is a daily signed risk dataset,
> never locked features.

**Primeiro comentário do maker (EN)** — story curta + limitações
honestas, como registrado:

> Hi! I built Zavik because two things bugged me: every AI coding agent
> re-learns the same repo from scratch (burning tokens guessing the
> runbook), and supply-chain audit tools are mostly heavy, enterprise,
> SaaS-shaped. Zavik is the opposite: one plain CLI, offline-first, that
> prepares any repo for humans *and* agents — and audits it before it
> runs.
>
> Honest limitations (yes, on launch day):
>
> - Zavik **proposes** fixes; it never applies them automatically —
>   execution is out of scope by design.
> - It's an **anti-dishonesty gate**, not an anti-attacker guarantee. We
>   don't claim more than the tool does.
> - Online enrichment needs the network; offline audits are more
>   conservative.
> - Human messages ship in pt-BR + English (default: English); the
>   machine contract (JSON, SARIF, exit codes) is always English.
> - The CLI is free forever (MIT). A daily signed risk dataset is the
>   paid add-on — price lands after launch, researched, not guessed.
>
> What would you pay for the dataset? Roast the docs, the GIF, or the
> whole thing — feedback *is* the launch. 🙌

**Tópicos sugeridos** (conferir a lista atual do PH na hora): Developer
Tools, Open Source, Artificial Intelligence, Productivity.

**Galeria:** GIF `detect → audit (achado) → anotação no PR` (topo do
README + slot do site — **pendência do founder**); 4–6 screenshots de
terminal (`context --json`, `audit`, `up`, action no PR); vídeo de 60s
opcional.

**Links:** site `https://jhongamers.github.io/zavik/` · repo
`https://github.com/Jhongamers/zavik`

### Pré-lançamento (checklist)
- [x] Arquivo `LICENSE` (MIT) na raiz ✅ (holder "Zavik contributors";
      nome legal do founder é opcional, não bloqueia).
- [ ] README com **GIF no topo**: `detect` → `audit` achando algo → anotação
      no PR (SARIF). Ferramenta de dev vende no GIF. **(pendência do
      founder — placeholder pronto no site e no README)**
- [x] `--version` e instalação documentados ✅ (install.sh + READMEs;
      install anônimo E2E verde).
- [x] Tag publicada ✅ — `v0.1.3` (20/10) com release 4/4 + E2E completo
      (binário, install.sh anônimo, action-smoke verde).
- [ ] 4-6 screenshots/GIFs + vídeo de 60s. **(founder)**

### No lançamento
- [ ] Terça–quinta, manhã EUA. Nunca sexta.
- [ ] Primeiro comentário = founder story curta + **limitações honestas**
      (é a marca: pouquíssima gente admite limite em security tool).
- [ ] Responder TODO comentário nas primeiras 12h.
- [ ] Não esconder o free tier: CLI generosa é o funil; monetiza quando o
      time inteiro já vive nela.

### Limitações do plano (honestidade)
- Conta do PH, assets e timing são do lado do founder (a IA não publica).
- A validação real de preço vem depois do primeiro contato com compradores.

## 5. Ideias em análise (NÃO implementadas — aguardando decisão)

### 5.0 Princípio de UX decidido: fluxo isolado E composto (decisão do founder)
Toda etapa deve servir **sozinha** (1 de cada, ou só 2) e também **compor no
fluxo completo** — "os 3 juntos":

- **Isolado:** `detect` / `audit` / `prepare` / `run` continuam comandos
  independentes — é o modo **IA** (o agente chama só o passo que precisa,
  com `--json`, economizando token) e o modo script/CI.
- **Composto:** um comando sugar tipo `zavik up <url|caminho>` roda o
  pipeline `clone? → audit → prepare → run`, orquestrado pelo
  **`WorkflowEngine` que já existe no kernel** (não é máquina nova).
- **Controle parcial no composto:** `--until <etapa>`, `--skip <etapa>`,
  `--only <etapa>`, `--dry-run` — atende "só 1", "só 2", "os 3".
- **Segurança não dilui:** cada etapa do composto passa pelo seu gate de
  permissão individual; o `up` nunca engole confirmações (`--yes` continua
  não cobrindo execução de código).

### 5.1 Execução pós-instalação (`zavik run`) — v1 IMPLEMENTADO ✅
Ideia do founder: após `prepare` dar certo (ex.: `npm install` sem erro), a CLI
dispara o comando de start do projeto — **em qualquer linguagem**.
**→ v1 em `zavik run`** (desenho: `docs/design-run.md`). Pendências: v1.1
(`--detach`), health https, macOS com descoberta de porta.

Análise honesta:
- **Captura de erro já é resolvida:** o `CommandRunner` roda sem shell (argv,
  imune a injeção) e captura exit code + stdout/stderr; o `diagnose` já consome
  texto de erro e devolve causa + conserto. O "ela saber expressar a saída" é
  exatamente o que 2.4 já faz.
- **O desafio real não é o erro — é o processo que NÃO termina.** Servidor web
  (`npm start`, `cargo run`, `python manage.py runserver`) fica rodando; "deu
  certo" não é exit code 0, é **porta respondendo / log sem exceção**. Requer:
  rodar em background, health-check (porta/URL), captura de log, timeout e
  parada limpa. É engenharia de processo, não de diagnóstico.
- **Descobrir o "start" por ecossistema:** `package.json` `scripts.start`,
  Procfile, `docker-compose up`, `make run/start`, `cargo run`, `go run .`,
  `python manage.py runserver`… Começar curado (npm, compose, Makefile, cargo)
  e ser honesto no que não suporta — igual fizemos no prepare.
- **Permissão:** rodar código do projeto é ação sensível → mesmo gate de
  confirmação de sempre.
- Veredito: **faz sentido** (fecha o ciclo "clone → audit → prepare → run" e
  reforça a tese "mãos pra IA"), mas é o item de maior complexidade técnica da
  lista — merece desenho antes.

### 5.2 `git clone` integrado
Ideia do founder: a CLI cuidar também do clone do repositório.

Análise honesta:
- **Faz sentido como porta de entrada**: `zavik from <url>` → clone
  (--depth 1) → `detect` → `context`/`audit` → `prepare` → (5.1) `run`. Vira
  "cole o link e receba o runbook" — ótima cunha demo-ável no PH.
- **Riscos a tratar (não ignorar):** repositório malicioso pode trazer
  **git hooks executáveis**, submodules/LFS apontando pra fora, e scripts nos
  manifestos (já tratamos com trust check). Medidas: clonar **sem hooks**
  (`core.hooksPath` vazio/template limpo), `--depth 1`, e o check de confiança
  antes de QUALQUER execução — a cultura de permissão que já existe.
- Veredito: **faz sentido**, baixo risco se respeitar os pontos acima; bom
  candidato a virar desenho junto com 5.1.

### Ordem sugerida (quando decidir implementar)
1. Desenho conjunto de `from <url>` (clone seguro) — menor, destrava demo.
2. Desenho de `run` (processo longo + health-check) — maior, fechar o ciclo.
