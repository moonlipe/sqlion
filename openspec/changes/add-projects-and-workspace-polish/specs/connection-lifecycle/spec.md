# Spec Delta

## Purpose

Define como o usuário desconecta e reconecta conexões abertas, incluindo a invalidação de sessão e de caches.

## ADDED Requirements

### Requirement: Desconectar e reconectar visíveis
A barra de ferramentas SHALL oferecer os botões Desconectar e Reconectar ao lado de "Nova conexão", atuando sobre a conexão atual, além das entradas no menu de contexto da conexão no navegador e no menu Banco de dados. Os botões SHALL ficar desabilitados quando não houver conexão atual.

#### Scenario: Desconectar
- **WHEN** a conexão atual está aberta e o usuário aciona Desconectar
- **THEN** a sessão é fechada, as caches de esquemas e tabelas daquela conexão são descartadas e o navegador mostra a conexão como desconectada

#### Scenario: Reconectar
- **WHEN** o usuário aciona Reconectar em uma conexão aberta ou cuja sessão caiu
- **THEN** a sessão antiga é descartada, uma nova é aberta com a configuração salva e as caches são recarregadas

#### Scenario: Falha ao reconectar
- **WHEN** a nova conexão falha
- **THEN** o erro legível é exibido, a conexão permanece desconectada e nada trava

### Requirement: Transação pendente ao desconectar
Ao desconectar ou reconectar com alterações pendentes (modo Manual ou Smart), o sistema SHALL perguntar se deseja confirmar, reverter ou cancelar a operação.

#### Scenario: Pendências
- **WHEN** há alterações pendentes e o usuário aciona Desconectar
- **THEN** um diálogo oferece Commit, Rollback ou Cancelar antes de fechar a sessão

### Requirement: Abas de conexão desconectada
Abas abertas de uma conexão desconectada SHALL continuar visíveis, indicar que a conexão está fechada e reabrir a sessão automaticamente ao executar ou recarregar.

#### Scenario: Executar após desconectar
- **WHEN** o usuário executa uma consulta em uma aba cuja conexão foi desconectada
- **THEN** a sessão é reaberta e a consulta roda
