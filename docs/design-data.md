# Design — Camada paga fase 1: dataset de risco assinado (`data update`)

> Desenho (out/2026). Status: **aguardando "manda codar"**. Contexto: o modelo
> decidido é open core + assinatura (`ROADMAP` §3.3, `plano-lancamento.md`).
> Esta é a peça que **depende só de trabalho, não de caixa** ($0 de infra).

## 1. Objetivo

Tornar o produto **comprável** sem gastar nada: o CLI ganha um tier **Pro**
activado por licença (o sistema `ZV1`/Ed25519 já existe e está testado),
entregando **dados de risco frescos** — o moat que código nenhum (nem IA)
reproduz, porque exige trabalho contínuo nosso.

**O que NÃO é esta fase** (é fase 2, paga pela receita da fase 1):
`api.zavik` (gate server-side), dashboard, trilha de auditoria,
entrega automatizada de licença via webhook. Aqui: **GitHub Releases é o
"servidor"** (grátis) e a verificação é **local, no binário**.

## 2. O que o Pro entrega (escopo v1 do dataset)

Hoje o `audit` faz confluência de fatores do manifest/lockfile (+ `--online`
npm opcional). **Não faz** lookup de advisory. O dataset acrescenta, com
dados **atualizados diariamente pela CI**:

1. **Janela de advisories recentes** — `github/advisory-database`,
   **somente `advisories/github-reviewed`** (36.9k revisados pelo time do
   GitHub; os 353k `unreviewed` ficam de fora: sem curatoria + clone de
   3.1 GB), janela de **90 dias** → medido hoje: **4.065 advisories** em
   10 ecossistemas. *(Amendment 16/10: números medidos contra o repo real,
   não estimados.)*
2. **Kill list de typosquat/malicious** — **nomes já reportados como ruins**
   por ecossistema, match **exato** nas dependências. Extraída por
   **patterns precisos de título** (4 formas, medidas nos 389.683 arquivos:
   "`X` was removed from crates.io for malicious code", "Malicious code in
   `X`", "X is malware", "X typosquatting malware") → **61 nomes reais**
   (`crossenv`, `mongose`, `lodahs`, `electorn`...), todos os anos (takedown
   de 2019 continua ruim). **Exclui título version-scoped** ("...in
   guardrails-ai 0.10.1") — o pacote é legítimo, as versões que foram: kill
   list por nome FALSARIA pra quem está patchado (esses caem no advisory).
   *(Amendment: a primeira versão disse "nomes canônicos" — direção errada;
   e `/malicious/` cru bateria em "malicious inputs" de `jsonata`/`surrealdb`
   = blacklist de pacotes legítimos — o E2E em dados reais pegou isso.)*
   **Idioma (18/10)**: todo prose autorado no builder — o `reason` da kill
   list e rótulos do dataset — é **EN**: classe conteúdo, nunca traduzido
   em runtime (`docs/design-i18n.md` §1); o advisory natural do GHSA já
   vem em EN. Build do dataset assinado precisa honorar isso.
3. **Pacotes maliciosos/removidos** entram **na kill list** (item 2) — sinal
   mais forte; **não** são duplicados como advisory `low` do mesmo nome.
   Upgrade opcional (decisão do founder): corpus OSV
   `ossf/malicious-packages` — 238k nomes (222k só npm) ≈ asset de ~28 MB
   e clone de 240k arquivos na CI vs. asset de ~1 MB hoje; v1 mantém GHSA
   (precisão + tamanho), o OSV vira etapa seguinte se a cobertura pesar.

Formato: **`dataset.json` único** (schema versionado `"schema": 2`) —
payload medido hoje: **750 KB** (assinado ≈ 1 MB), compilado pela CI, não
um dump bruto.

### 2.1 Controle de falso positivo (regra da casa)

- **Advisory = exposição proposta, nunca confirmação**: correspondência por
  **nome de pacote** → achado `severity: low` com link pro advisory e texto
  "possível exposição — confira a versão". **Sem** range-matching por
  ecossistema no v1 (semântica de range varia por linguagem = fora de
  controle de blast radius; o produto só **propõe**, o humano/CI decide).
- **Kill list = match exato** de nomes conhecidos ruins (`severity: high`,
  é positivo quase certo). **Sem** fuzzy/edit-distance no v1 (gera FP em
  massa). Matching por (ecossistema, nome) com lowercase
  (+ normalização PyPI `-`/`_`/`.`).
- Achados do dataset entram como **seção separada** (`source: "dataset"`,
  JSON `dataset_findings`) e **não alteram score, `passed` nem `--fail-on`**
  — audit grátis e Pro continuam comparáveis e o CI de ninguém muda de
  resultado; é seção informativa (a decisão de agir é humana).
- **Um achado por pacote** (amendment 16/10, do E2E real): um npm teve 9
  GHSAs na mesma janela → 9 linhas idênticas é ruído; agrupa em 1 achado
  com "id + mais N advisories". Advisory por nome **nunca** vira kill list
  semântica (só os 4 patterns de título, que são name-level).

## 3. Arquitetura

```
CI (cron diário + dispatch manual)              CLI (offline-first)
┌──────────────────────────────────┐            ┌──────────────────────────────┐
│ dataset.yml                      │            │ zavik data update          │
│  1. clone github/advisory-db     │  Releases │   1. gate: licença ZV1 válida?│
│  2. condensar (90d, typosquat)   │ ─────────► │   2. baixa dataset.json       │
│  3. assinar (ZV_DATASET_KEY)     │  tag      │   3. verifica assinatura      │
│  4. upload p/ tag `dataset-latest│  (grátis) │      (pubkey embutida)        │
│     --clobber` (substitui asset) │            │   4. grava no XDG atômico     │
└──────────────────────────────────┘            │ zavik data status          │
        chave privada = GitHub Secret           │ audit usa se presente+OK      │
                                                └──────────────────────────────┘
```

## 4. Componentes

### 4.1 Assinatura do dataset (chave separada da licença)

- Keypair **próprio** `ZV_DATASET_KEY` (blast radius: vazou a chave do dataset
  ≠ conseguem mintar licenças; rotacionar dataset = release nova do CLI).
- Privada em **GitHub Secret** (só o workflow roda com ela); pública embutida
  no binário (`DATASET_PUBLIC_KEY`, ao lado da `PUBLIC_KEY_B64`).
- Ferramenta: subcomando do `examples/license_tool.rs` (novo `sign-dataset` /
  `verify-dataset`), reaproveitando o codec atual.

### 4.2 Comandos novos

- **`zavik data update [--json] [--url <url>]`** — baixa, verifica,
  instala. **Gate: exige licença instalada E válida** (sem → mensagem honesta
  de Pro + como comprar/instalar; `--json` → linha `status:"no_license"`,
  exit 1). URL: `--url` → env `ZV_DATASET_URL` → const default (**vazia até
  o repo GitHub existir — preencher no F0**; sem → erro honesto). Aceita
  `file://`/caminho local (testes offline + espelho airgap). Download via
  `ureq` (mesma família do `--online`), com limite de tamanho (32 MB).
- **`zavik data status [--json]`** — versão, data, nº de registros,
  idade; avisa staleness (>14 dias → "atrasado, rode data update").
  Sem dataset → mostra o **snapshot embutido** (tier grátis) e como passar
  pra Pro. `status` é sempre grátis (mostrar o degrau é a venda).

### 4.3 Integração com `audit`

- Se dataset local presente **e** assinado válido → achados `source:"dataset"`
  (seção separada, §2.1) + campos `dataset: {version, date, age_days}` em
  `--json` e linha humana "dados de <data>". Stale (>14d) → aviso honesto.
- Se ausente/inválido → comportamento **hoje byte a byte igual**. *(Amendment:
  não existe "snapshot embutido" — o tier grátis é o motor offline de hoje,
  sem dataset; a mensagem é "sem dataset fresco — Pro: `data update`",
  nunca fingir dado que não tem.)*
- Dataset adulterado → **recusa** (assim como token adulterado), avisa, cai
  pro grátis. Nunca usa dado sem assinatura.

### 4.4 Entrega da compra (v1 = manual, $0)

1. Cliente compra (MoR a definir — pesquisa de preço é tarefa do founder).
2. Founder minta o token (`license_tool mint`) e envia por e-mail.
3. Cliente: `zavik license install ZV1...` → `zavik data update`.
4. Automação (webhook MoR → emissão) = fase 2 (é servidor; é lá que
   `api.zavik` entra).

## 5. Fluxo por persona

| Persona | Hoje | Fase 1 |
|---|---|---|
| Grátis (offline puro) | CLI inteira + audit do manifest | igual — nada muda, nada é bloqueado |
| Pro (comprou) | — | `data update` → audit com achados frescos + aviso de staleness |
| Pirateiro de chave própria | pode forjar token | forja = só o que já é grátis; dataset é público → **fase 2 fecha de verdade** (análise registrada na regra: gate client-side é velocidade, não parede) |

## 6. Testes (obrigação)

- **Unit**: gate da licença (válida/ausente/expirada/adulterada), verificação
  de assinatura do dataset (ok/lixo/adulterado), cálculo de staleness,
  parse/schema versionado, casamento anti-typosquat (positivo + **FP**:
  nome parecido mas não-listado → NÃO sinaliza), exposição advisory
  (nome na janela → low com link; fora da janela → nada).
- **Sem override de pubkey via env** (decisão de segurança: um `ZV_*_PUBKEY`
  transformaria forjar em 2 env vars sem rebuild — pior que o patch que já
  aceitamos). Cobertura do caminho feliz: unitário com chaves de teste +
  **E2E manual com as chaves reais** (precedente do ciclo de licença).
- **Integração** (binário real, tudo offline via `ZV_DATASET_URL=file://`):
  `data status --json` sem dataset; `data update` sem licença → `no_license`
  exit 1; com licença + dataset assinado local → instala e `status` passa;
  dataset adulterado → recusa; `audit` com dataset → seção extra presente;
  sem dataset → saída idêntica à atual (byte a byte no `--json`).
- **E2E manual**: workflow `dataset.yml` com `workflow_dispatch` → asset na
  Release → `data update` num clone limpo → `audit` mostra "dados de hoje".
- **Invariante**: nenhum teste do `audit` grátis muda de resultado por causa
  da feature (difere só quando dataset local existe).

## 7. Limitações honestas (dizer na cara)

- Dataset **público** na Release: o gate da `data update` é cortesia
  (bloqueia o esquecido/emprestado, não o forjador) — **fase 2 fecha**.
- Sem range-matching por versão no v1 → exposição proposta, não confirmação.
- Cadência diária é o máximo do $0 (cron do GitHub Actions); janela 90d.
- `file://`/`--url` existe p/ teste e self-host; por padrão só a URL nossa.
- Sem telemetry, sem conta, offline-first preservado (update é opt-in online).

## 8. Staging (cada etapa com teste antes da próxima)

1. [x] Schema do dataset + signer/verifier no `license_tool` (+ unit).
   — `crates/cli/src/dataset.rs` (envelope, verificação, XDG, matching puro);
   `genkey-dataset`/`sign-dataset`/`verify-dataset` no tool; chave real gerada
   (privada fora do repo, pública embutida); 15 testes unitários.
2. [x] `data status` (sem rede, sem gate) (+ integração).
   — ausente / inválido / instalado com idade e staleness; `--json` com
   `tier:"free"` quando não há dataset.
3. [x] `data update` com gate de licença + `ZV_DATASET_URL` (+ integração).
   — gate **antes** da rede; `--url` → env → const (vazia até o F0);
   `file://`/caminho local; teto de 32 MB; verificação → escrita atômica.
4. [x] Workflow `dataset.yml` (cron diário 05:17 UTC + dispatch, assina e
   publica) **e** o builder `license_tool build-dataset` — 11 testes
   unitários (janela com borda exata, withdrawn, ecossistema não mapeado,
   os 4 patterns de kill list + os FPs negativos, version-scoped, merge do
   `--bad-list`, determinismo byte a byte). Validação real executada:
   builder sobre os **36.885** GHSAs reviewed → 4.065 advisories + 61 kill
   list, 0 parse errado, 750 KB; assinado → verificado → `data update
   file://` → audit com `lodahs` real (GHSA link) e advisory real agrupado.
   Só não RODA de verdade sem F0 (repo remoto + secret `ZV_DATASET_KEY` +
   preencher `DEFAULT_DATASET_URL`).
5. [x] Integração no `audit` (seção separada, invariante do tier grátis) (+ testes).
   — `dataset`/`dataset_findings` no JSON só quando há dataset válido;
   inválido = aviso no stderr e stdout limpo; seção humana antes do Veredito;
   invariante "sem dataset ⇒ sem chave" testada.
6. [x] Docs: README (tabela grátis vs Pro), ROADMAP.
   — preço/compra continuam na §9 (do founder).

**Cobertura do caminho feliz** (sem override de pubkey, §6): unit com chaves
de teste + **E2E manual real executado** — licença de produção cunhada via
`mint`, dataset assinado com a chave real, `update file://` → `status` →
`audit` com a seção (kill list `high` + advisory `low`) → dataset adulterado
**recusado** (exit 1) com o anterior permanecendo.

## 9. Decisões que precisam do founder

1. [x] **Aprovar o desenho** — go para codar dado em 16/10 (staging 1–3, 5, 6
   implementados; §2.1 ajustado em amended: kill list = nomes conhecidos
   RUINS, não canônicos; sem "snapshot embutido" — grátis = motor de hoje).
2. [ ] Cadência: diário (recomendado) ou semanal (menos ruído de staleness).
3. [ ] MoR + preço: pesquisa sua, depois — a entrega v1 é manual mesmo.
4. [ ] Nome do tier: "Pro" (usado aqui) ou outro.
