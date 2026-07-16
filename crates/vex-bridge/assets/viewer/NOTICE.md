Viewer assets bundled for offline Vex Desktop rendering:

- three 0.149.0, MIT license. See `three/LICENSE`.
- web-ifc 0.0.77, MPL-2.0 license. See `web-ifc/LICENSE.md`.
- web-ifc-three 0.0.126, MIT license per package metadata.
  - `IFCLoader.js`: main-thread loader/API surface used by the dashboard viewer.
  - `IFCWorker.js`: the package's own dedicated Web Worker bundle. Used via
    `ifcManager.useWebWorkers(true, ...)` so large-model parsing/tessellation
    runs off the main thread instead of freezing the UI for the whole load.