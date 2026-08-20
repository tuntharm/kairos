# MLX-LM Worker Boundary V0

This directory is the fixed Python process boundary for `kairos-lab`. V0 accepts
one protocol-versioned JSON request on stdin and emits bounded NDJSON events on
stdout. The only implemented operation is deterministic `smoke_noop`.

It deliberately contains no MLX import, model resolution, download, source-data
reader, or training implementation. A later gate may add direct MLX-LM execution
behind the same validated envelope after the licence, immutable model revision,
runtime lock, private-data staging, cancellation, and artifact-hash checks exist.

Run the dependency-free tests with Python 3.12 or newer:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s workers/mlx_lm_worker -p 'test_*.py' -v
```
