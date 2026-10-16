# Fill to length: algorithm research (2026-10-16)

## Verdict
No ready-made "supreme" library does this for symbolic tracker blocks. The closest prior work is
**audio music retargeting**: it treats the song as a graph of segments and solves a constrained
shortest path / Viterbi DP for an exact target length. That is the right core for ChipWave. The
style rules and variations stay in your own code.

## Verified finds (checked via `gh repo view`)
| Name | URL | Lang | Licence | Algorithm | Relevance |
|---|---|---|---|---|---|
| radiotool (Rubin & Agrawala, Berkeley) | https://github.com/ucbvislab/radiotool | Python | ISC (GPL-3 compatible) | Music retargeting: beats as states, transition costs, constrained DP/Viterbi to hit an exact length with fixed start/end | **Highest**: the same problem in audio. Inactive since 2015; reference only |
| retarget-service | https://github.com/srubin/retarget-service | Python | none (all rights reserved, don't copy) | Backend for the above | Reference only |
| EternalJukebox | https://github.com/EternalBox/EternalJukebox | HTML/JS | MIT | Infinite Jukebox: beat-similarity graph plus random branch walk | Medium: shows variety from graph jumps, but no length targeting |
| InfiniteJukeboxAlgorithm | https://github.com/rigdern/InfiniteJukeboxAlgorithm | JS | MIT | Cleaned-up Infinite Jukebox branch logic | Medium; small and readable |
| Remixatron | https://github.com/drensin/Remixatron | Python (+ rust-app/) | Apache-2.0 (GPL-3 compatible) | Clusters beats, random walk with no-repeat constraints | Medium; active, has Rust work |
| REMI / Pop Music Transformer | https://github.com/YatingMusic/remi | Python | GPL-3.0 | Transformer note generation | Low: no explicit section planner, needs ML |
| Magenta (MusicVAE etc.) | https://github.com/magenta/magenta | Python | Apache-2.0 | ML generation | Low: no arrangement-to-length |
| echonest/remix | https://github.com/echonest/remix | Python | none listed | Old Echo Nest remix API (dead service) | Historic only |

Papers (not code): Wenner et al., "Scalable Music: Automatic Music Retargeting and Synthesis",
Eurographics 2013, which uses a graph of segments and an optimal path for the target length.
Rubin & Agrawala, UIST 2014, uses a constrained Viterbi in radiotool. Adobe Audition's Remix
works the same way (it is closed source). Game adaptive music (e.g. Wwise, FMOD) uses
hand-authored segment graphs and intensity parameters, which is like the energy curve.
GitHub searches for "song structure generation" and "procedural music structure" found nothing usable.

## Recommended approach for ChipWave
1. **Grammar (style rules)** as a weighted finite-state machine over roles. Example for Pop:
   intro→verse, verse→chorus|verse(0.3), chorus→verse|bridge|breakdown|outro, ..., and outro is
   the only end state. Weights act as Markov probabilities. Durations come from the real blocks
   (rows × speed / BPM).
2. **Exact length by DP**, a constrained shortest path. The state is (role FSM node, last block,
   repeat count, time bin), with time in rows or ticks so it fits exactly. Cost = −log transition
   prob + energy error + repetition penalty. Back-tracking gives the best structure in
   O(states × T), in milliseconds. Sample K-best paths or sample with noise to get varied candidates.
3. **Local polish with simulated annealing** (seeded): swap or insert variations (transposed
   chorus, fill, thin-out, key change) at a fixed length, scoring style, energy and novelty. Locked
   sections are frozen.
4. Keep the seed so results are deterministic, and "Reroll" just changes it.

```
fn fill(target_ticks, blocks, style, energy, seed):
  T = target_ticks / grid             // e.g. grid = 1 bar
  dp[t][node] = INF; dp[0][START] = 0
  for t in 0..T:
    for node, cost in dp[t] where cost<INF:
      for (next_role, p) in style.fsm[node.role]:
        for b in blocks.with_role(next_role):
          t2 = t + bars(b)
          if t2 > T: continue
          rep = node.repeats(b)+1; if rep > style.max_repeat: continue
          c = cost - ln(p) + w_e*energy_err(b, t, energy) + w_r*rep + noise(seed)
          relax dp[t2][Node(next_role,b,rep)] with c, back=(t,node)
  best = argmin over dp[T][n] where n.role==OUTRO (else allow fade-out / trim of last block, or a fill to pad)
  plan = backtrack(best)
  plan = anneal(plan, iters=2000, moves=[variation_swap, fill_insert, transpose_chorus], seed)
  return plan
```
If no exact path exists, relax by allowing ±1 bar, a 1-bar fill block, or a tempo tweak of
1–2%, and report how far it is from the target.

Compared with the current "200 random candidates" plan, the DP guarantees an exact fit and the
best grammar score. The random candidates become the variety step (K-best plus annealing).
Everything is ~300 lines of plain Rust with no dependencies. If you want an ILP or solver, the
good_lp/HiGHS crates exist, but they aren't needed.
