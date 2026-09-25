async function send(operation: string) {
  const response: unknown = await chrome.runtime.sendMessage({ operation });
  if (
    response &&
    typeof response === "object" &&
    "state" in response &&
    typeof response.state === "string"
  )
    document.getElementById("status")!.textContent = response.state;
}
document
  .getElementById("connect")!
  .addEventListener("click", () => void send("connect"));
document
  .getElementById("disconnect")!
  .addEventListener("click", () => void send("disconnect"));
void send("status");
setInterval(() => void send("status"), 1000);
