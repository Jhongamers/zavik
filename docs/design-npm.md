# Design — pacote npm (`npm/`)

Status: **implementado (26/10)** — no mesmo lote da preparação de publicação
do crates.io. Decisão registrada no ROADMAP ("npm publish apontando pra binário
da Release mais recente"); este doc fixa o desenho e o porquê.

## 1. O que o pacote é

`npm install -g zavik` instala um **shim JavaScript** (zero dependências
npm). O `npm install` **não baixa nem executa nada** — só copia arquivos. Na
**primeira execução** do `zavik`, o shim baixa o binário oficial da release
pro platform, **confere o sha256** contra o checksum publicado (mismatch é
recusado, sem flag de pular — mesma regra do `install.sh`) e cacheia em
`~/.cache/zavik/` (`XDG_CACHE_HOME` respeitado). Depois disso é idêntico a um
binário nativo: mesmos args, exit codes e sinais.

## 2. Por que sem install script (lazy) e não `postinstall`

Nosso próprio `audit` classifica install script como fator de risco
`install-script` — a classe de mecanismo. Um pacote npm do produto de
supply-chain **com** postinstall seria o próprio contra-exemplo. Com o
download na primeira execução:

- `npm install` é auditável: cópia de arquivos, zero rede, zero execução;
- o download roda **como o usuário** (não como root no momento da instalação
  global), que é exatamente o cenário que o aviso de install script existe
  pra cobrir;
- `npx zavik audit` continua funcionando (o shim resolve o binário sob
  demanda).

Custo aceito: a primeira execução leva alguns segundos a mais (download +
verificação). Registrado como trade-off consciente.

## 3. Matriz de plataforma (honesta)

Os assets publicados pelo `release.yml` são linux x64/arm64, macOS arm64 e
windows x64 (zip). O pacote npm v1 cobre **linux x64/arm64 + macOS arm64** e
falha com mensagem honesta no resto:

- macOS Intel: "sem binário — use `cargo install zavik`";
- Windows: "use o `.zip` das Releases";
- qualquer outro SO: link da seção de install.

Nada de silêncio nem fallback torto: o usuário descobre o motivo na primeira
execução, com o caminho certo na mensagem. Windows via npm (extração do zip
sem dependência externa — PowerShell `Expand-Archive`) e macOS Intel entram
junto com a porta F3 (binários per-platform).

## 4. Invariantes e testes

- **Versão**: o shim baixa a release da **sua própria versão**
  (`package.json`), então o teste `npm_package_version_matches_the_workspace_version`
  (em `crates/cli/tests/release_pins.rs`) mantém npm ↔ release em lockstep a
  cada bump — mismatch viraria 404 no primeiro run de todo usuário.
- **Testes do pacote** (`npm/test/ensure.test.js`, `node:test`, zero deps):
  matriz de plataforma (3 alvos + 3 recusas honestas), pin por
  `ZAVIK_VERSION`, caminho feliz E2E com release fake servida via HTTP local
  (instala, executa, cacheia sem nova requisição) e archive adulterado
  recusado sem instalar nada. Rodam no CI (job `npm-test`).
- Overrides alinhados ao `install.sh`: `ZAVIK_VERSION` (pin de tag) e
  `ZAVIK_BASE_URL` (espelhos/testes), mais `ZAVIK_CACHE_DIR`.

## 5. Publicação (founder) e bump de release

- Publicar: `cd npm && npm publish` (conta npm do fundador; nome `zavik`
  checado livre em 26/10 — rechecar no dia).
- Bump de release passa a ser os 8 pins: workspace `version` + `Cargo.lock` +
  deps internas do `Cargo.toml` (raiz) + `install.sh` FALLBACK + `action.yml`
  + `action-smoke` + `npm/package.json` + snippets dos READMEs. Os pins de
  código têm teste de coerção (action_yml.rs + release_pins.rs); só os
  snippets de README continuam manuais.
- A linha do portão npm nos READMEs entra no commit do primeiro publish
  (não antes: não anunciar porta que ainda 404).

## 6. Fora de escopo (depois)

- F3: binários per-platform via `optionalDependencies` (padrão esbuild/swc —
  `npm i` baixa só o binário da plataforma, sem shim), winget/scoop/choco.
- Auto-update do binário cacheado (o update-notice do ROADMAP §4 só PROPOE o
  comando — auto-update violaria "produto só propõe" e acrescenta superfície).
- Publicação automática do npm no `release.yml` (conta + token do fundador;
  automatizar é candidato no Paddle week).
