# Phase Compete — E2E FirstGoal gate report

**Result:** FAIL · **Similarity:** 0.188 (need ≥ 0.42) · **Features:** 45/100 · **Requirements:** 100/100

- Capture: `docs/screenshots/gate-latest.png` (1280×720)
- Reference (uploaded editor viewport): `docs/references/firstgoal-valley-viewport.png`
- Instance density: 90
- Spec: [PHASE_COMPETE.md](../PHASE_COMPETE.md) · mockup: [firstgoal-studio-editor.png](../references/firstgoal-studio-editor.png)

## Similarity vs FirstGoal viewport

- `sim composite=0.188 (SSIM 0.105 · hist_corr -0.238 · mae 0.347)`
- `reference: docs/references/firstgoal-valley-viewport.png`

## FirstGoal quality features

- `+0  weak sky/cool upper band (6%) — FirstGoal has blue sky`
- `+0  low edge detail (2.1) — proxies look flat vs FirstGoal pines`
- `+0  too many flat color runs (275) — greybox plates`
- `+15 water/cool mid band (70%)`
- `+15 depth luminance spread (0.95)`
- `+15 warm sun key (17%)`

## Latest Phase 5 / Compete requirements

- `+10 EDITOR_UX.md borrow map`
- `+15 FirstGoal reference present`
- `+10 Landscape/Foliage scene types`
- `+10 Rhai host + ScriptComponent`
- `+10 Editor layouts + content cook stubs`
- `+10 RayAccurate / Parry crate`
- `+5 Blender peer pipeline doc`
- `+5 Phase Compete spec`
- `+15 Valley instance density ≥ 60`
- `+10 Gate still path writable`

## Goal

E2E success = capture **matches** the uploaded FirstGoal Studio valley still (photoreal pines, river, sky, atmosphere) **and** Phase 5 authoring requirements. Greybox proxies must **fail**.
