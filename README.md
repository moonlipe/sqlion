// Read the file directly from the source path

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

Para compilar a partir do código-fonte é preciso o [Rust](https://rustup.rs) estável:

```sh
tar xzf sqlion-<versão>-source.tar.gz && cd sqlion-<versão>
cargo build --release --locked   # o executável fica em target/release/sqlion
```

No Linux instale antes `libxkbcommon-dev libwayland-dev libx11-dev libxcursor-dev libxi-dev libxrandr-dev libgl1-mesa-dev` (nomes Debian/Ubuntu).

## Publicar uma versão

1. Atualize `version` em `Cargo.toml` (e `Cargo.lock`, rodando `cargo build`), faça commit em `main`.
2. Crie e envie a tag: `git tag v0.2.0 && git push origin v0.2.0`.
3. O workflow *Release* compila os 5 alvos, gera o código-fonte e os checksums e cria o GitHub Release com notas automáticas. Tags com hífen (`v0.2.0-rc1`) viram pré-lançamento. A tag precisa ser igual à versão do `Cargo.toml`.
4. Para testar sem publicar, dispare o workflow manualmente em *Actions*: os pacotes ficam como artefatos da execução.

## Licença e créditos

Sqlion é software livre sob a GPL-3.0-or-later (veja `LICENSE`).

Inspirado no comportamento do [DBeaver Community](https://github.com/dbeaver/dbeaver) (Apache-2.0, © DBeaver Corp). Valores padrão e regras de comportamento foram lidos do código-fonte do DBeaver e reimplementados em Rust; as referências `Arquivo.java:linha` estão em `src/constants.rs` e `docs/MECHANICS.md`. Sqlion não é afiliado ao DBeaver nem endossado por seus autores. "DBeaver" é marca registrada de seus titulares.
