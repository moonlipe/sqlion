# Sqlion

Cliente de bancos de dados em Rust (egui) para SQLite, PostgreSQL, MySQL/MariaDB, SQL Server e MongoDB: navegador de bancos, editor SQL, grade de dados editável, exportação e importação, diagrama ER, modos de transação e projetos versionáveis com git.

## Atalhos

| Atalho | Ação |
| --- | --- |
| Ctrl+T / Ctrl+] | Novo script SQL vazio (na conexão atual) |
| F3 | Escolher um script do projeto da aba atual |
| F4 | Abrir o objeto sob o cursor: tabela abre a aba da tabela; view, procedure, função e gatilho abrem a definição |
| Ctrl+Enter / Alt+X | Executar a instrução / o script |
| Ctrl+Shift+F | Formatar SQL |
| Ctrl+/ | Comentar / descomentar |
| Ctrl+Espaço | Autocompletar |
| Page Up / Page Down | Página acima / abaixo no editor (com Shift, seleciona) |
| Ctrl+B | Mostrar / recolher o navegador |
| Ctrl+S / Ctrl+W | Salvar o script / fechar a aba |
| F5 | Atualizar o navegador |

O botão direito no editor abre as ações de edição e, sobre o nome de uma tabela ou outro objeto, as ações desse objeto. Na parte vazia do navegador, abre o menu para criar conexões, projetos, scripts e pastas.

## Modos de transação

Cada conexão tem um modo, escolhido no diálogo de conexão e na barra de ferramentas:

- **Auto**: cada instrução é confirmada na hora.
- **Manual**: a primeira instrução (inclusive um SELECT) abre uma transação, que fica aberta até Commit ou Rollback.
- **Smart**: só instruções que alteram dados ou estrutura abrem transação; consultas de leitura rodam direto e enxergam as alterações pendentes.

Conexões salvas por versões anteriores migram sozinhas (auto-commit ligado → Auto, desligado → Manual).

## Grid

Clique e arraste, Shift+clique e Ctrl+clique selecionam células e linhas; clicar no título seleciona a coluna (a seta ao lado ordena). Ctrl+C copia a seleção como TSV e Ctrl+V cola blocos (uma única ação de desfazer).

## Download e compilação

Cada versão publicada em *Releases* traz binários para Linux (x86_64 e arm64), Windows (x86_64) e macOS (arm64 e x86_64), o código-fonte (`sqlion-<versão>-source.tar.gz` / `.zip`) e o arquivo `SHA256SUMS` (`sha256sum -c SHA256SUMS`). No macOS o aplicativo não é assinado: na primeira execução use `xattr -d com.apple.quarantine ./sqlion`.

### Instalar no Linux

Os pacotes de cada versão estão em *Releases* (troque `<versão>` pelo número, por exemplo `0.3.0`). Todos põem o Sqlion no menu de aplicativos, com ícone.

| Distribuição | Arquivo | Comando |
| --- | --- | --- |
| Debian 12+, Ubuntu 22.04+, Mint, Pop!_OS | `sqlion_<versão>_amd64.deb` / `_arm64.deb` | `sudo apt install ./sqlion_<versão>_amd64.deb` |
| Fedora 36+, RHEL/AlmaLinux/Rocky 10+ | `sqlion-<versão>-x86_64.rpm` / `-aarch64.rpm` | `sudo dnf install ./sqlion-<versão>-x86_64.rpm` |
| openSUSE | `sqlion-<versão>-x86_64.rpm` / `-aarch64.rpm` | `sudo zypper install ./sqlion-<versão>-x86_64.rpm` |
| Arch, Manjaro, EndeavourOS, CachyOS | `PKGBUILD` | baixe o `PKGBUILD` do release numa pasta vazia e rode `makepkg -si` (compila do código-fonte do release, conferido pelo checksum) |
| Qualquer distro (Flatpak) | `sqlion-<versão>-x86_64.flatpak` | `flatpak install --user ./sqlion-<versão>-x86_64.flatpak` e depois `flatpak run io.github.moonlipe.Sqlion` |
| Qualquer distro, sem instalar | `sqlion-<versão>-x86_64.AppImage` / `-aarch64.AppImage` | `chmod +x sqlion-<versão>-x86_64.AppImage && ./sqlion-<versão>-x86_64.AppImage` |

- O AppImage e os pacotes `.deb`/`.rpm` são compilados no Ubuntu 22.04 e pedem glibc 2.35 ou mais nova (Debian 12, Ubuntu 22.04, Fedora 36, RHEL 10, openSUSE Leap 15.6 ou mais novos). RHEL/AlmaLinux/Rocky 9 têm glibc 2.34: neles, compile do código-fonte (abaixo). Sem FUSE instalado, rode o AppImage com `--appimage-extract-and-run`.
- O Flatpak precisa do runtime `org.freedesktop.Platform//24.08` (o `flatpak install` busca no Flathub se o remoto estiver configurado). Ele tem acesso à rede (os bancos) e à pasta pessoal (scripts, projetos e arquivos SQLite).

Para compilar a partir do código-fonte é preciso o [Rust](https://rustup.rs) estável:

```sh
tar xzf sqlion-<versão>-source.tar.gz && cd sqlion-<versão>
cargo build --release --locked   # o executável fica em target/release/sqlion
```

No Linux instale antes `libxkbcommon-dev libwayland-dev libx11-dev libxcursor-dev libxi-dev libxrandr-dev libgl1-mesa-dev` (nomes Debian/Ubuntu).

## Publicar uma versão

1. Atualize `version` em `Cargo.toml` (e `Cargo.lock`, rodando `cargo build`) e acrescente a versão em `<releases>` de `packaging/linux/io.github.moonlipe.Sqlion.metainfo.xml`; faça commit em `main`.
2. Crie e envie a tag: `git tag v0.2.0 && git push origin v0.2.0`.
3. O workflow *Release* compila os 5 alvos, gera os pacotes Linux (`.deb`, `.rpm`, AppImage, Flatpak), o código-fonte, o `PKGBUILD` com o checksum do código-fonte e os checksums, e cria o GitHub Release com notas automáticas. Tags com hífen (`v0.2.0-rc1`) viram pré-lançamento. A tag precisa ser igual à versão do `Cargo.toml`.
4. Para testar sem publicar, dispare o workflow manualmente em *Actions*: os pacotes ficam como artefatos da execução.

## Licença e créditos

Sqlion é software livre sob a GPL-3.0-or-later (veja `LICENSE`).

Inspirado no comportamento do [DBeaver Community](https://github.com/dbeaver/dbeaver) (Apache-2.0, © DBeaver Corp). Valores padrão e regras de comportamento foram lidos do código-fonte do DBeaver e reimplementados em Rust; as referências `Arquivo.java:linha` estão em `src/constants.rs` e `docs/MECHANICS.md`. Sqlion não é afiliado ao DBeaver nem endossado por seus autores. "DBeaver" é marca registrada de seus titulares.
