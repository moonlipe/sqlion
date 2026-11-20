# sqlserver-tls Specification

## Purpose
Garante que o app conecte em servidores SQL Server modernos e antigos (que só aceitam TLS 1.0/1.1 ou suítes de cifra antigas), com uma opção de criptografia por conexão e erros que dizem o que fazer.

## Requirements

### Requirement: Opção de criptografia por conexão SQL Server
O diálogo de conexão SQL Server SHALL ter o campo "Criptografia" com as opções:
- **Automático** (padrão): tenta TLS moderno (1.2/1.3); se o handshake TLS falhar, tenta de novo uma vez no modo legado.
- **Moderno**: só TLS 1.2/1.3.
- **Legado (TLS 1.0+)**: aceita TLS 1.0, 1.1 e suítes de cifra antigas.
- **Sem criptografia**: não negocia TLS; só funciona com servidores que não exigem criptografia, e o diálogo SHALL avisar que o tráfego, inclusive o login, segue sem criptografia.

A escolha SHALL ser salva com a conexão; conexões salvas antes desta mudança SHALL abrir como Automático. O certificado do servidor continua aceito sem validação, como hoje.

#### Scenario: Conexão existente
- **WHEN** o usuário abre uma conexão SQL Server salva por uma versão anterior
- **THEN** ela usa Automático e o campo mostra "Automático"

#### Scenario: Aviso sem criptografia
- **WHEN** o usuário escolhe "Sem criptografia"
- **THEN** o diálogo mostra o aviso de tráfego sem criptografia

### Requirement: Conexão com servidor de TLS antigo
Com Automático ou Legado, o app SHALL conectar em um SQL Server que só oferece TLS 1.0 ou 1.1, ou TLS 1.2 só com suítes sem ECDHE (por exemplo SQL Server 2008/2008 R2/2012 sem atualizações de TLS), em todas as plataformas do release, sem depender da versão de OpenSSL ou da política TLS instalada no sistema.

#### Scenario: Servidor só com TLS 1.0 em Automático
- **WHEN** o usuário conecta em Automático num servidor que só aceita TLS 1.0
- **THEN** a primeira tentativa falha no handshake, a segunda (legado) conecta e as consultas funcionam normalmente

#### Scenario: Servidor moderno
- **WHEN** o usuário conecta em Automático num servidor com TLS 1.2/1.3
- **THEN** a conexão usa TLS moderno na primeira tentativa, sem segunda tentativa

#### Scenario: Moderno contra servidor antigo
- **WHEN** a conexão está em Moderno e o servidor só aceita TLS 1.0
- **THEN** a conexão falha com a mensagem de erro de handshake descrita abaixo

### Requirement: Mensagem de erro de handshake
Quando a conexão falhar no handshake TLS, o app SHALL mostrar, em português, que o servidor recusou a negociação TLS, qual modo de criptografia foi tentado e a sugestão do próximo modo a experimentar (Legado, ou Sem criptografia se o Legado também falhou), mantendo o texto original do erro como detalhe.

#### Scenario: Falha também no legado
- **WHEN** as tentativas moderna e legada falham no handshake
- **THEN** a mensagem diz que o servidor recusou TLS moderno e legado, sugere "Sem criptografia" caso o servidor não exija criptografia, e inclui `tls handshake eof` como detalhe

### Requirement: Teste de conexão usa a mesma negociação
O botão de testar conexão SHALL seguir exatamente a mesma sequência de tentativas da conexão real e informar, em caso de sucesso, se foi usado TLS moderno, legado ou nenhum.

#### Scenario: Teste com fallback
- **WHEN** o usuário testa em Automático uma conexão com servidor de TLS antigo
- **THEN** o teste passa e informa que foi usado TLS legado
