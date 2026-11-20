# Spec Delta

## ADDED Requirements

### Requirement: Pacotes Linux nativos
Cada release SHALL publicar, além dos `.tar.gz`, pacotes Linux instaláveis gerados do mesmo binário de release:
- `.deb` para x86_64 (`amd64`) e arm64 (`arm64`), instalável em Debian 12+, Ubuntu 22.04+ e derivadas (Mint, Pop!_OS);
- `.rpm` para x86_64 e aarch64, instalável em Fedora 36+, RHEL/AlmaLinux/Rocky 10+ e openSUSE Leap 15.6+ (glibc 2.35 ou mais nova, a do Ubuntu 22.04 onde os binários são compilados; RHEL 9 e derivadas compilam do código-fonte);
- AppImage para x86_64 e aarch64, executável sem instalação;
- bundle Flatpak (`.flatpak`) para x86_64, instalável com `flatpak install` a partir do arquivo.

Os nomes SHALL seguir o padrão `sqlion-<versão>-<arquitetura>.<extensão>` (para `.deb`, `sqlion_<versão>_<arquitetura>.deb`). Todos os pacotes SHALL entrar no arquivo SHA-256 do release.

#### Scenario: Instalar no Ubuntu
- **WHEN** o usuário baixa `sqlion_0.3.0_amd64.deb` e roda `sudo apt install ./sqlion_0.3.0_amd64.deb` no Ubuntu 22.04
- **THEN** o pacote instala as dependências de sistema necessárias, o comando `sqlion` fica disponível e o app aparece no menu de aplicativos com ícone

#### Scenario: Instalar no Fedora
- **WHEN** o usuário roda `sudo dnf install ./sqlion-0.3.0-x86_64.rpm`
- **THEN** o app é instalado e aparece no menu de aplicativos

#### Scenario: AppImage
- **WHEN** o usuário marca `sqlion-0.3.0-x86_64.AppImage` como executável e o abre
- **THEN** o app inicia sem instalação nem permissões de administrador

#### Scenario: Flatpak
- **WHEN** o usuário roda `flatpak install --user sqlion-0.3.0-x86_64.flatpak`
- **THEN** o app é instalado e abre pelo menu ou por `flatpak run`, com acesso à pasta pessoal para scripts e projetos e à rede para os bancos

#### Scenario: Falha no empacotamento
- **WHEN** a geração de qualquer pacote Linux falha
- **THEN** nenhum release é publicado

#### Scenario: Execução manual
- **WHEN** o workflow é disparado manualmente
- **THEN** os pacotes Linux ficam disponíveis como artefatos da execução, sem criar release

### Requirement: Integração com o desktop Linux
Todos os pacotes Linux SHALL instalar um arquivo `.desktop` (nome Sqlion, categoria Development/Database), o ícone do app em PNG nos tamanhos 64, 128 e 256 e o arquivo de metadados AppStream, e o app SHALL usar o mesmo identificador de aplicativo da entrada `.desktop`, de modo que a janela fique associada ao ícone certo no painel do desktop.

#### Scenario: Ícone na barra de tarefas
- **WHEN** o app instalado por qualquer um dos pacotes é aberto no GNOME ou no KDE
- **THEN** a janela aparece no painel com o ícone do Sqlion, agrupada com o lançador do menu

### Requirement: PKGBUILD para Arch Linux
O repositório SHALL manter um `PKGBUILD` que compila o Sqlion a partir do arquivo de código-fonte do release, verificado pelo checksum, e instala o binário, a licença, o `.desktop` e os ícones, pronto para ser usado com `makepkg -si` ou publicado no AUR. O README SHALL listar como instalar em cada família de distribuição.

#### Scenario: Compilar no Arch
- **WHEN** o usuário roda `makepkg -si` na pasta do PKGBUILD da versão 0.3.0 no Arch ou em uma derivada (Manjaro, EndeavourOS, CachyOS)
- **THEN** o pacote é compilado a partir do código-fonte do release e instalado, e o app aparece no menu de aplicativos
