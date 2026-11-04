// CommonJS entry point for Node.
const { readFile } = require("node:fs/promises");
const native = require("./node/geoparquet_validator.js");

function checkBytes(name, bytes) {
  return JSON.parse(native.check_bytes(name, bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes)));
}
async function check(target) {
  if (/^https?:\/\//.test(target)) {
    const res = await fetch(target);
    if (!res.ok) throw new Error(`${target}: HTTP ${res.status}`);
    return checkBytes(target, new Uint8Array(await res.arrayBuffer()));
  }
  return checkBytes(target, new Uint8Array(await readFile(target)));
}
function failed(report, conformanceClass) {
  return report.outcomes
    .filter((o) => o.status === "fail" && (!conformanceClass || o.id.startsWith(`/conf/${conformanceClass}/`)))
    .map((o) => o.id);
}
function claimed(report, conformanceClass) {
  return report.outcomes.some((o) => o.status !== "skip" && o.id.startsWith(`/conf/${conformanceClass}/`));
}
// With no class: core for a file with `geo` metadata, native for one with only the native types.
function conformant(report, conformanceClass) {
  const cls = conformanceClass ?? (claimed(report, "native") ? "native" : "core");
  if (cls === "core" && !claimed(report, "core")) return false;
  return failed(report, cls).length === 0;
}
module.exports = { check, checkBytes, failed, conformant };
