let port: chrome.runtime.Port | null = null;
let state = "Not connected";
chrome.runtime.onMessage.addListener((message: unknown, sender, reply) => {
  if (
    sender.id !== chrome.runtime.id ||
    sender.url !== chrome.runtime.getURL("popup.html")
  )
    return false;
  if (!message || typeof message !== "object" || !("operation" in message))
    return false;
  const operation = (message as { operation: unknown }).operation;
  if (operation === "status") {
    reply({ state });
    return false;
  }
  if (operation === "connect") {
    if (!port) {
      try {
        port = chrome.runtime.connectNative("com.avesra.companion");
        state = "Connecting";
        port.onMessage.addListener((data: unknown) => {
          if (
            data &&
            typeof data === "object" &&
            "status" in data &&
            data.status === "unavailable"
          )
            state =
              "Companion reached; authenticated browser control unavailable";
          else {
            state = "Invalid native response";
            port?.disconnect();
            port = null;
          }
        });
        port.onDisconnect.addListener(() => {
          state = chrome.runtime.lastError
            ? "Native host unavailable. Complete Avesra setup."
            : "Disconnected";
          port = null;
        });
        port.postMessage({ version: 1, type: "status" });
      } catch {
        state = "Native host unavailable. Complete Avesra setup.";
        port = null;
      }
    }
    reply({ state });
    return false;
  }
  if (operation === "disconnect") {
    port?.disconnect();
    port = null;
    state = "Disconnected";
    reply({ state });
  }
  return false;
});
