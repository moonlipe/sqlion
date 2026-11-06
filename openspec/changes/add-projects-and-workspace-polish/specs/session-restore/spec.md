# Spec Delta

## Purpose

Mantém o espaço de trabalho entre sessões: ao abrir o aplicativo, as abas voltam como estavam, se o usuário quiser.

## ADDED Requirements

### Requirement: Salvar abas ao fechar
Ao fechar o aplicativo, e periodicamente durante o uso, o sistema SHALL salvar a lista de abas abertas: tipo, conexão, tabela ou diagrama, texto do script (inclusive alterações não gravadas), posição do cursor, aba ativa e visões do grid (modo e ordenação).

#### Scenario: Reabrir
- **WHEN** o usuário fecha o aplicativo com três abas abertas e o abre de novo
- **THEN** as mesmas três abas reaparecem na mesma ordem, com a mesma aba ativa

#### Scenario: Script não gravado
- **WHEN** uma aba de script tem alterações não gravadas ao fechar
- **THEN** o texto não gravado volta na próxima sessão, marcado como alterado

#### Scenario: Conexão removida
- **WHEN** uma aba pertence a uma conexão que não existe mais
- **THEN** a aba é ignorada na restauração, sem erro

### Requirement: Restauração sob demanda de conexões
A restauração SHALL NOT abrir conexões automaticamente. Abas de tabela e de diagrama SHALL carregar seus dados quando o usuário as ativar ou recarregar.

#### Scenario: Banco indisponível
- **WHEN** o servidor está fora do ar na abertura
- **THEN** o aplicativo abre normalmente e a aba mostra o erro só ao ser carregada

### Requirement: Restauração configurável
Existirá uma preferência "Reabrir as abas da última sessão", ligada por padrão. Com ela desligada, o aplicativo SHALL abrir apenas a página de boas-vindas e SHALL NOT guardar o estado das abas.

#### Scenario: Desligar
- **WHEN** o usuário desliga a preferência e reinicia o aplicativo
- **THEN** só a página de boas-vindas aparece

#### Scenario: Estado ilegível
- **WHEN** o arquivo de sessão está corrompido
- **THEN** o aplicativo abre com a boas-vindas e descarta o arquivo
