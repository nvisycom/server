# nvisy-rig

AI/LLM orchestration layer. Provides unified interfaces for LLM providers and agent workflows.

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

| Provider | Extra | Description |
|----------|-------|-------------|
| OpenAI | `openai` | GPT models, embeddings |
| Anthropic | `anthropic` | Claude models |
| Cohere | `cohere` | Command models, embeddings |

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

## TODO

- [ ] OpenAI provider
- [ ] Anthropic provider
- [ ] Cohere provider
- [ ] Agent framework
- [ ] RAG pipelines
- [ ] Tool integration
