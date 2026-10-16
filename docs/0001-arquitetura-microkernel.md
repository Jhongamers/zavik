# ADR 0001: Arquitetura Microkernel

## Status
Aprovado

## Contexto
O DevForge precisa atuar como um agente extensível, permitindo que novos comportamentos, suporte a linguagens de programação e regras de diagnóstico sejam adicionados continuamente sem inflar ou corromper o núcleo do sistema (*Core/Kernel*). Além disso, deve ser comercializável e suportar um marketplace de plugins no futuro.

## Decisão
Adotamos o padrão arquitetural de **Microkernel (Plug-and-Play)**. O núcleo do sistema é enxuto, responsável unicamente pelo ciclo de vida da aplicação (boot/shutdown dos serviços), e expõe um barramento central de comunicação. Toda a inteligência técnica de stacks (Node, Docker, Laravel) e integrações são tratadas como plugins acoplados dinamicamente (ou estaticamente na V1) ao Kernel.

## Consequências
*   **Positivas**:
    *   Kernel extremamente pequeno, facilitando testes e auditorias de segurança.
    *   Alta modularidade, permitindo substituir ou atualizar módulos de stack de forma independente.
    *   Facilidade para escalar o produto para um modelo de Marketplace.
*   **Negativas**:
    *   Necessidade de projetar uma API/SDK clara de comunicação para os plugins, aumentando o tempo inicial de design de contratos.

## Alternativas Descartadas
*   **Monolito Integrado**: Descartado por violar o princípio de responsabilidade única (SRP). O núcleo precisaria importar dezenas de crates e gerenciar regras de todas as linguagens, tornando o software pesado e difícil de manter.
