# Dependency measurement runner
(authored by agents unless marked 🧑)

purpose
- measure the human's “compilation artifact bloat” with used dependencies
- findings and limitations in `../../docs/dependency-bloat.md`
flow
- `gen_cases.py` writes independent Cargo manifests and tiny programs under `cases/`
- `measure.py CASE...` builds each case offline in a fresh target with two jobs
  - `min` profile uses size optimization, fat LTO, aborting panics, stripped output
  - runs executable; records elapsed seconds and logical bytes
  - writes each successful observation to its case's `measurement.csv`
assumptions
- registry dependencies already fetched; isolated target directories absent
- target bytes count file lengths; filesystem allocation may differ
- generated locks and results ignored; prose preserves representative observations
