/* Build (from the repository root, after `cargo build --release --features capi`):
 *   cc examples/check.c -Iinclude -Ltarget/release -lgeoparquet_validator -o check
 *   LD_LIBRARY_PATH=target/release ./check corpus/data/bbox/bbox-present.parquet
 */
#include <stdio.h>
#include <string.h>
#include "geoparquet_validator.h"

int main(int argc, char **argv) {
    if (argc < 2) { fprintf(stderr, "usage: check <file-or-url>\n"); return 2; }
    printf("geoparquet-validator %s\n", gpv_version());
    char *report = gpv_check(argv[1], 0);
    if (!report) { fprintf(stderr, "out of memory\n"); return 2; }
    /* the report is JSON; a real program would parse it, this one just looks for failures */
    int failed = strstr(report, "\"status\":\"fail\"") != NULL;
    printf("%.160s...\n", report);
    printf("%s\n", failed ? "NOT CONFORMANT" : "no failed test");
    gpv_free(report);
    return failed ? 1 : 0;
}
