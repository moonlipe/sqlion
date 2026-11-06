## ADDED Requirements

### Requirement: Liberação de memória de consultas anteriores
Ao fechar uma aba, substituir os resultados por uma nova execução ou fechar um projeto, o sistema SHALL liberar os dados dos resultados anteriores (linhas, estado das linhas, desfazer, texto do painel de valor), e o uso de memória do processo SHALL voltar a um patamar próximo do inicial, sem crescer a cada nova consulta.

#### Scenario: Fechar a aba de script
- **WHEN** o usuário executa uma consulta grande e fecha a aba
- **THEN** os resultados são descartados e a memória do processo cai de volta, em vez de permanecer no pico

#### Scenario: Várias consultas em sequência
- **WHEN** o usuário executa 20 consultas grandes seguidas na mesma aba, cada uma substituindo a anterior
- **THEN** a memória estabiliza em torno do tamanho de um resultado e não cresce de forma cumulativa

#### Scenario: Resultado antigo ao recarregar tabela
- **WHEN** o usuário recarrega os dados de uma tabela
- **THEN** a grade anterior é descartada antes de a nova ser mantida
