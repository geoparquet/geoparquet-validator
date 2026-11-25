# Homebrew formula for the geoparquet-validator command. Lives in the geoparquet/homebrew-tap
# repository as Formula/geoparquet-validator.rb; this copy is the source of truth and is updated
# with every release:  brew install geoparquet/tap/geoparquet-validator
class GeoparquetValidator < Formula
  desc "Validate GeoParquet files against the spec and the OGC 2.0 abstract tests"
  homepage "https://validator.geoparquet.org/"
  license "Apache-2.0"

  on_macos do
    on_arm do
      url "https://github.com/geoparquet/geoparquet-validator/releases/download/v0.4.0/geoparquet-validator-v0.4.0-aarch64-apple-darwin.tar.gz"
      sha256 "57ee566f45cb06ce120e16893cb105ec3ddb8f5ad066256b7b5cdc5c015db430"
    end
    on_intel do
      url "https://github.com/geoparquet/geoparquet-validator/releases/download/v0.4.0/geoparquet-validator-v0.4.0-x86_64-apple-darwin.tar.gz"
      sha256 "43ba60011ec1873c62d42ca70c848e524453646ab503215e280595ef49f8767e"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/geoparquet/geoparquet-validator/releases/download/v0.4.0/geoparquet-validator-v0.4.0-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "db7c571300b81af8215327af80727efe44ffc0dca2a0baa9c58706e6da0a0db0"
    end
    on_intel do
      url "https://github.com/geoparquet/geoparquet-validator/releases/download/v0.4.0/geoparquet-validator-v0.4.0-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "f807614a731385b758fedc000cdab186f98c999d9a631eadfe5b8c6b2225ac5d"
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
