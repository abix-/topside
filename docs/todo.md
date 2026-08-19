# topside open issues

| Priority | System | Todo | Done when |
|---:|---|---|---|
| 1 | docs/design.md | [ ] Decide the core loop and write docs/design.md: what the player sees and does minute to minute (view, controls, session shape), how the bunker start works, and how the extraction, incremental, and loot-randomization ideas fit around the base simulation | docs/design.md exists and the operator has approved the core loop |
| 1 | World | [ ] Decide the world substrate and engine for topside, naming prior art for each candidate considered | The open question in docs/authority.md is replaced by the recorded decision and its reasoning |
| 5 | modforge | [ ] Decide where the game systems live in the modforge workspace (modules in the foundation crate versus a new workspace crate) | The decision is recorded in docs/authority.md and the modforge readme |
| 10 | Storyteller | [ ] Read survivalist-mod's storyteller code (incursion.rs, stranger.rs) and list every game read and write it performs, answering lift versus rewrite behind a binder trait | The lift-versus-rewrite answer is recorded in the docs/authority.md storyteller row |
| 15 | Genome | [ ] Read survivalist-mod's genome and franchise-vote code and list every game read and write it performs, answering lift versus rewrite behind a binder trait | The lift-versus-rewrite answer is recorded in the docs/authority.md genome row |
| 20 | Acts | [ ] Read survivalist-mod's act modules and list every game read and write they perform, answering lift versus rewrite behind a binder trait | The lift-versus-rewrite answer is recorded in the docs/authority.md acts row |
| 30 | Content registry | [ ] Decide the checked-in content format for topside definitions (items, structures, factions, incursion kinds, upgrade tracks), naming prior art (factoriobot Blueprints, survivalist story XML) | The decision is recorded in docs/authority.md row 12 |
