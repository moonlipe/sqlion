## ADDED Requirements

### Requirement: CI de verificação
O repositório SHALL ter um workflow que roda testes e compilação em pull requests e em pushes para `main`.

#### Scenario: Pull request
- **WHEN** um pull request é aberto ou atualizado
- **THEN** os testes e a compilação rodam e o resultado aparece no PR

### Requirement: Release automático por tag
Ao empurrar uma tag no formato `vMAJOR.MINOR.PATCH`, o sistema SHALL compilar o Sqlion em modo release para Linux x86_64, Linux arm64, Windows x86_64, macOS arm64 e macOS x86_64, e publicar um GitHub Release com os binários empacotados, o código-fonte arquivado e um arquivo de checksums.

#### Scenario: Tag de versão
- **WHEN** a tag `v0.2.0` é empurrada e a versão do `Cargo.toml` é 0.2.0
- **THEN** são publicados no release um pacote por plataforma, os arquivos de código-fonte e um arquivo SHA-256 cobrindo todos, com notas geradas dos commits

#### Scenario: Versão divergente
- **WHEN** a tag não corresponde à versão do `Cargo.toml`
- **THEN** o workflow falha antes de compilar, informando a divergência

#### Scenario: Falha em uma plataforma
- **WHEN** a compilação de uma plataforma falha
- **THEN** nenhum release é publicado

### Requirement: Pacotes por plataforma
Cada pacote SHALL conter o executável, a licença e o README, com nome que identifique versão, sistema e arquitetura, em `.tar.gz` (Linux e macOS) ou `.zip` (Windows).

#### Scenario: Nome do pacote
- **WHEN** o release da versão 0.2.0 é publicado
- **THEN** os arquivos seguem o padrão `sqlion-0.2.0-<alvo>.tar.gz` ou `.zip`

### Requirement: Disparo manual
O workflow de release SHALL poder ser iniciado manualmente para gerar os pacotes como artefatos de teste, sem publicar release.

#### Scenario: Execução manual
- **WHEN** o workflow é disparado manualmente
- **THEN** os pacotes ficam disponíveis como artefatos da execução e nenhum release é criado

### Requirement: Código-fonte no release
Cada release SHALL incluir o código-fonte da versão em `.tar.gz` e `.zip`, sem diretórios de build nem arquivos locais, com nome `sqlion-<versão>-source`, de modo que qualquer pessoa possa compilar sem clonar o repositório. O README SHALL explicar como compilar a partir desse arquivo.

#### Scenario: Baixar o código-fonte
- **WHEN** o usuário baixa `sqlion-0.2.0-source.zip` do release e extrai
- **THEN** a pasta contém `Cargo.toml`, `Cargo.lock`, `src/`, `LICENSE` e `README.md`, e `cargo build --release` compila o aplicativo

#### Scenario: Conteúdo limpo
- **WHEN** o arquivo de código-fonte é gerado
- **THEN** ele não contém `target/`, `.git/`, bancos locais, `.env` nem a pasta `openspec/`
