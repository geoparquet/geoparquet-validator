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
function conformant(report, conformanceClass = "core") {
  return failed(report, conformanceClass).length === 0;
}
module.exports = { check, checkBytes, failed, conformant };
