#!/usr/bin/env -S node --import tsx
/**
 * Generates a RIKU_DASHBOARD_PASSWORD_HASH value from a chosen password.
 *
 * Run with nub:  nub run hash-password -- 'my-chosen-password'
 */
import { hashPassword } from "../lib/password-hash";

const password = process.argv[2];
if (!password) {
  console.error("usage: nub run hash-password -- '<password>'");
  process.exit(1);
}

hashPassword(password).then((hash) => {
  console.log("Add this to the dashboard's environment:\n");
  console.log(`RIKU_DASHBOARD_PASSWORD_HASH=${hash}`);
});
