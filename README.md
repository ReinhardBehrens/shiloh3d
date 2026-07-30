<p align="center">
  <img src="logo_shiloh3d.png" alt="Shiloh3D" width="180" />
</p>

<p align="center">
  <strong>Build worlds. Ship games. Stay in control.</strong><br/>
  <sub>A modern 3D game engine in Rust — fast, safe, and built for real production.</sub>
</p>

<p align="center">
  <a href="#why-shiloh3d">Why Shiloh3D</a> ·
  <a href="#what-you-get">What you get</a> ·
  <a href="#get-started">Get started</a> ·
  <a href="#roadmap">Roadmap</a> ·
  <a href="#learn-more">Learn more</a>
</p>

---

## Why Shiloh3D?

Shiloh3D is a **from-scratch 3D game engine** for teams that want:

- A **polished editor** and clear workflow  
- **High-end look** where it matters (lighting, materials, atmosphere, water)  
- **Large worlds** and multiplayer as first-class goals — not afterthoughts  
- An open stack you can own, extend, and ship with confidence  

We compete on **usability** and **selected graphical quality** — not on matching every feature of a commercial mega-engine on day one.

> *“And the whole congregation of the children of Israel assembled together at Shiloh, and set up the tabernacle of the congregation there. And the land was subdued before them.”*  
> — Joshua 18:1 (KJV)

---

## What you get

| | |
|---|---|
| **Runtime** | Solid frame loop, entities, scenes, input, timing |
| **Look** | Lit 3D path with shadows, fog, water, HUD, and color grading |
| **Content** | glTF import, save/load scenes, isometric camera for ARPG / RTS feel |
| **Tools** | Scene editor (hierarchy, inspector, play mode) · project CLI |
| **Platforms** | Desktop bring-up today · path to native GPU and web |

**Quality target:** cinematic ARPG swamp presentation — mood, atmosphere, and **real water**. See the [quality bar](docs/QUALITY_BAR.md).

---

## Get started

**Need:** Rust 1.85+ and a C linker (`gcc` or `clang`).

```bash
# Play the showcase
cargo run -p shiloh-demo

# Open the scene editor
cargo run -p shiloh-editor

# Quick headless check
cargo run -p shiloh-demo -- --headless-frames 60
```

| Control | Action |
|---|---|
| WASD | Pan |
| Drag | Pan |
| Scroll | Zoom |
| Esc | Quit |

---

## How the product is organized

Everything is a focused library. Games talk to **Shiloh APIs** — not raw third-party engine guts.

| Piece | What it does for you |
|---|---|
| **App** | Window, lifecycle, run loop |
| **Editor** | Docked Studio shell — outliner, inspector, node graph, world items, URL import, play / stop |
| **Scene** | Worlds of objects, parents/children, cameras |
| **Assets** | Load meshes and materials (glTF) |
| **Render** | Draw the frame — lights, shadows, water, UI overlay |
| **Animation · Physics · Audio** | Motion, bodies, sound (growing every phase) |
| **Scripting** | Rust gameplay modules first; JS-style scripting later |
| **Network** | Multiplayer foundations early |
| **CLI** | New project and packaging helpers |

Full module list and policies live in the docs below — this page stays product-focused.

---

## Roadmap

| Phase | Focus | Status |
|---|---|---|
| **1** | Core engine & world-builder foundation | **Done** |
| **2** | Usable 3D vertical slice | **Done** |
| **3** | Production workflow + Blackmarsh-class water & atmosphere | **Next** |
| **4** | Scale — streaming, advanced graphics, multiplayer depth | Later |

**Right now:** deepen **real water**, fog, and mood toward the quality bar, while making day-to-day content work smoother (reload, package, profile).

Details: [Roadmap](docs/ROADMAP.md) · [Quality bar](docs/QUALITY_BAR.md)

---

## Learn more

| Doc | For |
|---|---|
| [Quality bar](docs/QUALITY_BAR.md) | The look we’re aiming at |
| [Roadmap](docs/ROADMAP.md) | What’s done and what’s next |
| [Tech stack](docs/TECH_STACK.md) | How we choose and wrap tools |
| [Graphics](docs/GRAPHICS.md) | Desktop, web, and GPU paths |
| [Premium brief](docs/PREMIUM.md) | Product overview & gallery |
| [Showcase demo](shiloh-demo/README.md) | Running the sample |

---

## License

MIT **or** Apache-2.0 — your choice.

---

<p align="center">
  <img src="logo_shiloh3d.png" alt="" width="56" />
</p>
