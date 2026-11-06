# Spec Delta

## Purpose

Organiza conexões e scripts em projetos que vivem em uma pasta, podem ser versionados com git e compartilhados por um repositório, e permite levá-los entre computadores por exportação e importação seletivas.

## ADDED Requirements

### Requirement: Projeto em pasta
Um projeto SHALL ser uma pasta no disco, escolhida pelo usuário, contendo o arquivo `sqlion-project.json` (nome, conexões, pastas de conexões, ícones e cores) e a subpasta `scripts/` com arquivos `.sql`. Sempre existirá um projeto padrão, na pasta de configuração do aplicativo. O usuário SHALL poder criar um projeto em uma pasta, abrir uma pasta existente ou clonada como projeto, renomear, fechar e mover conexões e scripts entre projetos. O navegador SHALL mostrar a árvore por projeto.

#### Scenario: Criar projeto em uma pasta
- **WHEN** o usuário cria o projeto "Financeiro" na pasta `~/repos/financeiro`
- **THEN** a pasta passa a ter `sqlion-project.json` e `scripts/`, e o projeto aparece no navegador

#### Scenario: Abrir pasta clonada
- **WHEN** o usuário abre como projeto uma pasta clonada de um repositório que tem `sqlion-project.json`
- **THEN** as conexões e os scripts do projeto aparecem, sem senhas

#### Scenario: Scripts salvos em um projeto
- **WHEN** o usuário salva um script em um projeto
- **THEN** o arquivo `.sql` fica em `scripts/` dentro da pasta do projeto

#### Scenario: Dados da versão anterior
- **WHEN** o aplicativo abre dados da versão anterior, com scripts soltos e conexões sem projeto
- **THEN** tudo aparece no projeto padrão, sem perda

#### Scenario: Fechar projeto
- **WHEN** o usuário fecha um projeto aberto
- **THEN** ele sai do navegador, as abas dele se fecham e os arquivos da pasta permanecem intactos

### Requirement: Segredos ficam fora do projeto
O arquivo do projeto e a pasta `scripts/` SHALL NOT conter senhas, histórico de consultas nem estado das abas. As senhas SHALL ficar apenas na máquina do usuário, na pasta de configuração do aplicativo, associadas a um identificador estável da conexão. Uma conexão cujo identificador não tem senha local SHALL pedir a senha ao conectar e oferecer guardá-la localmente.

#### Scenario: Primeiro uso após clonar
- **WHEN** o usuário conecta pela primeira vez em uma conexão vinda de um clone
- **THEN** o aplicativo pede a senha e, se o usuário aceitar, guarda-a localmente sem alterar a pasta do projeto

#### Scenario: Senha não vai para o repositório
- **WHEN** o usuário edita a senha de uma conexão de projeto
- **THEN** `sqlion-project.json` não muda por causa da senha

#### Scenario: Identificador estável
- **WHEN** duas pessoas clonam o mesmo repositório
- **THEN** a mesma conexão tem o mesmo identificador para ambas, e cada uma guarda a própria senha

### Requirement: Configuração de equipe versionada
O arquivo do projeto SHALL guardar, por conexão: nome, tipo de banco, host, porta, banco, usuário, modo de transação, tipo de conexão (Desenvolvimento, Teste, Produção), somente leitura, pasta, ícone e cor. O tipo de conexão e a proteção de somente leitura SHALL valer para quem abrir o projeto, incluindo a confirmação antes de executar comandos que alteram dados em Produção.

#### Scenario: Produção protegida para a equipe
- **WHEN** uma conexão de Produção do projeto é aberta por outra pessoa
- **THEN** a confirmação antes de executar comandos de escrita também vale para ela

#### Scenario: Scripts vinculados à conexão
- **WHEN** um script do projeto foi executado em uma conexão do projeto
- **THEN** o vínculo é guardado no projeto pelo identificador da conexão e o script reabre nela em outra máquina

### Requirement: Versionamento git do projeto
O sistema SHALL mostrar o estado git da pasta do projeto (arquivos alterados) e SHALL permitir registrar um commit com mensagem, criar o repositório se a pasta ainda não for um, e sugerir um `.gitignore` que exclua arquivos locais. Enviar e receber (push e pull) SHALL ser feito pelo git do sistema fora do aplicativo; o aplicativo SHALL recarregar o projeto quando a pasta mudar.

#### Scenario: Commit do projeto
- **WHEN** o usuário altera um script e aciona o commit do projeto
- **THEN** o arquivo é registrado no repositório da pasta com a mensagem informada

#### Scenario: Pasta sem repositório
- **WHEN** o usuário aciona o versionamento em uma pasta que não é um repositório
- **THEN** o aplicativo oferece criar o repositório

#### Scenario: Mudança externa
- **WHEN** um `git pull` altera `sqlion-project.json` ou scripts
- **THEN** o aplicativo carrega as alterações ao receber o foco, sem perder abas com texto não gravado

### Requirement: Exportar projeto
O sistema SHALL exportar um projeto para um arquivo `.sqlion` (JSON), para backup e transporte pontual. O diálogo de exportação SHALL listar o que pode ser incluído, cada item com caixa de seleção: conexões (com ou sem senha), scripts, preferências, aparência (ícones e cores), histórico de consultas. Senhas incluídas SHALL ir ofuscadas, e o diálogo SHALL avisar que ofuscar não é criptografar.

#### Scenario: Exportação completa
- **WHEN** o usuário marca todos os itens e exporta
- **THEN** o arquivo contém conexões, scripts, preferências, aparência e histórico do projeto

#### Scenario: Sem senhas
- **WHEN** o usuário desmarca "senhas"
- **THEN** o arquivo não contém nenhuma senha

#### Scenario: Seleção parcial
- **WHEN** o usuário seleciona só alguns scripts e conexões
- **THEN** o arquivo contém apenas esses itens

### Requirement: Importar projeto
O sistema SHALL importar um arquivo `.sqlion`, mostrando antes o conteúdo com caixas de seleção, em um projeto novo (em uma pasta escolhida) ou em um projeto existente. Em conflito de nome ou identificador, o usuário SHALL escolher entre manter ambos (renomeando), substituir ou ignorar. A importação SHALL NOT sobrescrever nada sem confirmação.

#### Scenario: Importar em novo projeto
- **WHEN** o usuário importa um arquivo e escolhe criar um projeto em uma pasta
- **THEN** o projeto é criado na pasta com os itens selecionados e seus scripts

#### Scenario: Conflito de nome
- **WHEN** já existe um script com o mesmo nome
- **THEN** o usuário escolhe renomear, substituir ou ignorar antes de qualquer alteração

#### Scenario: Arquivo inválido
- **WHEN** o arquivo não é um `.sqlion` válido ou tem versão desconhecida
- **THEN** uma mensagem clara é exibida e nada é alterado

### Requirement: Compatibilidade de formato
O arquivo do projeto e o arquivo exportado SHALL registrar a versão do formato, e a leitura SHALL aceitar versões anteriores suportadas e recusar versões desconhecidas sem alterar nada.

#### Scenario: Versão registrada
- **WHEN** um projeto é salvo ou exportado
- **THEN** o arquivo traz `format_version`, e o exportado também o nome e a versão do aplicativo
