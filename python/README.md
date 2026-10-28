# Python

Module for [Python](https://www.python.org/), a programming language.

## Concepts

### Programs

[uv](https://github.com/astral-sh/uv) installs Python itself, and the tools below.
`python3` is the version in `program-versions`, also available as `python<version>` for scripts that need that exact one.

- [ruff](https://github.com/astral-sh/ruff): linter and formatter
- [ty](https://github.com/astral-sh/ty): type checker and language server
- [ropify](https://github.com/niqodea/ropify): command line for the [Rope](https://github.com/python-rope/rope) refactoring library

### Neovim

In X mode, `:PythonRuff` and `:PythonMypy` run ruff and mypy on the current file and put the errors in the location list, and `:PythonRuffProject` and `:PythonMypyProject` do the same for the whole project with the quickfix list.
This module does not install mypy.

The `lsp` X module, loaded with `:XLoadLsp`, makes ty the language server for Python files.
It also adds the `:PythonRopify` commands, which move modules and symbols and show imports.
Rope keeps its state in the atdir.
