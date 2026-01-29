# nvisy-rig

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/server/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/server/actions/workflows/build.yml)

AI/LLM orchestration layer. Provides unified interfaces for LLM providers and
agent workflows.

## Installation

```bash
# Core package
uv add nvisy-rig

# With specific providers
uv add "nvisy-rig[openai,anthropic]"

# All providers
uv add "nvisy-rig[all]"
```

## Available Providers

| Provider  | Extra       | Description                |
| --------- | ----------- | -------------------------- |
| OpenAI    | `openai`    | GPT models, embeddings     |
| Anthropic | `anthropic` | Claude models              |
| Cohere    | `cohere`    | Command models, embeddings |

## Usage

```python
from nvisy_rig.agents import Agent

# Create an agent
agent = Agent(
    model="gpt-4",
    system_prompt="You are a helpful assistant.",
)

# Run completion
response = await agent.complete("Hello, world!")
print(response)
```

## Architecture

This package provides the Python AI/LLM layer for the nvisy system:

- **nvisy-dal**: Data access layer (storage, databases, vector stores)
- **nvisy-rig**: AI orchestration layer (LLM providers, agents, RAG)

## Development

```bash
# Install dev dependencies
uv sync --extra dev

# Run tests
uv run pytest

# Type check
uv run pyright

# Lint
uv run ruff check .
```

## Changelog

See [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License - see [LICENSE.txt](../../LICENSE.txt)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/server/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
- **API Status**: [nvisy.openstatus.dev](https://nvisy.openstatus.dev)
