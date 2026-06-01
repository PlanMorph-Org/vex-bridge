// Injected into the Vex Atlas webview before page load (see desktop.rs).
// Exposes native-only capabilities to the dashboard JS via window.__vexNative.
// In a plain browser this object is absent, so the dashboard falls back to
// manual entry / server-side browser opening.
(function () {
  if (window.__vexNative) return;
  var pending = new Map();
  var seq = 0;

  function post(message) {
    if (window.ipc && typeof window.ipc.postMessage === "function") {
      window.ipc.postMessage(JSON.stringify(message));
    }
  }

  window.__vexNative = {
    available: true,

    // Open a native folder picker. Resolves to the absolute path string, or
    // null if the user cancelled.
    pickFolder: function () {
      return new Promise(function (resolve) {
        var requestId = "fp-" + ++seq;
        pending.set(requestId, resolve);
        post({ type: "pickFolder", requestId: requestId });
      });
    },

    // Open a URL in the user's real default browser (used for account pairing).
    openExternal: function (url) {
      post({ type: "openExternal", url: url });
    },

    // Called from Rust once the native folder dialog closes.
    _onFolderPicked: function (payload) {
      var resolve = pending.get(payload.requestId);
      if (resolve) {
        pending.delete(payload.requestId);
        resolve(payload.path || null);
      }
    },
  };
})();
