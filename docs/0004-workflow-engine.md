# ADR 0004: Workflow Engine Baseada em Steps

## Status
Aprovado

## Contexto
Diferente de scripts simples, a automação de correção de ambientes pode falhar em estágios intermediários (ex: baixar pacote com sucesso, mas falhar ao compilar). O Zavik precisa de resiliência para não deixar o ambiente do desenvolvedor quebrado ou inconsistente.

## Decisão
Implementamos uma **Workflow Engine** estruturada no modelo hierárquico **Workflow -> Step -> Action**. Cada **Step** gerencia uma etapa lógica do fluxo de automação e deve conter:
- **Entrada (Input)**: Parâmetros e dados de contexto necessários para a execução.
- **Processamento (Processing)**: A lógica operacional que executa uma ou mais **Actions** (comandos, escritas de arquivo, etc.).
- **Saída (Output)**: Dados resultantes produzidos pelo processamento.
- **Estado (State)**: A situação atual da máquina de estados do passo (ex: Pending, Running, Completed, Failed, RolledBack).
- **Erro (Error)**: Informações estruturadas detalhadas em caso de falha.

Se um Step falha, a engine interrompe o workflow e expõe o erro estruturado, permitindo rollback, pausa ou repetição.

## Consequências
*   **Positivas**:
    *   Execução segura, transparente e transacional.
    *   Possibilidade de pausar, retomar ou editar passos antes da execução.
    *   Redução drástica do risco de corrupção do ambiente do usuário.
*   **Negativas**:
    *   Aumento da complexidade da engine de execução em Rust (gerenciamento de estados e concorrência).

## Alternativas Descartadas
*   **Shell Script Linear Simples**: Descartado por não possuir tolerância a falhas, nem permitir o usuário escolher rodar apenas partes do plano ou reverter alterações feitas.
