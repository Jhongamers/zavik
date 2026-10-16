# ADR 0002: Barramento de Eventos (Event Bus)

## Status
Aprovado

## Contexto
O DevForge precisa suportar múltiplos canais de apresentação (CLI, GUI via Tauri, futura extensão de VSCode) compartilhando a mesma lógica de negócio, sem que o Core precise conhecer cada interface gráfica ou terminal.

## Decisão
Implementamos um **Event Bus** assíncrono interno baseado em eventos de domínio puro no Kernel. A comunicação entre os módulos core e as camadas de apresentação ocorre exclusivamente por meio do envio e assinatura de eventos assíncronos. Nenhum módulo do Core chama métodos ou instancia dependências das camadas de apresentação.

## Consequências
*   **Positivas**:
    *   Desacoplamento temporal e estrutural total.
    *   Fácil adição de novas interfaces de usuário (ex: VSCode LSP) apenas assinando os mesmos eventos de domínio.
    *   Facilidade para gravar logs de auditoria e auditorias de segurança globais apenas escutando o barramento.
*   **Negativas**:
    *   Rastreabilidade de chamadas mais complexa no código (indireção via eventos em vez de chamadas diretas de função).

## Alternativas Descartadas
*   **Chamadas Diretas Síncronas (Orquestração Direta)**: Descartada pois causaria acoplamento forte entre o fluxo de execução e a UI do Tauri, tornando inviável uma CLI funcional e independente.
