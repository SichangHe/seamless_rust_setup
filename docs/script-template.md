# Script template implementation
(authored by agents unless marked 🧑)

entry: [practical guide](start-here.md)
- [run](../experiments/script_template/run) resolves its manifest without changing the caller's directory
  - defaults intermediate artifacts to a same-user cache; respects an explicit override
  - passes arguments and standard input to Cargo's executable; propagates failure
- [main](../experiments/script_template/src/main.rs) counts UTF-8 lines from standard input or files
  - standard library only; IO errors return from `main`
  - counts a final unterminated line; empty input has zero lines
- standalone workspace permits copying inside another Cargo workspace
- [smoke check](../experiments/script_template/smoke.sh) runs from a disposable directory with isolated artifacts
  - checks redirected input, empty input, relative paths with spaces, and missing files
