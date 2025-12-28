# Contributing

Thank you for your interest in contributing to Nvisy Server.

## Requirements

- Rust 1.89+
- PostgreSQL 17+
- NATS 2.10+ (with JetStream)
- Ollama (for AI features)

## Setup

```bash
git clone https://github.com/nvisycom/server.git
cd server
make install-all
make generate-keys
```

## Pull Request Process

1. Fork the repository
2. Create a feature branch
3. Make changes with tests
4. Run checks: `cargo fmt && cargo clippy && cargo test`
5. Submit a pull request

### Checklist

- [ ] Tests pass (`cargo test --workspace`)
- [ ] Code formatted (`cargo fmt`)
- [ ] No clippy warnings (`cargo clippy -- -D warnings`)
- [ ] Migrations included if needed (`make generate-migrations`)

## Database Changes

- Create migrations for schema changes
- Test migrations forward and backward
- Update schema with `make generate-migrations`

## Security

- Never commit secrets or API keys
- Use environment variables for configuration
- Validate all external inputs

## License

By contributing, you agree your contributions will be licensed under the MIT
License.
