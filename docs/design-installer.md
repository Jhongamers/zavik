# Design — instalador (`install.sh`)

Status: **implementado (26/10)** — autodetect da última release no mesmo lote
do gate de traces, com testes. Pedido do fundador (reduzir o trabalho de
release: o bump de 7 arquivos incluía o pin do `install.sh` a cada tag).

## 1. O que o instalador é (e o que NÃO é)

Única porta do projeto que toca rede: detecta OS/arch, baixa o tarball da
release, **verifica sha256** (sem flag de pular) e instala em
`~/.local/bin`. POSIX sh, sem dependências (nem `jq`). O próprio `audit` do
zavik classifica `curl | sh` como risco — por isso o README pede para baixar,
ler e então rodar.

## 2. Resolução de versão (novo em 26/10)

Ordem, com o primeiro hit vencendo:

1. `ZAVIK_VERSION` definido → usa a tag literal (pin explícito: testes,
   espelhos, CI, "quero exatamente vX").
2. Senão → **autodetect**: `GET https://api.github.com/repos/Jhongamers/zavik/releases/latest`,
   parse do `tag_name` via `sed` (o instalador continua sem `jq`), com
   timeout de 10s. `ZAVIK_LATEST_URL` sobrepõe o endpoint (testes servem o
   JSON fake via `file://`).
3. API fora do ar (offline, air-gapped, rate limit do GitHub) →
   **fallback honesto**: instala `FALLBACK_VERSION` (pin embutido no script)
   com aviso no stderr dizendo qual tag e como pinar.

Consequência: o caminho normal instala **sempre a última release** sem editar
`install.sh`; o pin embutido virou só rede de segurança, e é obrigatório
bumpá-lo a cada release (uma linha), mas esquecer o bump degrada para "versão
antiga com aviso", nunca para "instalação quebrada".

Por que API e não redirect de `releases/latest`: o redirect resolveria sem
rate limit, mas não é testável offline (`file://` não redireciona); a API tem
60 req/h sem token, suficiente para um instalador, e o fallback cobre o
estouro.

## 3. Invariantes

- sha256 conferido sempre; **não existe flag para pular** a verificação.
- Arquiteturas fora de x86_64/aarch64 e SOs fora de linux/darwin → erro
  claro antes de qualquer rede (o teste de SO finge `uname` SunOS).
- Nada é instalado quando o checksum diverge.
- `--json`/contrato de máquina não é afetado: o instalador só publica binário.

## 4. Cobertura (`crates/cli/tests/installer.rs`)

Releia a release fake (tarball + sha256) via `file://`:

- sintaxe POSIX (`sh -n`);
- caminho feliz instala binário que roda;
- tarball adulterado é recusado e nada é instalado;
- SO desconhecido recebe mensagem clara;
- **autodetect**: JSON da API fake (`ZAVIK_LATEST_URL=file://…`) → instala a
   tag do JSON, sem warning;
- **fallback**: endpoint inalcansável → warning com a tag pinada lida do
   próprio script (bump de release não quebra o teste) e instalação segue.

## 5. Fora de escopo (depois)

- Assinatura (minisign/cosign) além do sha256 — sha256 protege contra
  corrupção, não contra comprometimento da própria release.
- Windows no script (hoje: `.zip` manual na página de Releases).
- Homebrew/scoop/nix — quando o volume de instalações justificar.
