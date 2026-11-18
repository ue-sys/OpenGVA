# OpenGVA's design, in SysML v2

| File | What |
|---|---|
| `structure.sysml` | the crew station: image layers, the software as it runs, the HMI's crates, the bench |
| `behaviour.sysml` | boot to first frame, the display state, a role-locked area, an alarm |
| `requirements.sysml` | who uses it, what it has to do, what satisfies each and how it is checked |
| `views.sysml` | the diagrams: one view each |

A diagram exists because a view in `views.sysml` asks for it. Open the files
in SysML Fabric (VS Code extension or web app) to see and arrange them; the
arrangement is saved beside them in `.fabric` files.

On a push to `main` that changes `sysml/`,
`.github/workflows/sysml-fabric-pages.yml` publishes them to GitHub Pages:
the site, `views/<file>/<view>-<kind>.svg` and `.png` for each view, and the
model as `OpenGVA.kpar` (the run's artifact).

To build the same site locally, from a checkout of SysML Fabric beside this one:

    INPUT_FOLDER=sysml INPUT_PATH=/tmp/site INPUT_TITLE=OpenGVA \
        node ../sysml-core/packages/action/dist/index.mjs
