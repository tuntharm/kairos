# Kairos Design System

Production-ready starter assets for the Kairos Tauri 2 + React desktop app.

## What is inside

- Approved raster app icon and a clean vector recreation
- macOS iconset, `.icns`, Windows `.ico`, and common Tauri PNG sizes
- Gradient, dark, light, and monochrome brand marks
- Wordmarks and horizontal lockups
- macOS menu-bar template icon plus active and attention states
- Routing-state icons
- Brain and file citation markers
- Brain-type utility icons
- Animated SVG, CSS, React loader, and Lottie JSON
- Design tokens in CSS and JSON
- React/TypeScript starter components
- Browser preview and implementation notes

## Install in the Kairos repository

Copy this folder into your repo, for example:

```bash
cp -R kairos-design-system /Users/tharm/dev/kairos/design
```

For Tauri 2, generate the platform bundle icons from the 1024px source:

```bash
pnpm tauri icon design/assets/brand/kairos-app-icon-1024.png
```

Or copy the prepared files from `assets/brand/` into `src-tauri/icons/`.

## Menu bar

Use `assets/menu-bar/kairos-template.png` or its SVG source. On macOS, configure it as a template image so the operating system can tint it correctly in light and dark menu bars. The active and attention variants are supplied for custom state handling.

## Small-size artwork

The 16–64 px iconset files use a simplified vector rendering. This is deliberate: the approved 3D master loses edge clarity at menu and Finder-list sizes.

## Brand semantics

- Broken ring: the opening or opportune moment
- K-shaped node graph: connected brains and routing
- Centre: composer / synthesis
- Gold point: selected action at the right moment

## Primary palette

- Background `#030814`
- Surface `#07142B`
- Routing blue `#4C9FFF`
- Active cyan `#76E4FF`
- Ivory `#F4F7FF`
- Kairos gold `#FFC766`

Open `preview/index.html` to inspect the package visually.
