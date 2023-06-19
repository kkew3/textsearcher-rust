# Multithreaded search for query in a group of text files

This project aims to search for keywords in a groups of text.
The project was originally designed to search in text parsed from PDFs using [`pdftotext`](https://www.xpdfreader.com/pdftotext-man.html), but can be applied to other sources of text as well.
Each text file should be small enough to be able to read into memory all at once.
The keywords are structured in an expressive AND-of-ORs-of-literals query and then compiled into certain regex that is robust to extraneous spaces in text.
A Python interface is designed to allow invocation of `search_text` in Python side.

## AND-of-ORs-of-literals query

- `primary_atom`: the primary literal to search for
- `and_of_or_atoms`: e.g. `[[A, B], [C]]` means to search for (A **OR** B) **AND** (C) where each of A, B, C is a literal.

`primary_atom` and `and_of_or_atoms` are **AND**ed together to form the overall query.

## Example usage from Python side

```python
from glob import glob
from textsearcher import textsearcher

q = textsearcher.QueryGroup(
    'primary_literal',
    [['A', 'alternative name for A'], ['B', 'alternative name for B']])
files = textsearcher.FilePaths(glob('*.txt'))
results = textsearcher.search_text(q, files)
```

## Build Python package

In your virtual environment,

```bash
pip install maturin
maturin new -b pyo3 textsearcher
# Copy the `[dependencies]` section of Cargo.toml to the new textsearcher/Cargo.toml.
# Copy the content of src/lib.rs to the new textsearcher/src/lib.rs.
# And then,
maturin develop --release
```
