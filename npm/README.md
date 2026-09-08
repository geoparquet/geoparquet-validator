# geoparquet-validator (npm)

The GeoParquet validator compiled to WebAssembly: the abstract tests of the OGC GeoParquet 2.0
draft, and the 1.0 / 1.1 community rules. Same code as the command-line tool and the page at
https://geoparquet.org/geoparquet-validator/.

```js
import { check, conformant, failed } from "geoparquet-validator";

const report = await check("example.parquet");        // or an https:// URL, downloaded whole
console.log(report.version, conformant(report, "core"), failed(report));
```

In a browser, import `geoparquet-validator/bundler` through your bundler and call
`check_bytes(name, uint8array)`, which returns the report as JSON text. The report's shape is
`schemas/report.schema.json` in the repository. Source and issues:
https://github.com/geoparquet/geoparquet-validator
