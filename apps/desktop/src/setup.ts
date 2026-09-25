import { command } from "./runtime";

/** UI coordination only. Each native operation still consumes its own proof. */
export async function ensureManagementVerification(
  current: () => boolean,
  progress: (message: string) => void,
) {
  const check = () => {
    if (!current()) throw new Error("Owner setup changed. Reopen this section and try again.");
  };
  check();
  progress("Checking Windows verification…");
  const status = await command<{ local_authentication: string }>("setup_status");
  check();
  if (status.local_authentication !== "verified") {
    if (status.local_authentication !== "available") {
      throw new Error("Windows verification is unavailable. Check Windows sign-in options before owner setup.");
    }
    progress("Verify with Windows Hello to continue…");
    await command("verify_setup");
    check();
  }
}
