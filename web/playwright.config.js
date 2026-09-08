import { defineConfig } from "@playwright/test";
export default defineConfig({
  testDir: "./tests",
  workers: 1,
  timeout: 45000,
  use: { viewport: { width: 1280, height: 900 }, trace: "retain-on-failure" },
  reporter: "list",
});
