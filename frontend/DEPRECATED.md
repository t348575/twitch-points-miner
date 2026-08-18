# Deprecated

This is the original Svelte + Bun UI. It has been replaced by `../ui`, which is
built with Vite, TypeScript, React, Mantine and ECharts.

Nothing builds this directory anymore: Docker (`app.dockerfile`), CI
(`.github/workflows/ui.yml`) and the devcontainer all target `../ui`.

It is kept only as a reference while the new UI settles. Both apps build to the
repo-root `dist/`, so running `bun run build` here would overwrite the UI the
backend serves. Run `cd ../ui && npm run build` to restore it.
