# Guia Rápido de Início - Regent Rust + Artichoke

Este guia te ajuda a começar com o novo projeto Regent baseado em Rust + Artichoke Ruby.

## ⚡ Início Rápido (5 minutos)

### Pré-requisitos
```bash
# Instalar Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Verificar instalação
rustc --version
cargo --version
```

### Compilar o Projeto

```bash
# Entrar no diretório
cd /Users/felipe/Dev/regent

# Build em desenvolvimento
cargo build

# Build para produção (otimizado)
cargo build --release

# O executável estará em:
# target/debug/regent     (dev)
# target/release/regent   (prod, ~5MB)
```

### Testar

```bash
# Rodar todos os testes
cargo test

# Testar com output detalhado
cargo test -- --nocapture

# Testar um módulo específico
cargo test cli::new --lib
```

### Usar o CLI

```bash
# Ajuda geral
./target/release/regent --help

# Criar novo módulo
./target/release/regent new test_module \
  --author "Seu Nome" \
  --license Apache-2.0 \
  --description "Meu módulo de teste"

# Validar módulo
./target/release/regent validate test_module

# Gerar classe
./target/release/regent generate class myclass --module-path test_module

# Gerar task
./target/release/regent generate task mytask --module-path test_module

# Construir pacote
./target/release/regent build --path test_module

# Rodar testes
./target/release/regent test --path test_module
```

## 📁 Estrutura de Pastas

### Código Rust (Performance)
```
src/
├── main.rs                  # Entrada CLI
├── lib.rs                   # Raiz da biblioteca
├── cli/
│   ├── mod.rs              # CLI dispatcher
│   ├── new.rs              # Command: novo módulo
│   ├── generate.rs         # Command: gerar componente
│   ├── validate.rs         # Command: validar
│   ├── build.rs            # Command: construir
│   └── test.rs             # Command: testes
├── config.rs               # Configuração
├── generator.rs            # Lógica de geração
├── validator.rs            # Validação
├── builder.rs              # Build system
├── tester.rs               # Test runner
├── ruby_interop.rs         # Bridge Rust↔Ruby
└── artichoke_runtime.rs    # Config Artichoke
```

### Código Ruby (Compatibilidade)
```
lib/
└── regent/
    ├── version.rb          # Versão
    ├── config.rb           # Config Ruby
    ├── generator.rb        # Generator Ruby
    └── ...

spec/
├── regent_spec.rb
└── regent/
    ├── config_spec.rb
    └── ...
```

### Templates & Docs
```
templates/                  # Templates para módulos
├── spec_helper.rb
├── Rakefile
├── task_ruby.rb
├── task_shell.sh
├── task_python.py
└── plan.pp

ARCHITECTURE.md            # Arquitetura detalhada
ARTICHOKE_INTEGRATION.md   # Guia Artichoke
RUST_RUBY_INTEROP.md       # Guia interop Rust/Ruby
EXAMPLES.md                # Exemplos práticos
CONTRIBUTING.md            # Como contribuir
MIGRATION.md               # Detalhes da migração
```

## 🔧 Desenvolvimento

### Format & Lint
```bash
# Formatar código Rust
cargo fmt

# Verificar problemas (Rust)
cargo clippy -- -D warnings

# Verificar segurança
cargo audit
```

### Debug

```bash
# Build com symbols de debug
cargo build

# Executar com debug info
RUST_LOG=debug ./target/debug/regent --help

# Usar debugger
rust-gdb ./target/debug/regent
```

### Performance

```bash
# Build release otimizado
cargo build --release

# Benchmark
cargo bench

# Profile CPU
perf record -g ./target/release/regent new test
perf report
```

## 📚 Documentação

### Lendo a Arquitetura
1. Começar: [ARCHITECTURE.md](./ARCHITECTURE.md)
2. Integração Ruby: [ARTICHOKE_INTEGRATION.md](./ARTICHOKE_INTEGRATION.md)
3. Interop detalhado: [RUST_RUBY_INTEROP.md](./RUST_RUBY_INTEROP.md)
4. Exemplos: [EXAMPLES.md](./EXAMPLES.md)

### Entender o Código Rust
```bash
# Ver documentação gerada
cargo doc --open

# Pesquisar no código
grep -r "pub fn" src/

# Ver estrutura de um arquivo
cargo expand src/cli/new.rs
```

## 🐛 Troubleshooting

### "Rust not found"
```bash
# Instalar/atualizar Rust
rustup update
```

### "Cargo not found"
```bash
# Adicionar ao PATH
export PATH="$HOME/.cargo/bin:$PATH"
```

### Build falha
```bash
# Limpar e rebuildar
cargo clean
cargo build --release
```

### Testes falham
```bash
# Rodar um teste específico
cargo test name_of_test -- --nocapture

# Rodar testes ignorados
cargo test -- --include-ignored

# Debug de teste
RUST_LOG=debug cargo test test_name -- --nocapture
```

## 📝 Checklist de Desenvolvimento

- [ ] Clonar repositório: `git clone https://github.com/ffquintella/regent`
- [ ] Instalar Rust: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- [ ] Build: `cargo build --release`
- [ ] Testes: `cargo test`
- [ ] Format: `cargo fmt`
- [ ] Lint: `cargo clippy`
- [ ] Documentação: `cargo doc --open`

## 💡 Dicas Úteis

### Rápido Compile Turnaround
```bash
# Use cargo-watch para rebuild automático
cargo install cargo-watch
cargo watch -x build -x test
```

### Melhor Performance
```bash
# Use sccache para cachear compilações
cargo install sccache
export RUSTC_WRAPPER=sccache
```

### Entender Dependências
```bash
# Mostrar árvore de dependências
cargo tree

# Atualizar dependências
cargo update
```

### Git Workflow

```bash
# Criar feature branch
git checkout -b feature/sua-feature

# Fazer commits
git commit -m "feat: descrição da mudança"

# Push para seu fork
git push origin feature/sua-feature

# Criar Pull Request no GitHub
```

## 🎯 Próximos Passos

1. **Ler** a arquitetura: [ARCHITECTURE.md](./ARCHITECTURE.md)
2. **Explorar** o código: `cargo doc --open`
3. **Criar** um teste: `cargo test`
4. **Contribuir**: Ver [CONTRIBUTING.md](./CONTRIBUTING.md)

## 📞 Ajuda

- 📖 Documentação: [README.md](./README.md)
- 🎓 Tutoriais: [EXAMPLES.md](./EXAMPLES.md)
- 🐛 Problemas: [Issues](https://github.com/ffquintella/regent/issues)
- 💬 Discussões: [Discussions](https://github.com/ffquintella/regent/discussions)

---

**Última atualização**: Janeiro 2026
**Maintainer**: Felipe Quintella
