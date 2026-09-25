let port: chrome.runtime.Port | null = null;
let state = "Not connected";
let generation = 0;
function closeOwned(owned: chrome.runtime.Port, epoch: number, reason: string) {
  if (port !== owned || epoch !== generation) return;
  port = null;
  generation++;
  state = reason;
  owned.disconnect();
}
chrome.runtime.onMessage.addListener((message: unknown, sender, reply) => {
  if (
    sender.id !== chrome.runtime.id ||
    sender.url !== chrome.runtime.getURL("popup.html")
  )
    return false;
  if (!message || typeof message !== "object" || Array.isArray(message) || Object.keys(message).length !== 1 || !("operation" in message))
    return false;
  const operation = (message as { operation: unknown }).operation;
  if (operation === "status") {
    reply({ state });
    return false;
  }
  if (operation === "connect") {
    if (!port) {
      try {
        const owned = chrome.runtime.connectNative("com.avesra.companion");
        const epoch = ++generation;
        port = owned;
        state = "Connecting";
        owned.onMessage.addListener((data: unknown) => {
          if (port !== owned || epoch !== generation) return;
          if (
            data &&
            typeof data === "object" &&
            !Array.isArray(data) &&
            Object.keys(data).length === 3 &&
            "version" in data && data.version === 1 &&
            "reason" in data && data.reason === "authenticated_browser_control_not_configured" &&
            "status" in data &&
            data.status === "unavailable"
          )
            state =
              "Companion reached; authenticated browser control unavailable";
          else {
            closeOwned(owned, epoch, "Invalid native response");
          }
        });
        owned.onDisconnect.addListener(() => {
          const failed = chrome.runtime.lastError;
          if (port !== owned || epoch !== generation) return;
          state = failed
            ? "Native host unavailable. Complete Avesra setup."
            : "Disconnected";
          port = null;
          generation++;
        });
        owned.postMessage({ version: 1, type: "status" });
      } catch {
        state = "Native host unavailable. Complete Avesra setup.";
        const owned = port;
        port = null; generation++;
        owned?.disconnect();
      }
    }
    reply({ state });
    return false;
  }
  if (operation === "disconnect") {
    const owned = port;
    port = null;
    generation++;
    state = "Disconnected";
    owned?.disconnect();
    reply({ state });
  }
  return false;
});
