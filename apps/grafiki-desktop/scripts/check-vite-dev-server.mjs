const VITE_DEV_URL = "http://127.0.0.1:1420/";

const controller = new AbortController();
const timeout = setTimeout(() => controller.abort(), 1_000);

try {
  const response = await fetch(VITE_DEV_URL, { signal: controller.signal });
  if (!response.ok) throw new Error(`HTTP ${response.status}`);
} catch {
  console.error(
    `Grafiki e2e requires the Vite dev server at ${VITE_DEV_URL}. Start it in another terminal with: npm run dev`,
  );
  process.exit(1);
} finally {
  clearTimeout(timeout);
}
