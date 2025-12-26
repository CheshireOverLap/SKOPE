# SKOPE Engine

A high-performance game engine built with Rust, designed for cross-platform game development with Blender integration.

## Philosophy

**SKOPE = Rust Engine + Blender Editor**

- **Engine**: Written in Rust for performance, safety, and modern graphics (Vulkan/wgpu)
- **Editor**: Blender as the primary content creation and level editing tool
- **Target Platforms**: Linux (Steam Machine/AMD) and Windows

## Features (Planned)

### Engine Core
- Vulkan-based rendering via wgpu
- ECS architecture with bevy_ecs
- 3D physics with rapier3d
- Cross-platform support (Linux & Windows)
- Hot-reload asset pipeline

### Blender Integration
- Direct .blend file import
- GLTF/GLB export workflow
- Custom Blender addons for SKOPE
- Level design in Blender
- Material and lighting pipeline

## Tech Stack

- **Language**: Rust 2021 edition
- **Graphics API**: Vulkan (via wgpu 0.18)
- **Windowing**: winit 0.29
- **Math**: glam 0.25
- **ECS**: bevy_ecs (planned)
- **Physics**: rapier3d (planned)
- **Asset Loading**: gltf (planned)

## Project Structure

```
SKOPE/
├── src/
│   └── main.rs         # Engine entry point
├── assets/             # Game assets (Blender files, models, textures)
├── examples/           # Example projects
├── Cargo.toml          # Rust dependencies
└── README.md
```

## Development Setup

### Prerequisites
- Rust 1.70+ (edition 2021)
- Vulkan SDK
- Blender 3.0+
- Linux or Windows

### Building

```bash
# Clone the repository
git clone https://github.com/SKOPEGames/SKOPE.git
cd SKOPE

# Build in development mode
cargo build

# Run
cargo run

# Build optimized release
cargo build --release
```

## Development Roadmap

### Phase 1: Foundation (Current)
- [x] Project initialization
- [x] Basic Cargo setup
- [ ] Window creation with wgpu
- [ ] Vulkan rendering pipeline
- [ ] Basic triangle rendering

### Phase 2: Core Systems (1-2 months)
- [ ] ECS integration
- [ ] 3D model loading (GLTF)
- [ ] Camera system
- [ ] Basic lighting
- [ ] Input handling

### Phase 3: Blender Pipeline (2-3 months)
- [ ] Direct .blend import
- [ ] Material conversion
- [ ] Scene graph import
- [ ] Custom Blender exporter addon
- [ ] Asset hot-reload

### Phase 4: Advanced Features (3-6 months)
- [ ] PBR rendering
- [ ] Shadow mapping
- [ ] Physics integration
- [ ] Audio system
- [ ] Particle system

## Why Rust + Blender?

**Rust Engine Benefits:**
- Memory safety without garbage collection
- Zero-cost abstractions
- Fearless concurrency
- Modern graphics API support (Vulkan)
- Cross-platform with native performance

**Blender Editor Benefits:**
- Professional-grade 3D tooling
- Python scripting for custom workflows
- Active community and ecosystem
- Free and open source
- Industry-standard features

## Contributing

This is currently a personal project by SKOPE Games. Contributions welcome in the future!

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Contact

**SKOPE Games**
Email: admin@skope.games
Repository: https://github.com/SKOPEGames/SKOPE

---

Built with ❤️ using Rust and Blender
