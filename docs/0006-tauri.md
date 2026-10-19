# ADR 0006: Utilização do Tauri v2

## Status
Aprovado

## Contexto
O Zavik é um agente Desktop e requer uma interface gráfica amigável, nativa e leve para o desenvolvedor, sem consumir gigabytes de memória RAM na máquina do usuário.

## Decisão
Adotamos o **Tauri v2** para empacotar a aplicação desktop. A lógica nativa roda em Rust (nosso Kernel) e a interface do usuário é construída com React, Vite e TypeScript, se comunicando através de Tauri IPC (Commands e Event Listeners).

## Consequências
*   **Positivas**:
    *   Tamanho de binário final extremamente reduzido (geralmente < 20MB).
    *   Consumo de memória RAM mínimo (usa o webview nativo do sistema operacional).
    *   Excelente integração nativa com Rust, facilitando a amarração de commands.
*   **Negativas**:
    *   Dependência de webviews instalados no SO do usuário (Webview2 no Windows, WebKit no Linux/macOS).

## Alternativas Descartadas
*   **Electron**: Descartado por gerar binários muito pesados (> 100MB) e alto consumo de memória RAM (> 200MB ocioso).
*   **Rust GUI Nativa (ex: Druid/egui)**: Descartada pelo ecossistema ainda imaturo e complexidade de criar layouts refinados de workflow em comparação com o ecossistema React.
