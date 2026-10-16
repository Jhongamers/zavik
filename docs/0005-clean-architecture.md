# ADR 0005: Adoção de Clean Architecture

## Status
Aprovado

## Contexto
O software precisa de alta testabilidade e deve sobreviver à evolução tecnológica sem reescrever regras de negócio centrais (ex: o que é um Workflow ou como funciona o Permission Manager).

## Decisão
Adotamos os princípios de **Clean Architecture**. Dividimos o projeto em:
1.  **Domain/Entities**: Conceitos fundamentais de negócio (`Workspace`, `Diagnostic`, `Workflow`, `Step`).
2.  **Use Cases**: Lógica de fluxo (motor do workflow, scanner).
3.  **Adapters/Presentation**: Pontes para bancos SQLite, Tauri e CLI.
4.  **Infrastructure**: OS Layer, Sandbox e frameworks de terceiros.
As dependências fluem estritamente de fora para dentro. O Domínio e Casos de Uso dependem apenas de Traits (contratos).

## Consequências
*   **Positivas**:
    *   Testabilidade exemplar por meio de injeção de dependências e mock de contratos.
    *   Flexibilidade total para alterar banco de dados, biblioteca gráfica ou sistema operacional.
*   **Negativas**:
    *   Maior quantidade de arquivos e boilerplate inicial no Rust devido à separação estrita de camadas.

## Alternativas Descartadas
*   **ActiveRecord / Framework-Driven Architecture**: Onde todo o código de regras de negócio fica acoplado diretamente a queries SQL ou recursos da API do Tauri. Descartado pelo acoplamento severo.
