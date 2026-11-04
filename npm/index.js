// Node entry point (ES modules). Browsers should import "geoparquet-validator/bundler" through a
// bundler, or use the ready-made page at https://geoparquet.org/geoparquet-validator/.
import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";

const native = createRequire(import.meta.url)("./node/geoparquet_validator.js");

/** Check a file already in memory. Returns the report object (schemas/report.schema.json). */
export function checkBytes(name, bytes) {
  return JSON.parse(native.check_bytes(name, bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes)));
}

/** Check a local file, or an http(s) URL. A URL is downloaded whole first: the range-request
 *  reader needs the synchronous callbacks the browser worker provides, which Node does not have;
 *  for big remote files use the command line, which reads only what it needs. */
export async function check(target) {
  if (/^https?:\/\//.test(target)) {
    const res = await fetch(target);
    if (!res.ok) throw new Error(`${target}: HTTP ${res.status}`);
    return checkBytes(target, new Uint8Array(await res.arrayBuffer()));
  }
  return checkBytes(target, new Uint8Array(await readFile(target)));
}

/** Ids of the failed tests, optionally within one class (core, covering, distribution, native). */
export function failed(report, conformanceClass) {
  return report.outcomes
    .filter((o) => o.status === "fail" && (!conformanceClass || o.id.startsWith(`/conf/${conformanceClass}/`)))
    .map((o) => o.id);
}

/** Whether any test of the class ran (a class with every test skipped is not claimed). */
function claimed(report, conformanceClass) {
  return report.outcomes.some((o) => o.status !== "skip" && o.id.startsWith(`/conf/${conformanceClass}/`));
}

/**
 * True when the file conforms. With no class, the class the file is judged by: core for a file
 * with GeoParquet `geo` metadata, native for one whose geometry columns use the native Parquet
 * GEOMETRY/GEOGRAPHY types without it. With "core", a file that does not claim core is not
 * conformant; for the other classes, true when no test of the class failed.
 */
export function conformant(report, conformanceClass) {
  const cls = conformanceClass ?? (claimed(report, "native") ? "native" : "core");
  if (cls === "core" && !claimed(report, "core")) return false;
  return failed(report, cls).length === 0;
}
