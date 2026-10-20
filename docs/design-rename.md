# Design: Rename de marca (devforge → zavik) e decisão de namespace

**Status:** Lote A/B em execução (2026-10-19) · Decisão final do usuário ("manda ve")

## Contexto — por que sair de `devforge`

Verificação empírica multi-canal mostrou o nome `devforge` ocupado em **todas** as
superfícies críticas do nosso espaço de categoria:

| Superfície | Evidência |
|---|---|
| GitHub | orgs `devforge`, `devforge-io`, `devforge-labs`, `devforge-dev`, `devforge-org`, `devforgehq` — todas ocupadas |
| npm | pacote `devforge` existe — "development workflow automation tool" |
| crates.io | crate `devforge` existe — `xtask, orchestrator` (fev/2026) |
| Web | `devforge.io` = Devforge Pty Ltd, com tool grátis de Website Audit |

Colisão de categoria em todas as superfícies → confusão de marca, SEO diluído e
distribuição quebrada (`cargo install`/npm impossíveis).

## Sagas do Lote N (naming) — lições registradas

1. **`avouch` escolhido e perdido** — dicionário confirmava o sentido exato
   ("to confirm or verify"; parente de *vouch*, doublet de *advocate*), mas durante a
   janela de decisão ficou ocupado em GitHub+npm (snipe ou negativo envenenado — não
   distinuível daqui).
2. **Rede local serve 404 fantasma** — o egresso desta máquina devolveu "livre" para
   nomes ocupados repetidamente (`gage`, `smelt`, a lista "CORE-LIVRE" inteira, depois
   `zavik`). **Sinais positivos são confiáveis; negativos não.** Oráculo final de
   disponibilidade = o formulário de criação de org no navegador do usuário (canal limpo)
   + triagem de positivos daqui.
3. **Handles curtos no GitHub estão minerados** — toda coinagem de4-5 letras testada
   já é conta de alguém (usuário ou org), e conta bloqueia org homônima.
4. **Sobrevivente final: `zavik`** — inventado, 5 letras, `za-VIK` (fácil em PT-BR),
   zero semântica para colidir, marca se cria (precedente: Vite, Deno, Bun).

## Decisão final de namespace — SEM ORG

O slot "org = marca" (`github.com/zavik`) morreu com a conta tomada; sem ele, a org
perdeu todo o valor e o usuário decidiu manter **tudo na conta pessoal**:

| Motivação de org | Equivalente na conta pessoal |
|---|---|
| Colaboradores sem expor outros repos | invite **por-repo** (Settings → Collaborators) |
| Secrets do dataset | já são repo-level (`DF_DATASET_KEY` existente) |
| Credibilidade PH | padrão-ouro do CLI Rust é conta pessoal (`BurntSushi/ripgrep`, `sharkdp/fd`) |
| Site Pages | `jhongamers.github.io/zavik` — igual, $0 |
| Namespace de marca | morto (conta `zavik` alheia) |

**Resultado:** repo renomeado **in place**: `Jhongamers/devforge` → `Jhongamers/zavik`
(rename, sem transfer; GitHub redireciona a URL antiga). Org fica adiada até haver motivo
real (time/receita) — criação futura + transfer com redirect = custo zero.

Regra de nomenclatura (três caixas, convenção de marca de tech):

| Forma | Onde vive |
|---|---|
| `Zavik` | produto no texto/prósa |
| `zavik` | binário, crate, npm, URLs, comandos |
| `ZAVIK` | banner do CLI (`✨ ZAVIK — audit ✨`) e `ZAVIK_LANG` |

Prefixos alinhados (nada publicado — custo zero): `ZV_SIGNING_KEY`, `ZV_DATASET_KEY`,
token de licença `ZV1.…`.

## Escopo dos lotes

- **Lote A — código:** crates (`devforge-*` → `zavik-*`), pacote/binário, banner e
  catálogo i18n (chave=literal preservada: chave e valor mudam juntos), `HELP_*`, env
  (`DEVFORGE_LANG` → `ZAVIK_LANG`, `DF_` → `ZV_`), licença (`DF1` → `ZV1`), testes,
  workflows, docs, READMEs (menções de produto).
- **Lote B — repo/URLs:** rename via API do repositório, sweep `Jhongamers/devforge` →
  `Jhongamers/zavik` (install.sh `REPO`, badges, clones, `uses:`), secret do GitHub
  `DF_DATASET_KEY` → `ZV_DATASET_KEY` (valor vem do arquivo local `~/.devforge-dataset.key`),
  reconferir os3 workflows.
- **Lote C — site:** `jhongamers.github.io/zavik` (hero + GIF + install + limitações).

## Proteções do sweep (Lote A NÃO toca)

- `Jhongamers/devforge` — URL válida até o rename do repo (sweep é do Lote B).
- `devforge-corpus` e o diretório de trabalho local — paths que existem de
  verdade: o diretório do repo foi renomeado no Lote G
  (`~/devforge` → `~/zavik`, no diretório home do fundador);
  `devforge-corpus` mantém
  o nome legado (cache local de dados, fora do git).
- Este arquivo (`docs/design-rename.md`) — ADR histórico, excluído do varrimento.

## Invariantes preservados

- i18n chave = literal pt emitido: chaves e valores renomeados **juntos** → regra intacta.
- Classe conteúdo (evidence/finding/risk) segue EN; machine output não traduz.
- Escopo do rename é **cosmético de nome**: nenhum comportamento, formato ou regra muda.

## Log de status

- 2026-10-19: decisão fechada — produto `zavik`, sem org, prefixos `ZV_`/`ZV1.`.
- 2026-10-19: **Lote A concluído** —71 arquivos renomeados; residual zero fora das
  proteções;200 testes / clippy0 / fmt (só2 falsos positivos de header em drift
  pré-existente: main.rs:1927,2424); smoke OK (banner `✨ ZAVIK — audit ✨`, help pt,
  erros EN/pt com binário `zavik`).
- 2026-10-19: **Lote B concluído** — repo `Jhongamers/devforge` → `Jhongamers/zavik`
  (redirect API confirmado por sinal positivo; curl local continua servindo 404
  fantasma), secret `ZV_DATASET_KEY` (valor do arquivo local; antigo removido),
  sweep de URLs em install.sh +2 READMEs, CI green (`963725d`) e **dataset workflow
  disparado com sucesso** → release `dataset-latest/dataset.json` prova o secret E2E.
- 2026-10-19: **Lote G concluído** — diretório local de trabalho renomeado
  `~/devforge` → `~/zavik` (rename cosmético pós-release:
  nenhum path absoluto do repo apontava para o diretório — grep verificado —;
  sessão de trabalho movida junto; `devforge-corpus` mantém nome legado).
- 2026-10-19: **Lote C (scaffold) concluído** — site estático (`index.html`
  autocontido + `.nojekyll` + README de publicação) na branch **`gh-pages`
  órfã** do próprio repo (`2c35e48`). Endereço verificado, não assumido:
  Pages de project-site serve `<owner>.github.io/<repo>` literalmente (doc
  oficial + caso-controle `RayeRen/acad-homepage.github.io` → 200 com o
  sufixo, 404 sem) — um repo `zavik.github.io` publicaria em
  `.../zavik.github.io/` e **divergiria** do ADR; logo o site vive na branch,
  não em repo novo. Ativação no dia de abrir o repo (Pages free exige repo
  público). Faltam pro founder: GIF demo (placeholder pronto) e nome legal
  da LICENSE no footer.
