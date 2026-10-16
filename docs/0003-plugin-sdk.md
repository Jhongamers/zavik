# ADR 0003: Plugin SDK Independente

## Status
Aprovado

## Contexto
Para incentivar o ecossistema e garantir que desenvolvedores terceiros possam criar plugins para o DevForge sem expor o código do Kernel ou causar acoplamentos perigosos, precisamos de uma camada estável e isolada de definição de capacidades.

## Decisão
Criamos um sub-crate chamado `sdk` que hospeda apenas os tipos de dados e os contratos de **Capabilities** dos plugins. O SDK é independente do Kernel e de frameworks de terceiros. Os plugins dependem apenas do SDK. O Kernel, por sua vez, depende do SDK para instanciar e executar as capacidades dos plugins carregados.

## Consequências
*   **Positivas**:
    *   Plugins podem ser compilados separadamente.
    *   O SDK pode ser publicado de forma independente em repositórios (ex: crates.io ou npm/wasm).
    *   Mudanças internas de implementação no Kernel não quebram os plugins instalados, contanto que o SDK permaneça estável.
*   **Negativas**:
    *   Exige cuidado na evolução do SDK (mudanças de assinatura exigirão versionamento semântico estrito).

## Alternativas Descartadas
*   **Interface Direta no Kernel**: Descartada pois obrigaria qualquer plugin a importar todo o código-fonte ou dependências internas do Kernel para poder compilar.
