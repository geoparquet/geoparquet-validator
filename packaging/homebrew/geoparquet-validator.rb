# Homebrew formula for the geoparquet-validator command. Lives in the geoparquet/homebrew-tap
# repository as Formula/geoparquet-validator.rb; this copy is the source of truth and is updated
# with every release:  brew install geoparquet/tap/geoparquet-validator
class GeoparquetValidator < Formula
  desc "Validate GeoParquet files against the spec and the OGC 2.0 abstract tests"
  homepage "https://validator.geoparquet.org/"
  license "Apache-2.0"

  on_macos do
    on_arm do
      url "https://github.com/geoparquet/geoparquet-validator/releases/download/v0.3.0/geoparquet-validator-v0.3.0-aarch64-apple-darwin.tar.gz"
      sha256 "7a4587c05de4eb8bdecb977ada86ab0840a2bd5f4d92bbf65b0e800efbd48961"
    end
    on_intel do
      url "https://github.com/geoparquet/geoparquet-validator/releases/download/v0.3.0/geoparquet-validator-v0.3.0-x86_64-apple-darwin.tar.gz"
      sha256 "6e494536e1dd6d7d2e33159922ddea822a02ae08ea722b786785eb0935354343"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/geoparquet/geoparquet-validator/releases/download/v0.3.0/geoparquet-validator-v0.3.0-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "3b854bd41c684a80d70dba1bcb3bb536570d8f6db4f55afb72c182a3ad643ce2"
    end
    on_intel do
      url "https://github.com/geoparquet/geoparquet-validator/releases/download/v0.3.0/geoparquet-validator-v0.3.0-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "7dd0e6609e8ece81d8db78e78c910c778e2237bc988b1764cf48e4c14b94f88d"
    end
  end

  def install
    bin.install "geoparquet-validator"
    lib.install Dir["libgeoparquet_validator.*"]
    include.install "include/geoparquet_validator.h"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/geoparquet-validator --version")
  end
end
