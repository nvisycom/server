# Contributing

Thank you for your interest in contributing to Nvisy Server.

## Requirements

- Rust 1.89+ (nightly for formatting)
- PostgreSQL 17+
- NATS 2.10+ (with JetStream)

## Setup

```bash
git clone https://github.com/nvisycom/server.git
cd server
make install-all
make generate-keys
```

### SSH Access

Some dependencies are fetched from private GitHub repositories via SSH. Ensure
your SSH key is added to your GitHub account and ssh-agent is running:

```bash
eval "$(ssh-agent -s)"
ssh-add ~/.ssh/id_ed25519
ssh -T git@github.com  # verify access
```

If cargo fails to fetch git dependencies, enable CLI-based git fetching:

```bash
export CARGO_NET_GIT_FETCH_WITH_CLI=true
```

## Development

Run all CI checks locally before submitting a pull request:

```bash
make ci        # runs check, fmt, clippy, test, docs
make security  # runs cargo deny (requires cargo-deny)
make fmt       # fix formatting (requires nightly)
```

To install the security tools:

```bash
cargo install cargo-deny --locked
```

## Pull Request Process

1. Fork the repository
2. Create a feature branch
3. Make changes with tests
4. Run `make ci` to verify all checks pass
5. Submit a pull request

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
