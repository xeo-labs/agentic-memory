# AgenticMemory Terminal Agent

Interactive terminal agent for testing and validating AgenticMemory. Used for Phase 7A and 7B validation.

## Features

- Interactive chat with persistent memory
- 6 validation protocols (basic recall, decision recall, correction persistence, long-range memory, cross-topic inference, stress testing)
- Cross-provider testing (Claude, GPT, Ollama)
- 97 tests passing

## Usage

```bash
cd agent/
pip install -e ".[dev]"
python -m amem_agent
```

## Validation

```bash
python -m amem_agent.validation.run_all
```

See [validation results](../validation/phase7a_results.md) for full test report.

## License

MIT
