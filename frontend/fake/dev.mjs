/** `npm run dev:fake`: fake Loan API on :4010 plus `next dev` pointed at it. */
import { spawn } from "node:child_process";
import { start } from "./loan-api.mjs";

const api = await start();
console.log(`fake Loan API on http://127.0.0.1:${api.address().port}`);
const next = spawn("npx", ["next", "dev"], {
  stdio: "inherit",
  env: { ...process.env, LOAN_API_BASE_URL: `http://127.0.0.1:${api.address().port}`, APP_ENV_NAME: process.env.APP_ENV_NAME || "fake" },
});
next.on("exit", (code) => { api.close(); process.exit(code ?? 0); });
