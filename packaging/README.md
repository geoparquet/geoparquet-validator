# Packaging

Files other package managers consume; the release workflow builds the archives they point at.

| Directory | Channel | How it reaches users |
| --- | --- | --- |
| `homebrew/` | Homebrew tap `geoparquet/homebrew-tap` | copy the formula to the tap's `Formula/` on each release; `brew install geoparquet/tap/geoparquet-validator` |
| `winget/` | Windows Package Manager | submit the manifests to microsoft/winget-pkgs on each release; `winget install GeoParquet.Validator` |
| `conda/` | conda-forge | the recipe submitted to conda-forge/staged-recipes; afterwards the feedstock updates itself from crates.io |

The checksums in these files are those of the v0.2.0 release assets and must be refreshed with
every tag; `sh packaging/update.sh v0.2.0` does that.
