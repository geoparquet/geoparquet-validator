/* geoparquet-validator C interface. Build the library with `cargo build --release --features capi`
 * (libgeoparquet_validator.so / .dylib / .dll next to the binary). Every call returns a JSON
 * report with the shape of schemas/report.schema.json, or {"error": "..."}; free it with gpv_free. */
#ifndef GEOPARQUET_VALIDATOR_H
#define GEOPARQUET_VALIDATOR_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Check a local path or an s3://, gs://, az:// or https:// URL. max_rows 0 reads every row. */
char *gpv_check(const char *target, uint64_t max_rows);

/* Check a file held in memory; name only labels the report and may be NULL. */
char *gpv_check_bytes(const char *name, const uint8_t *data, size_t len, uint64_t max_rows);

/* Free a string returned by gpv_check or gpv_check_bytes. NULL is ignored. */
void gpv_free(char *report);

/* The library version. Static; do not free. */
const char *gpv_version(void);

#ifdef __cplusplus
}
#endif
#endif
