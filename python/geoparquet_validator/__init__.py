"""Validate GeoParquet files.

    >>> import geoparquet_validator as gpv
    >>> report = gpv.check("example.parquet")
    >>> report["outcomes"][0]
    {'id': '/conf/core/geo-metadata', 'status': 'pass', 'message': ''}
    >>> gpv.conformant(report, "core")
    True

The report is a plain dict with the shape of schemas/report.schema.json in the repository.
"""

from __future__ import annotations

import json
import sys
from typing import Any, Optional

from . import _native

__version__ = _native.__version__
__all__ = ["check", "check_bytes", "conformant", "failed", "main", "__version__"]


def check(target: str, max_rows: Optional[int] = None) -> dict[str, Any]:
    """Check a local path or an s3://, gs://, az:// or https:// URL.

    `max_rows` reads only the first row groups holding that many rows; the report then has
    `sampled: True` and its data tests are a sample, not a conformance pass.
    """
    return json.loads(_native.check(target, max_rows))


def check_bytes(name: str, data: bytes, max_rows: Optional[int] = None) -> dict[str, Any]:
    """Check a file already in memory; `name` only labels the report."""
    return json.loads(_native.check_bytes(name, data, max_rows))


def failed(report: dict[str, Any], conformance_class: Optional[str] = None) -> list[str]:
    """The ids of the failed tests, optionally within one class: core, covering or distribution."""
    return [
        o["id"]
        for o in report["outcomes"]
        if o["status"] == "fail"
        and (conformance_class is None or o["id"].startswith(f"/conf/{conformance_class}/"))
    ]


def conformant(report: dict[str, Any], conformance_class: str = "core") -> bool:
    """True when no test of the class failed. A class with every test skipped is not claimed;
    this still returns True, so check `report["outcomes"]` if the distinction matters."""
    return not failed(report, conformance_class)


def main(argv: Optional[list[str]] = None) -> int:
    """The `geoparquet-validator` command; returns the exit code."""
    code = _native.main(sys.argv if argv is None else ["geoparquet-validator", *argv])
    if argv is None:
        sys.exit(code)
    return code
