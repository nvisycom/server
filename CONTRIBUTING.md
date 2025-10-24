# Contributing

Thank you for your interest in contributing to the Nvisy API Server.

## Requirements

- Rust 1.89 or higher
- PostgreSQL 17 or higher
- NATS 2.10 or higher (for messaging, sessions, and queues)
- MinIO RELEASE.2025 or higher (for object storage)
- OpenRouter API key (for AI features)
- PaddleX service (for document processing and OCR)
- Mistral AI API key (for advanced document understanding)
- OpenSSL (for key generation)

## Development Setup

```bash
git clone https://github.com/nvisycom/api.git
cd api
make install-all
```

## Development

### Scripts

- `make install-all` - Install required tools and make scripts executable
- `make generate-keys` - Generate RSA key pair for JWT authentication
- `make generate-migrations` - Run database migrations and update schema
- `make clear-migrations` - Revert all database migrations

### Quality Checks

Before submitting changes:

```bash
make install-all        # Setup development environment
cargo fmt --all         # Format code
cargo clippy            # Lint code
cargo test --workspace  # Run tests
cargo build --release   # Verify release build
```

## Pull Request Process

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests for new functionality
5. Run quality checks: `cargo fmt && cargo clippy && cargo test`
6. Submit a pull request

### Pull Request Checklist

- [ ] Tests pass (`cargo test --workspace`)
- [ ] Code follows Rust style guide (`cargo fmt`)
- [ ] No clippy warnings (`cargo clippy`)
- [ ] Documentation updated if needed
- [ ] Database migrations included (`make generate-migrations`)
- [ ] No breaking changes (or properly documented)

## Code Standards

- Follow standard Rust formatting with `rustfmt`
- Write comprehensive tests for new features
- Use `#[must_use]` for functions that return important values
- Include rustdoc comments for public APIs
- Prefer explicit error handling over panics
- Follow workspace patterns established in existing crates

## Documentation Standards

### README Format

All crate README files should follow this consistent format:

```markdown
# api.nvisy.com/crate-name

Brief description of the crate's purpose and key functionality.

[![rust](https://img.shields.io/badge/Rust-1.89+-000000?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![relevant-badge](https://img.shields.io/badge/Technology-Version+-000000?style=flat-square&logo=tech&logoColor=white)](https://example.com/)

## Features

- **Feature Name** - Brief description of what it does
- **Another Feature** - Brief description with technical details
- **Error Handling** - Comprehensive error types with context

## Key Dependencies

- `dependency-name` - Brief description of what it provides
- `another-dep` - Brief description of its role in the crate

## Architecture/Capabilities

Brief section explaining the crate's architecture or key capabilities.
```

### Documentation Requirements

- Keep README files concise and focused on essential information
- Use consistent badge formatting with the project color scheme
- Include Features and Key Dependencies sections for all crates
- Avoid excessive code examples - link to docs.rs instead
- Use present tense and active voice
- Follow the established pattern from nvisy-postgres, nvisy-nats

## Database Changes

- Always create migrations for schema changes
- Test migrations both forward and backward
- Update the schema file with `make generate-migrations`
- Include migration rollback strategies

## Security

- Never commit secrets or API keys
- Use environment variables for configuration
- Validate all external inputs
- Follow secure coding practices for authentication

## License

By contributing, you agree your contributions will be licensed under the MIT
License.
